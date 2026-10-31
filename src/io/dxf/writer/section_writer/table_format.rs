//! Project representable modern instance formatting into the legacy ACAD_TABLE DXF fields.
use crate::entities::table::{CellStylePropertyFlags as P, Table};
use std::borrow::Cow;

pub(super) fn with_legacy_format(table: &Table) -> Cow<'_, Table> {
    if table.base_style.is_none() && table.rows.iter().all(|row| row.style.is_none()) {
        return Cow::Borrowed(table);
    }
    let mut legacy = table.legacy_style_override.clone().unwrap_or_default();
    legacy.flags = table
        .legacy_style_override
        .as_ref()
        .map_or(table.override_flag as i32, |s| s.flags);
    if let Some(base) = &table.base_style {
        // Legacy table-wide margins are symmetric. Do not average unequal edges
        // or serialize an unasserted modern default as an instance override.
        if base.sets(P::MARGIN_LEFT | P::MARGIN_RIGHT)
            && base.margin_left == base.margin_right
            && base.margin_left.is_finite()
            && base.margin_left >= 0.0
        {
            legacy.flags |= 0x8;
            legacy.horizontal_cell_margin = Some(base.margin_left);
        }
        if base.sets(P::MARGIN_TOP | P::MARGIN_BOTTOM)
            && base.margin_top == base.margin_bottom
            && base.margin_top.is_finite()
            && base.margin_top >= 0.0
        {
            legacy.flags |= 0x10;
            legacy.vertical_cell_margin = Some(base.margin_top);
        }
    }
    let title = !legacy.title_suppressed.unwrap_or(false);
    let header = !legacy.header_suppressed.unwrap_or(false);
    let mut heights = [None; 3];
    let mut old_index = 0;
    for (kind, id) in [3, 1, 2].into_iter().enumerate() {
        let bit = 0x100000 << kind;
        if legacy.flags & bit != 0 {
            heights[kind] = Some(legacy.row_heights.get(old_index).copied().unwrap_or(0.0));
            old_index += 1;
        }
        let mut value = None;
        let mut consistent = true;
        for (index, row) in table.rows.iter().enumerate() {
            let row_id = if row.style_id != 0 {
                row.style_id
            } else if title && index == 0 {
                1
            } else if header && index == usize::from(title) {
                2
            } else {
                3
            };
            if row_id != id {
                continue;
            }
            let style = row
                .style
                .as_ref()
                .filter(|s| s.sets(P::TEXT_HEIGHT))
                .or_else(|| table.base_style.as_ref().filter(|s| s.sets(P::TEXT_HEIGHT)));
            let Some(height) = style
                .map(|s| s.text_height)
                .filter(|v| v.is_finite() && *v > 0.0)
            else {
                consistent = false;
                break;
            };
            if value.is_some_and(|v| v != height) {
                consistent = false;
                break;
            }
            value = Some(height);
        }
        // A table-wide row-type override must not flatten distinct row formats.
        if consistent && value.is_some() {
            heights[kind] = value;
        }
    }
    legacy.flags &= !0x700000;
    legacy.row_heights.clear();
    for (kind, height) in heights.into_iter().enumerate() {
        if let Some(height) = height {
            legacy.flags |= 0x100000 << kind;
            legacy.row_heights.push(height);
        }
    }
    if table.legacy_style_override.as_ref() == Some(&legacy) {
        return Cow::Borrowed(table);
    }
    let mut result = table.clone();
    result.override_flag = legacy.flags != 0;
    result.legacy_style_override = Some(legacy);
    Cow::Owned(result)
}
