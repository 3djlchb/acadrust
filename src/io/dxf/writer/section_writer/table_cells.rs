//! Legacy table cells are a cache; the owned TABLECONTENT keeps style inheritance.
use crate::entities::table::{
    BorderPropertyFlags as B, CellEdgeFlags as E, CellStylePropertyFlags as P,
};
use crate::entities::{CellBorder, CellStyle, Table, TableCell};
use std::collections::HashMap;

fn merge(a: &CellBorder, b: &CellBorder) -> Option<CellBorder> {
    let mut result = a.clone();
    macro_rules! property {
        ($flag:ident, $field:ident) => {
            if b.override_flags.contains(B::$flag) {
                if a.override_flags.contains(B::$flag) && a.$field != b.$field {
                    return None;
                }
                result.$field = b.$field;
            }
        };
    }
    property!(BORDER_TYPE, border_type);
    property!(LINE_WEIGHT, line_weight);
    property!(LINE_TYPE, line_type_handle);
    property!(COLOR, color);
    property!(INVISIBILITY, invisible);
    property!(DOUBLE_LINE_SPACING, double_spacing);
    result.override_flags |= b.override_flags;
    Some(result)
}

fn cell_mut(table: &mut Table, row: usize, col: usize) -> Option<&mut TableCell> {
    table.rows.get_mut(row)?.cells.get_mut(col)
}

fn apply(
    table: &mut Table,
    row: usize,
    col: usize,
    edge: E,
    border: &CellBorder,
    virtual_edge: bool,
) {
    let Some(cell) = cell_mut(table, row, col) else {
        return;
    };
    let style = cell.style.get_or_insert_with(CellStyle::new);
    match edge {
        E::TOP => style.top_border = border.clone(),
        E::RIGHT => style.right_border = border.clone(),
        E::BOTTOM => style.bottom_border = border.clone(),
        E::LEFT => style.left_border = border.clone(),
        _ => unreachable!(),
    }
    style.applied_border_edges |= edge;
    cell.edge_flags |= edge.bits() as u8;
    if virtual_edge {
        cell.virtual_edge |= edge.bits() as i16;
    } else {
        cell.virtual_edge &= !(edge.bits() as i16);
    }
}

pub(super) fn project(table: &mut Table) {
    // Resolve only explicitly asserted text heights. A stored default (0.18)
    // without its override bit must never replace an inherited row height.
    for row in &mut table.rows {
        for (col, cell) in row.cells.iter_mut().enumerate() {
            if cell.style.as_ref().is_some_and(|s| s.legacy_override_bits) {
                continue;
            }
            let content_height = cell
                .contents
                .first()
                .filter(|c| c.format_override_flags & P::TEXT_HEIGHT.bits() as i32 != 0)
                .map(|c| c.text_height);
            let asserted =
                |s: Option<&CellStyle>| s.filter(|s| s.sets(P::TEXT_HEIGHT)).map(|s| s.text_height);
            let height = content_height
                .or_else(|| asserted(cell.style.as_ref()))
                .or_else(|| {
                    let column = asserted(table.columns.get(col).and_then(|c| c.style.as_ref()));
                    let row = asserted(row.style.as_ref());
                    match (column, row) {
                        // Keep the authoritative styles in TABLECONTENT rather than
                        // guess a precedence for conflicting inherited properties.
                        (Some(a), Some(b)) if a != b => None,
                        (Some(height), _) | (_, Some(height)) => Some(height),
                        _ => asserted(table.base_style.as_ref()),
                    }
                });
            if let Some(height) = height.filter(|v| v.is_finite() && *v > 0.0) {
                let style = cell.style.get_or_insert_with(CellStyle::new);
                style.override_flags |= P::TEXT_HEIGHT.bits() as i32;
                style.text_height = height;
            }
        }
    }
    // One geometric edge is shared by two cells. The upper/left cell owns
    // the real payload; the other carries a virtual edge to the same values.
    // Conflicting explicitly asserted properties are left independent.
    let mut borders: HashMap<(bool, usize, usize), Option<CellBorder>> = HashMap::new();
    for (r, row) in table.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            let Some(style) = cell.style.as_ref().filter(|s| !s.legacy_override_bits) else {
                continue;
            };
            for (edge, key, border) in [
                (E::TOP, (false, r, c), &style.top_border),
                (E::BOTTOM, (false, r + 1, c), &style.bottom_border),
                (E::LEFT, (true, r, c), &style.left_border),
                (E::RIGHT, (true, r, c + 1), &style.right_border),
            ] {
                if !style.applied_border_edges.contains(edge) || border.override_flags.is_empty() {
                    continue;
                }
                borders
                    .entry(key)
                    .and_modify(|existing| {
                        *existing = existing.as_ref().and_then(|a| merge(a, border));
                    })
                    .or_insert_with(|| Some(border.clone()));
            }
        }
    }
    for ((vertical, r, c), border) in borders {
        let Some(border) = border else {
            continue;
        };
        let sides = if vertical {
            if c == 0 {
                vec![(r, c, E::LEFT, false)]
            } else {
                vec![(r, c - 1, E::RIGHT, false), (r, c, E::LEFT, true)]
            }
        } else if r == 0 {
            vec![(r, c, E::TOP, false)]
        } else {
            vec![(r - 1, c, E::BOTTOM, false), (r, c, E::TOP, true)]
        };
        if sides.iter().any(|(r, c, _, _)| {
            table
                .rows
                .get(*r)
                .and_then(|r| r.cells.get(*c))
                .and_then(|c| c.style.as_ref())
                .is_some_and(|s| s.legacy_override_bits)
        }) {
            continue;
        }
        for (r, c, edge, virtual_edge) in sides {
            apply(table, r, c, edge, &border, virtual_edge);
        }
    }
    for row in &mut table.rows {
        for cell in &mut row.cells {
            let projected = super::table_format::with_legacy_cell(cell).into_owned();
            *cell = projected;
        }
        row.style = None;
    }
    for column in &mut table.columns {
        column.style = None;
    }
    table.base_style = None;
    // Duplicating these global overrides makes native readers reapply them
    // over the complete per-cell cache, discarding its borders and heights.
    if let Some(style) = table.legacy_style_override.as_mut() {
        style.flags &= !0x700018;
        style.horizontal_cell_margin = None;
        style.vertical_cell_margin = None;
        style.row_heights.clear();
    }
    table.override_flag = table
        .legacy_style_override
        .as_ref()
        .is_some_and(|s| s.flags != 0);
}
