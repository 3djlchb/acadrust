//! Modern DWG table formatting must survive the legacy DXF projection.
use acadrust::entities::table::{CellStylePropertyFlags as P, LegacyTableStyleOverride};
use acadrust::entities::{CellStyle, EntityType, Table};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DxfReader, DxfWriter};
use std::io::Cursor;

fn source() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut table = Table::new(Vector3::ZERO, 5, 3);
    let mut base = CellStyle::new();
    base.margin_left = 4.5;
    base.margin_right = 4.5;
    base.margin_top = 3.0;
    base.margin_bottom = 3.0;
    base.margin_override_flags = 1;
    base.override_flags =
        (P::MARGIN_LEFT | P::MARGIN_RIGHT | P::MARGIN_TOP | P::MARGIN_BOTTOM).bits() as i32;
    table.base_style = Some(base);
    for (index, row) in table.rows.iter_mut().enumerate() {
        row.style_id = if index == 0 {
            1
        } else if index == 1 {
            2
        } else {
            3
        };
        let mut style = CellStyle::new();
        style.text_height = if index == 0 {
            6.0
        } else if index == 1 {
            4.0
        } else {
            2.5
        };
        style.override_flags = P::TEXT_HEIGHT.bits() as i32;
        style.content_format_override_flags = P::TEXT_HEIGHT.bits() as i32;
        row.style = Some(style);
    }
    doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    doc
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let mut bytes = Vec::new();
    DxfWriter::new(doc).write_to_writer(&mut bytes).unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn formatting(doc: &CadDocument) -> &LegacyTableStyleOverride {
    let EntityType::Table(table) = doc
        .entities()
        .find(|e| matches!(e, EntityType::Table(_)))
        .unwrap()
    else {
        unreachable!()
    };
    table
        .legacy_style_override
        .as_ref()
        .expect("missing legacy instance format")
}

#[test]
fn modern_base_margins_are_projected_into_dxf_without_mutating_source() {
    let doc = source();
    let before = doc
        .entities()
        .find(|e| matches!(e, EntityType::Table(_)))
        .unwrap()
        .clone();
    let back = roundtrip(&doc);
    let style = formatting(&back);
    assert_eq!(style.horizontal_cell_margin, Some(4.5));
    assert_eq!(style.vertical_cell_margin, Some(3.0));
    assert_eq!(
        doc.entities()
            .find(|e| matches!(e, EntityType::Table(_)))
            .unwrap(),
        &before
    );
}

#[test]
fn modern_row_type_text_heights_are_projected_in_data_title_header_order() {
    let back = roundtrip(&source());
    let style = formatting(&back);
    assert_eq!(style.flags & 0x700000, 0x700000);
    assert_eq!(style.row_heights, [2.5, 6.0, 4.0]);
}

#[test]
#[ignore = "requires ARQ_NATIVE_TABLE_ROOT with original ProgeCAD sources"]
fn native_dwg_exports_instance_margins_and_row_heights() {
    let root = std::env::var("ARQ_NATIVE_TABLE_ROOT").unwrap();
    let doc = DwgReader::from_file(format!("{root}/tables-native-ac1032.dwg"))
        .unwrap()
        .read()
        .unwrap();
    if let Some(path) = std::env::var_os("ARQ_TABLE_DXF_OUTPUT") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        DxfWriter::new(&doc).write_to_writer(&mut file).unwrap();
    }
    let back = roundtrip(&doc);
    let mut checked = 0;
    for entity in back.entities() {
        let EntityType::Table(t) = entity else {
            continue;
        };
        checked += 1;
        let (h, v, text) = match t.insertion_point.x as i32 {
            160 => (4.5, 3.0, 3.0),
            480 => (2.0, 2.0, 4.0),
            _ => (1.5, 1.5, 2.5),
        };
        let style = t.legacy_style_override.as_ref().unwrap();
        assert_eq!(style.horizontal_cell_margin, Some(h));
        assert_eq!(style.vertical_cell_margin, Some(v));
        assert_eq!(style.row_heights, [text, text, text]);
    }
    assert_eq!(checked, 4);
}

