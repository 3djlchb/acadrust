//! Modern per-cell format must not be interpreted as legacy override bits.
use acadrust::entities::table::{
    BorderPropertyFlags as B, CellEdgeFlags, CellStylePropertyFlags as P,
};
use acadrust::entities::{CellStyle, EntityType, Table};
use acadrust::types::{Color, DxfVersion, LineWeight, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn source(style: CellStyle) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut table = Table::new(Vector3::ZERO, 3, 2);
    table.base_style = Some(CellStyle::new());
    for row in &mut table.rows {
        let mut s = CellStyle::new();
        s.override_flags = P::TEXT_HEIGHT.bits() as i32;
        s.content_format_override_flags = P::TEXT_HEIGHT.bits() as i32;
        s.text_height = 4.0;
        row.style = Some(s);
        for cell in &mut row.cells {
            cell.set_text("Cell value");
        }
    }
    table.rows[2].cells[0].style = Some(style);
    doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    doc
}
fn output(doc: &CadDocument) -> Vec<u8> {
    let mut bytes = Vec::new();
    DxfWriter::new(doc).write_to_writer(&mut bytes).unwrap();
    bytes
}
fn projected(doc: &CadDocument) -> Table {
    let back = DxfReader::from_reader(Cursor::new(output(doc)))
        .unwrap()
        .read()
        .unwrap();
    let result = back
        .entities()
        .find_map(|e| {
            if let EntityType::Table(t) = e {
                Some(t.as_ref().clone())
            } else {
                None
            }
        })
        .unwrap();
    result
}
#[test]
fn modern_cell_text_height_uses_the_legacy_height_bit() {
    let mut s = CellStyle::new();
    s.override_flags = P::TEXT_HEIGHT.bits() as i32;
    s.text_height = 7.0;
    let t = projected(&source(s));
    let cell = &t.rows[2].cells[0];
    let style = cell.style.as_ref().expect("missing explicit height");
    assert!(style.sets(P::TEXT_HEIGHT));
    assert_eq!(style.text_height, 7.0);
}
#[test]
fn modern_content_color_does_not_assert_the_default_text_height() {
    let mut s = CellStyle::new();
    s.override_flags = P::CONTENT_COLOR.bits() as i32;
    s.content_color = Color::from_index(1);
    let t = projected(&source(s));
    let style = t.rows[2].cells[0].style.as_ref().unwrap();
    assert_eq!(style.content_color, Color::from_index(1));
    assert_eq!(
        style.text_height, 4.0,
        "the derived cache keeps the inherited row height, not 0.18"
    );
}
#[test]
fn modern_right_border_projects_color_weight_and_visibility_independently() {
    let mut s = CellStyle::new();
    s.applied_border_edges = CellEdgeFlags::RIGHT;
    s.right_border.override_flags = B::COLOR | B::LINE_WEIGHT | B::INVISIBILITY;
    s.right_border.color = Color::from_index(1);
    s.right_border.line_weight = LineWeight::from_value(50);
    s.right_border.invisible = true;
    let t = projected(&source(s));
    let style = t.rows[2].cells[0].style.as_ref().unwrap();
    assert_eq!(style.right_border.color, Color::from_index(1));
    assert_eq!(style.right_border.line_weight, LineWeight::from_value(50));
    assert!(style.right_border.invisible);
    assert_eq!(
        style.override_flags & 0x3ffc0,
        0x8880,
        "only asserted right-edge properties belong in the legacy projection"
    );
}
#[test]
fn value_cells_assert_the_legacy_value_present_bit() {
    let bytes = output(&source(CellStyle::new()));
    let text = String::from_utf8(bytes).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    let pairs = lines
        .chunks_exact(2)
        .map(|x| (x[0].trim().parse::<i32>().unwrap(), x[1]))
        .collect::<Vec<_>>();
    let start = pairs
        .iter()
        .position(|x| x.0 == 100 && x.1 == "AcDbTable")
        .unwrap();
    let mut cells = 0;
    let mut in_cell = false;
    for (code, value) in &pairs[start + 1..] {
        if *code == 0 {
            break;
        }
        if *code == 171 {
            in_cell = true;
        }
        if in_cell && *code == 91 {
            assert_ne!(
                value.trim().parse::<i32>().unwrap() & 0x40000,
                0,
                "modern values need the legacy value-present flag"
            );
            cells += 1;
            in_cell = false;
        }
    }
    assert_eq!(cells, 6);
}

