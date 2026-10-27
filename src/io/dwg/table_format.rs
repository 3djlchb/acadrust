//! Complete a legacy entity's format from its owned native TABLECONTENT.
//! Values and geometry stay with the entity; the auxiliary object supplies only
//! format fields absent from the legacy projection. Never mutate the caller.
use crate::entities::table::{CellStylePropertyFlags as P, LegacyTableStyleOverride};
use crate::{entities::Table, objects::ObjectType, CadDocument};
use std::borrow::Cow;

fn companion<'a>(table: &Table, doc: &'a CadDocument) -> Option<&'a Table> {
    let dictionary = doc.extension_dictionary_handle(table.common.handle)?;
    let ObjectType::Dictionary(dictionary) = doc.objects.get(&dictionary)? else {
        return None;
    };
    if dictionary.owner != table.common.handle {
        return None;
    }
    let record = dictionary.get("ACAD_XREC_ROUNDTRIP")?;
    let ObjectType::XRecord(record) = doc.objects.get(&record)? else {
        return None;
    };
    if record.owner != dictionary.handle || record.reactors.iter().any(|r| *r != dictionary.handle)
    {
        return None;
    }
    let mut section = false;
    let mut content = None;
    for entry in &record.entries {
        if entry.code == 102 {
            section = entry.value.as_string() == Some("ACAD_ROUNDTRIP_2008_TABLE_ENTITY");
        } else if section && entry.code == 360 {
            if content.is_some() {
                return None;
            }
            content = entry.value.as_handle();
        }
    }
    let ObjectType::TableContent(content) = doc.objects.get(&content?)? else {
        return None;
    };
    (content.common.owner_handle == record.handle
        && content.table_style_handle == table.table_style_handle
        && content.rows.len() == table.rows.len()
        && content.columns.len() == table.columns.len())
    .then_some(content)
}

fn selected<T>(values: &[T], flags: i32, first: i32, kind: usize) -> Option<&T> {
    let bit = first << kind;
    (flags & bit != 0)
        .then(|| values.get((flags & (first * 7) & (bit - 1)).count_ones() as usize))
        .flatten()
}

fn row_override(
    style: &mut crate::entities::CellStyle,
    legacy: &LegacyTableStyleOverride,
    kind: usize,
) {
    if let Some(value) = selected(&legacy.row_heights, legacy.flags, 0x100000, kind) {
        style.text_height = *value;
        style.override_flags |= P::TEXT_HEIGHT.bits() as i32;
        style.content_format_override_flags |= P::TEXT_HEIGHT.bits() as i32;
    }
    if let Some(value) = selected(&legacy.row_alignments, legacy.flags, 0x4000, kind) {
        style.alignment = i32::from(*value);
        style.override_flags |= P::ALIGNMENT.bits() as i32;
        style.content_format_override_flags |= P::ALIGNMENT.bits() as i32;
    }
    if let Some(value) = selected(&legacy.row_colors, legacy.flags, 0x20, kind) {
        style.content_color = *value;
        style.override_flags |= P::CONTENT_COLOR.bits() as i32;
        style.content_format_override_flags |= P::CONTENT_COLOR.bits() as i32;
    }
    if let Some(value) = selected(&legacy.text_style_names, legacy.flags, 0x20000, kind) {
        style.text_style_name = value.clone();
        style.text_style_handle = None;
        style.override_flags |= P::TEXT_STYLE.bits() as i32;
        style.content_format_override_flags |= P::TEXT_STYLE.bits() as i32;
    }
    if let Some(value) = selected(&legacy.text_style_handles, legacy.flags, 0x20000, kind) {
        style.text_style_handle = Some(*value);
        style.override_flags |= P::TEXT_STYLE.bits() as i32;
        style.content_format_override_flags |= P::TEXT_STYLE.bits() as i32;
    }
}

pub(super) fn with_companion_format<'a>(table: &'a Table, doc: &CadDocument) -> Cow<'a, Table> {
    // Modern entities already own their authoritative format. A retained,
    // possibly older round-trip object must not overwrite it after edits.
    if table.base_style.is_some() || table.rows.iter().any(|row| row.style.is_some()) {
        return Cow::Borrowed(table);
    }
    let Some(content) = companion(table, doc) else {
        return Cow::Borrowed(table);
    };
    let mut result = table.clone();
    result.base_style = content.base_style.clone();
    let legacy = table.legacy_style_override.clone().unwrap_or_default();
    if let Some(base) = result.base_style.as_mut() {
        if let Some(value) = legacy.horizontal_cell_margin {
            base.margin_left = value;
            base.margin_right = value;
            base.margin_override_flags |= 1;
            base.override_flags |= (P::MARGIN_LEFT | P::MARGIN_RIGHT).bits() as i32;
        }
        if let Some(value) = legacy.vertical_cell_margin {
            base.margin_top = value;
            base.margin_bottom = value;
            base.margin_override_flags |= 1;
            base.override_flags |= (P::MARGIN_TOP | P::MARGIN_BOTTOM).bits() as i32;
        }
    }
    let title = !legacy.title_suppressed.unwrap_or(false);
    let header = !legacy.header_suppressed.unwrap_or(false);
    for (index, row) in result.rows.iter_mut().enumerate() {
        let id = if row.style_id != 0 {
            row.style_id
        } else if title && index == 0 {
            1
        } else if header && index == usize::from(title) {
            2
        } else {
            3
        };
        let from = content
            .rows
            .get(index)
            .filter(|r| r.style_id == id)
            .or_else(|| content.rows.iter().find(|r| r.style_id == id));
        row.style = from.and_then(|r| r.style.clone());
        row.style_id = id;
        if let Some(style) = row.style.as_mut() {
            row_override(
                style,
                &legacy,
                match id {
                    1 => 1,
                    2 => 2,
                    _ => 0,
                },
            );
        }
    }
    for (column, from) in result.columns.iter_mut().zip(&content.columns) {
        if column.style.is_none() {
            column.style = from.style.clone();
        }
    }
    Cow::Owned(result)
}