#[test]
fn modern_asserted_format_wins_stale_legacy_projection() {
    let mut doc = source();
    let handle = doc
        .entities()
        .find(|e| matches!(e, EntityType::Table(_)))
        .unwrap()
        .common()
        .handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    t.legacy_style_override = Some(LegacyTableStyleOverride {
        flags: 0x700018,
        horizontal_cell_margin: Some(0.06),
        vertical_cell_margin: Some(0.06),
        row_heights: vec![0.18; 3],
        ..Default::default()
    });
    let back = roundtrip(&doc);
    let s = formatting(&back);
    assert_eq!(s.horizontal_cell_margin, Some(4.5));
    assert_eq!(s.vertical_cell_margin, Some(3.0));
    assert_eq!(s.row_heights, [2.5, 6.0, 4.0]);
}

#[test]
fn unequal_margins_and_distinct_data_row_heights_are_not_flattened() {
    let mut doc = source();
    let handle = doc
        .entities()
        .find(|e| matches!(e, EntityType::Table(_)))
        .unwrap()
        .common()
        .handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    t.base_style.as_mut().unwrap().margin_right = 7.0;
    t.rows[3].style.as_mut().unwrap().text_height = 3.5;
    let back = roundtrip(&doc);
    let s = formatting(&back);
    assert_eq!(s.horizontal_cell_margin, None);
    assert_eq!(s.vertical_cell_margin, Some(3.0));
    assert_eq!(s.flags & 0x100000, 0);
    assert_eq!(s.row_heights, [6.0, 4.0]);
}

#[test]
fn legacy_only_format_is_unchanged() {
    let mut doc = source();
    let handle = doc
        .entities()
        .find(|e| matches!(e, EntityType::Table(_)))
        .unwrap()
        .common()
        .handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    t.base_style = None;
    for row in &mut t.rows {
        row.style = None;
    }
    let legacy = LegacyTableStyleOverride {
        flags: 0x200018,
        horizontal_cell_margin: Some(0.0),
        vertical_cell_margin: Some(5.0),
        row_heights: vec![8.0],
        ..Default::default()
    };
    t.legacy_style_override = Some(legacy.clone());
    assert_eq!(formatting(&roundtrip(&doc)), &legacy);
}

#[test]
fn unasserted_modern_defaults_do_not_become_global_overrides() {
    let mut doc = source();
    let handle = doc.entities().next().unwrap().common().handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    t.base_style = Some(CellStyle::new());
    for row in &mut t.rows {
        row.style = Some(CellStyle::new());
    }
    let back = roundtrip(&doc);
    let EntityType::Table(table) = back.entities().next().unwrap() else {
        panic!()
    };
    assert!(table.legacy_style_override.as_ref().is_none_or(|style| {
        style.flags & 0x700018 == 0
            && style.horizontal_cell_margin.is_none()
            && style.vertical_cell_margin.is_none()
            && style.row_heights.is_empty()
    }));
}

#[test]
fn absent_title_uses_header_then_data_when_style_ids_are_unspecified() {
    let mut doc = source();
    let handle = doc.entities().next().unwrap().common().handle;
    let Some(EntityType::Table(t)) = doc.get_entity_mut(handle) else {
        panic!()
    };
    t.rows.remove(0);
    for row in &mut t.rows {
        row.style_id = 0;
    }
    t.legacy_style_override = Some(LegacyTableStyleOverride {
        flags: 1,
        title_suppressed: Some(true),
        ..Default::default()
    });
    let back = roundtrip(&doc);
    let style = formatting(&back);
    assert_eq!(style.flags & 0x700000, 0x500000);
    assert_eq!(style.row_heights, [2.5, 4.0]);
    assert_eq!(style.flags & 1, 1);
}