#[test]
fn modern_visibility_only_does_not_assert_a_line_weight_or_other_edges() {
    let mut s = CellStyle::new();
    s.applied_border_edges = CellEdgeFlags::RIGHT;
    s.right_border.override_flags = B::INVISIBILITY;
    s.right_border.invisible = true;
    let t = projected(&source(s));
    let style = t.rows[2].cells[0].style.as_ref().unwrap();
    assert_eq!(style.override_flags & 0x3ffc0, 0x8000);
    assert!(style.right_border.invisible);
}
#[test]
fn unasserted_modern_defaults_do_not_become_cell_overrides() {
    let mut doc = source(CellStyle::new());
    let handle = doc.entities().next().unwrap().common().handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    for row in &mut t.rows {
        row.style = None;
    }
    let t = projected(&doc);
    assert!(t.rows[2].cells[0].style.is_none());
}
#[test]
fn cell_projection_does_not_mutate_the_modern_source() {
    let mut s = CellStyle::new();
    s.override_flags = P::CONTENT_COLOR.bits() as i32;
    s.content_color = Color::from_index(1);
    let doc = source(s);
    let before = doc.entities().next().unwrap().clone();
    output(&doc);
    assert_eq!(doc.entities().next().unwrap(), &before);
}

#[test]
fn cell_overrides_precede_the_modern_cell_value_block() {
    let mut s = CellStyle::new();
    s.override_flags = P::TEXT_HEIGHT.bits() as i32;
    s.text_height = 7.0;
    let text = String::from_utf8(output(&source(s))).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    let pairs = lines
        .chunks_exact(2)
        .map(|x| (x[0].trim().parse::<i32>().unwrap(), x[1]))
        .collect::<Vec<_>>();
    let start = pairs
        .iter()
        .position(|x| x.0 == 100 && x.1 == "AcDbTable")
        .unwrap();
    let cell = pairs[start..]
        .iter()
        .enumerate()
        .filter(|(_, x)| x.0 == 171)
        .nth(4)
        .unwrap()
        .0
        + start;
    let end = pairs[cell + 1..]
        .iter()
        .position(|x| x.0 == 171 || x.0 == 0)
        .unwrap()
        + cell
        + 1;
    let data = &pairs[cell..end];
    let height = data.iter().position(|x| x.0 == 140).unwrap();
    let value = data
        .iter()
        .position(|x| x.0 == 301 && x.1 == "CELL_VALUE")
        .unwrap();
    assert!(
        height < value,
        "native readers expect cell overrides before CELL_VALUE"
    );
}

#[test]
fn derived_cell_height_is_projected_without_duplicate_global_overrides() {
    let t = projected(&source(CellStyle::new()));
    assert_eq!(
        t.rows[2].cells[0]
            .style
            .as_ref()
            .expect("missing effective height")
            .text_height,
        4.0
    );
    assert!(t
        .legacy_style_override
        .as_ref()
        .is_none_or(|s| s.flags & 0x700018 == 0));
}
#[test]
fn a_shared_border_is_written_on_both_cells_with_one_real_edge() {
    let mut s = CellStyle::new();
    s.applied_border_edges = CellEdgeFlags::RIGHT;
    s.right_border.color = Color::from_index(1);
    s.right_border.line_weight = LineWeight::from_value(50);
    s.right_border.override_flags = B::COLOR | B::LINE_WEIGHT;
    let t = projected(&source(s));
    let left = &t.rows[2].cells[0];
    let right = &t.rows[2].cells[1];
    assert_eq!(left.virtual_edge & CellEdgeFlags::RIGHT.bits() as i16, 0);
    assert_ne!(right.virtual_edge & CellEdgeFlags::LEFT.bits() as i16, 0);
    assert_eq!(
        left.edge_flags & CellEdgeFlags::RIGHT.bits() as u8,
        CellEdgeFlags::RIGHT.bits() as u8
    );
    assert_eq!(
        right.edge_flags & CellEdgeFlags::LEFT.bits() as u8,
        CellEdgeFlags::LEFT.bits() as u8
    );
}

