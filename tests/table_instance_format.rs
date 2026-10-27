//! Native and synthetic regression coverage for instance formatting conversion.
use acadrust::entities::table::{BorderPropertyFlags, CellEdgeFlags, CellStylePropertyFlags as P};
use acadrust::entities::{CellStyle, EntityType, Table, TableCell};
use acadrust::objects::{ObjectType, XRecord, XRecordEntry};
use acadrust::types::{Color, DxfVersion, Handle, LineWeight, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

fn written(doc: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(doc).unwrap()))
        .read()
        .unwrap()
}
fn table(doc: &CadDocument, handle: Handle) -> &Table {
    let Some(EntityType::Table(value)) = doc.get_entity(handle) else {
        panic!("table missing")
    };
    value
}
fn source() -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1024);
    let mut entity = Table::new(Vector3::ZERO, 3, 2);
    entity.table_style_handle = doc.objects.values().find_map(|object| match object {
        ObjectType::TableStyle(style) => Some(style.handle),
        _ => None,
    });
    for row in &mut entity.rows {
        for cell in &mut row.cells {
            *cell = TableCell::text("Original");
        }
    }
    let handle = doc.add_entity(EntityType::Table(Box::new(entity))).unwrap();
    let dictionary = doc.ensure_extension_dictionary(handle);
    let mut record = XRecord::new();
    let record_handle = doc.allocate_handle();
    record.handle = record_handle;
    record.owner = dictionary;
    record.reactors = vec![dictionary];
    let content_handle = doc.allocate_handle();
    record.entries.push(XRecordEntry::string(
        102,
        "ACAD_ROUNDTRIP_2008_TABLE_ENTITY",
    ));
    record
        .entries
        .push(XRecordEntry::handle(360, content_handle));
    let mut content = table(&doc, handle).clone();
    content.common.handle = content_handle;
    content.common.owner_handle = record_handle;
    let mut base = CellStyle::new();
    base.margin_left = 4.5;
    base.margin_right = 4.5;
    base.margin_top = 3.0;
    base.margin_bottom = 3.0;
    base.margin_override_flags = 1;
    base.override_flags =
        (P::MARGIN_LEFT | P::MARGIN_RIGHT | P::MARGIN_TOP | P::MARGIN_BOTTOM).bits() as i32;
    content.base_style = Some(base);
    for (i, row) in content.rows.iter_mut().enumerate() {
        row.style_id = (i + 1) as i32;
        let mut style = CellStyle::new();
        style.text_height = [6.0, 4.0, 2.5][i];
        style.override_flags = P::TEXT_HEIGHT.bits() as i32;
        style.content_format_override_flags = style.override_flags;
        row.style = Some(style);
    }
    doc.objects
        .insert(content_handle, ObjectType::TableContent(content));
    doc.objects
        .insert(record_handle, ObjectType::XRecord(record));
    let Some(ObjectType::Dictionary(entries)) = doc.objects.get_mut(&dictionary) else {
        panic!()
    };
    entries.add_entry("ACAD_XREC_ROUNDTRIP", record_handle);
    (doc, handle, content_handle)
}