#[test]
fn native_style_companion_has_checked_ownership_and_preserves_complete_format() {
    let doc = source();
    let original_objects = doc.objects.len();
    let handle = doc.entities().next().unwrap().common().handle;
    let back = roundtrip(&doc);
    assert_eq!(doc.objects.len(), original_objects);
    assert_eq!(doc.extension_dictionary_handle(handle), None);
    let dict_handle = back.extension_dictionary_handle(handle).unwrap();
    let Some(acadrust::objects::ObjectType::Dictionary(dict)) = back.objects.get(&dict_handle)
    else {
        panic!()
    };
    assert_eq!(dict.owner, handle);
    let record_handle = dict.get("ACAD_XREC_ROUNDTRIP").unwrap();
    let Some(acadrust::objects::ObjectType::XRecord(record)) = back.objects.get(&record_handle)
    else {
        panic!()
    };
    assert_eq!(record.owner, dict_handle);
    assert_eq!(record.reactors, [dict_handle]);
    let content_handle = record
        .entries
        .iter()
        .find(|e| e.code == 360)
        .unwrap()
        .value
        .as_handle()
        .unwrap();
    let Some(acadrust::objects::ObjectType::TableContent(content)) =
        back.objects.get(&content_handle)
    else {
        panic!()
    };
    assert_eq!(content.common.owner_handle, record_handle);
    assert_eq!(content.base_style.as_ref().unwrap().margin_left, 4.5);
    assert_eq!(
        content
            .rows
            .iter()
            .map(|r| r.style.as_ref().unwrap().text_height)
            .collect::<Vec<_>>(),
        [6.0, 4.0, 2.5, 2.5, 2.5]
    );
    let again = roundtrip(&back);
    assert_eq!(
        again
            .objects
            .values()
            .filter(|o| matches!(o, acadrust::objects::ObjectType::TableContent(_)))
            .count(),
        1
    );
}

#[test]
fn ascii_and_binary_dxf_preserve_the_same_complete_instance_format() {
    let doc = source();
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        for binary in [false, true] {
            let mut writer = DxfWriter::new(&doc).with_version(version);
            writer.set_binary(binary);
            let back = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
                .unwrap()
                .read()
                .unwrap();
            let content = back
                .objects
                .values()
                .find_map(|o| match o {
                    acadrust::objects::ObjectType::TableContent(t) => Some(t),
                    _ => None,
                })
                .unwrap();
            assert_eq!(content.base_style.as_ref().unwrap().margin_left, 4.5);
            assert_eq!(content.rows[0].style.as_ref().unwrap().text_height, 6.0);
            assert_eq!(formatting(&back).row_heights, [2.5, 6.0, 4.0]);
        }
    }
}

#[test]
fn existing_roundtrip_record_is_not_replaced() {
    let mut doc = source();
    let handle = doc.entities().next().unwrap().common().handle;
    let record = doc.ensure_xrecord(handle, "ACAD_XREC_ROUNDTRIP");
    let Some(acadrust::objects::ObjectType::XRecord(r)) = doc.objects.get_mut(&record) else {
        panic!()
    };
    r.entries.push(acadrust::objects::XRecordEntry::string(
        1,
        "Preserve vendor data",
    ));
    let back = roundtrip(&doc);
    assert!(!back
        .objects
        .values()
        .any(|o| matches!(o, acadrust::objects::ObjectType::TableContent(_))));
    let Some(acadrust::objects::ObjectType::XRecord(r)) = back.objects.get(&record) else {
        panic!()
    };
    assert_eq!(r.entries[0].value.as_string(), Some("Preserve vendor data"));
}

#[test]
fn new_content_does_not_inherit_entity_reactors() {
    let mut doc = source();
    let handle = doc.entities().next().unwrap().common().handle;
    doc.get_entity_mut(handle)
        .unwrap()
        .common_mut()
        .reactors
        .push(handle);
    let back = roundtrip(&doc);
    assert_eq!(back.get_entity(handle).unwrap().common().reactors, [handle]);
    let content = back
        .objects
        .values()
        .find_map(|o| match o {
            acadrust::objects::ObjectType::TableContent(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert!(content.common.reactors.is_empty());
}