#[test]
fn complete_companion_keeps_the_original_cell_inheritance() {
    let mut style = CellStyle::new();
    style.override_flags = P::CONTENT_COLOR.bits() as i32;
    style.content_color = Color::from_index(1);
    let doc = source(style);
    let back = DxfReader::from_reader(Cursor::new(output(&doc)))
        .unwrap()
        .read()
        .unwrap();
    let content = back
        .objects
        .values()
        .find_map(|o| {
            if let acadrust::objects::ObjectType::TableContent(t) = o {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    let original = content.rows[2].cells[0].style.as_ref().unwrap();
    assert!(!original.sets(P::TEXT_HEIGHT));
    assert_eq!(content.rows[2].style.as_ref().unwrap().text_height, 4.0);
}
#[test]
fn conflicting_shared_border_overrides_are_not_flattened() {
    let mut style = CellStyle::new();
    style.applied_border_edges = CellEdgeFlags::RIGHT;
    style.right_border.color = Color::from_index(1);
    style.right_border.override_flags = B::COLOR;
    let mut doc = source(style);
    let handle = doc.entities().next().unwrap().common().handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    let mut other = CellStyle::new();
    other.applied_border_edges = CellEdgeFlags::LEFT;
    other.left_border.color = Color::from_index(3);
    other.left_border.override_flags = B::COLOR;
    t.rows[2].cells[1].style = Some(other);
    let t = projected(&doc);
    assert_eq!(
        t.rows[2].cells[0]
            .style
            .as_ref()
            .unwrap()
            .right_border
            .color,
        Color::from_index(1)
    );
    assert_eq!(
        t.rows[2].cells[1].style.as_ref().unwrap().left_border.color,
        Color::from_index(3)
    );
    assert_eq!(
        t.rows[2].cells[1].virtual_edge & CellEdgeFlags::LEFT.bits() as i16,
        0
    );
}
#[test]
fn binary_and_ascii_keep_the_same_derived_cell_cache() {
    let mut s = CellStyle::new();
    s.applied_border_edges = CellEdgeFlags::RIGHT;
    s.right_border.color = Color::from_index(1);
    s.right_border.override_flags = B::COLOR;
    let doc = source(s);
    let expected = projected(&doc);
    let mut writer = DxfWriter::new(&doc);
    writer.set_binary(true);
    let back = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
        .unwrap()
        .read()
        .unwrap();
    let table = back
        .entities()
        .find_map(|e| {
            if let EntityType::Table(t) = e {
                Some(t.as_ref().clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(table.rows, expected.rows);
}

#[test]
fn ambiguous_row_and_column_heights_are_not_flattened_into_the_cache() {
    let mut doc = source(CellStyle::new());
    let handle = doc.entities().next().unwrap().common().handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    let mut column = CellStyle::new();
    column.override_flags = P::TEXT_HEIGHT.bits() as i32;
    column.text_height = 7.0;
    t.columns[0].style = Some(column);
    let t = projected(&doc);
    assert!(t.rows[2].cells[0]
        .style
        .as_ref()
        .is_none_or(|s| !s.sets(P::TEXT_HEIGHT)));
}

#[test]
fn visibility_only_survives_the_dxf_to_dwg_boundary_without_a_weight_override() {
    let mut style = CellStyle::new();
    style.applied_border_edges = CellEdgeFlags::RIGHT;
    style.right_border.override_flags = B::INVISIBILITY;
    style.right_border.invisible = true;
    let doc = source(style);
    let dxf = DxfReader::from_reader(Cursor::new(output(&doc)))
        .unwrap()
        .read()
        .unwrap();
    let bytes = acadrust::DwgWriter::write_to_vec(&dxf).unwrap();
    let back = acadrust::DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap();
    let table = back
        .entities()
        .find_map(|e| {
            if let EntityType::Table(t) = e {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    let border = &table.rows[2].cells[0].style.as_ref().unwrap().right_border;
    assert!(border.invisible);
    assert!(border.override_flags.contains(B::INVISIBILITY));
    assert!(!border.override_flags.contains(B::LINE_WEIGHT));
}