#[test]
fn owned_content_supplies_format_without_overwriting_values_or_source() {
    let (mut doc, handle, _) = source();
    let Some(EntityType::Table(entity)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    entity.rows[1].cells[0].set_text("Edited entity value");
    entity.columns[0].width = 37.0;
    let original = table(&doc, handle).clone();
    let back = written(&doc);
    let result = table(&back, handle);
    assert_eq!(table(&doc, handle), &original);
    assert_eq!(result.rows[1].cells[0].text_value(), "Edited entity value");
    assert_eq!(result.columns[0].width, 37.0);
    let base = result.base_style.as_ref().unwrap();
    assert_eq!((base.margin_left, base.margin_top), (4.5, 3.0));
    assert_eq!(
        result
            .rows
            .iter()
            .map(|r| r.style.as_ref().unwrap().text_height)
            .collect::<Vec<_>>(),
        vec![6.0, 4.0, 2.5]
    );
}

#[test]
fn explicit_legacy_edits_override_older_companion_format() {
    let (mut doc, handle, _) = source();
    let Some(EntityType::Table(value)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    value.legacy_style_override = Some(acadrust::entities::table::LegacyTableStyleOverride {
        // Horizontal/vertical margins, data color, header alignment, title height.
        flags: 0x18 | 0x20 | 0x10000 | 0x200000,
        horizontal_cell_margin: Some(7.0),
        vertical_cell_margin: Some(5.0),
        row_colors: vec![Color::from_index(5)],
        row_alignments: vec![7],
        row_heights: vec![8.0],
        ..Default::default()
    });
    let back = written(&doc);
    let result = table(&back, handle);
    let base = result.base_style.as_ref().unwrap();
    assert_eq!((base.margin_left, base.margin_right), (7.0, 7.0));
    assert_eq!((base.margin_top, base.margin_bottom), (5.0, 5.0));
    let styles = result
        .rows
        .iter()
        .map(|r| r.style.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        styles.iter().map(|s| s.text_height).collect::<Vec<_>>(),
        vec![8.0, 4.0, 2.5]
    );
    assert_eq!(styles[1].alignment, 7);
    assert_eq!(styles[2].content_color, Color::from_index(5));
}

#[test]
fn broken_ownership_and_layout_do_not_supply_foreign_format() {
    for fault in 0..4 {
        let (mut doc, handle, content) = source();
        if let Some(ObjectType::TableContent(value)) = doc.objects.get_mut(&content) {
            match fault {
                0 => value.common.owner_handle = Handle::new(0xdead),
                1 => {
                    value.rows.pop();
                }
                2 => value.table_style_handle = Some(Handle::new(0xbeef)),
                _ => {}
            }
        }
        if fault == 3 {
            let dictionary = doc.extension_dictionary_handle(handle).unwrap();
            let Some(ObjectType::Dictionary(value)) = doc.objects.get_mut(&dictionary) else {
                panic!()
            };
            value.owner = Handle::new(0xdead);
        }
        assert!(
            table(&written(&doc), handle).base_style.is_none(),
            "fault {fault}"
        );
    }
}

#[test]
fn modern_instance_format_wins_over_a_retained_older_companion() {
    let (mut doc, handle, _) = source();
    let Some(EntityType::Table(value)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    let mut base = CellStyle::new();
    base.margin_left = 9.0;
    base.margin_override_flags = 1;
    base.override_flags = P::MARGIN_LEFT.bits() as i32;
    value.base_style = Some(base);
    assert_eq!(
        table(&written(&doc), handle)
            .base_style
            .as_ref()
            .unwrap()
            .margin_left,
        9.0
    );
}

#[test]
fn right_edge_has_file_grid_index_32_not_inside_horizontal_index_2() {
    let mut doc = CadDocument::new();
    let mut entity = Table::new(Vector3::ZERO, 1, 1);
    let mut style = CellStyle::new();
    style.applied_border_edges = CellEdgeFlags::RIGHT | CellEdgeFlags::INSIDE_HORIZONTAL;
    style.right_border.color = Color::from_index(1);
    style.right_border.line_weight = LineWeight::from_value(50);
    style.right_border.override_flags =
        BorderPropertyFlags::COLOR | BorderPropertyFlags::LINE_WEIGHT;
    let mut inside = style.right_border.clone();
    inside.color = Color::from_index(3);
    style.additional_borders.push((2, inside));
    entity.rows[0].cells[0].style = Some(style);
    let handle = doc.add_entity(EntityType::Table(Box::new(entity))).unwrap();
    let back = written(&doc);
    let style = table(&back, handle).rows[0].cells[0]
        .style
        .as_ref()
        .unwrap();
    assert_eq!(style.right_border.color, Color::from_index(1));
    assert_eq!(style.additional_borders[0].0, 2);
    assert_eq!(style.additional_borders[0].1.color, Color::from_index(3));
    let mut back = back;
    let mut content = table(&back, handle).clone();
    content.common.handle = back.allocate_handle();
    back.objects
        .insert(content.common.handle, ObjectType::TableContent(content));
    let mut bytes = Vec::new();
    acadrust::DxfWriter::new(&back)
        .write_to_writer(&mut bytes)
        .unwrap();
    // The complete format includes both distinct native grid indexes.
    let text = String::from_utf8(bytes).unwrap();
    let pairs = text.lines().map(str::trim).collect::<Vec<_>>();
    assert!(pairs.windows(2).any(|p| p == ["95", "32"]));
    assert!(pairs.windows(2).any(|p| p == ["95", "2"]));
}

#[test]
#[ignore = "requires ARQ_NATIVE_TABLE_ROOT with the original ProgeCAD sources"]
fn native_dxf_to_dwg_preserves_instance_margins_rows_and_right_border() {
    let root = std::env::var("ARQ_NATIVE_TABLE_ROOT").unwrap();
    let source = DxfReader::from_file(format!("{root}/tables-native-ac1024.dxf"))
        .unwrap()
        .read()
        .unwrap();
    let native = DwgReader::from_file(format!("{root}/tables-native-ac1032.dwg"))
        .unwrap()
        .read()
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let back = DwgReader::from_stream(Cursor::new(bytes.clone()))
        .read()
        .unwrap();
    let mut checked = 0;
    for entity in source.entities() {
        let EntityType::Table(before) = entity else {
            continue;
        };
        checked += 1;
        let actual = table(&back, before.common.handle);
        let expected = table(&native, before.common.handle);
        assert_eq!(actual.base_style, expected.base_style);
        for (a, b) in actual.rows.iter().zip(&expected.rows) {
            assert_eq!(a.style, b.style);
            assert!((a.height - b.height).abs() < 1e-10);
        }
        assert_eq!(
            actual.columns.iter().map(|c| c.width).collect::<Vec<_>>(),
            expected.columns.iter().map(|c| c.width).collect::<Vec<_>>()
        );
        assert_eq!(actual.merged_ranges, expected.merged_ranges);
        if before.insertion_point.x == 480.0 {
            for cell in [6, 7] {
                let right = &actual.rows[cell / 3].cells[cell % 3]
                    .style
                    .as_ref()
                    .unwrap()
                    .right_border;
                assert_eq!(right.color, Color::from_index(1));
                assert_eq!(right.line_weight, LineWeight::from_value(50));
            }
        }
    }
    assert_eq!(checked, 4);
    if let Some(path) = std::env::var_os("ARQ_TABLE_FORMAT_OUTPUT") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut file, &bytes).unwrap();
    }
}
