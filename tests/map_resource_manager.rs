//! Opaque Map application objects must retain their streams and references.
use acadrust::classes::DxfClass;
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{Dictionary, ObjectType, ProxyObject};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

const NAME: &str = "MAPRESOURCEMANAGEROBJECT";

fn manager(document: &CadDocument, handle: Handle) -> &ProxyObject {
    let ObjectType::ProxyObject(value) = &document.objects[&handle] else {
        panic!("expected opaque Map proxy: {:?}", document.objects[&handle]);
    };
    let class = document
        .classes
        .iter()
        .find(|class| i32::from(class.class_number) == value.class_id)
        .unwrap_or_else(|| {
            panic!(
                "missing proxy class {} in {:?}",
                value.class_id, document.classes
            )
        });
    assert_eq!(class.dxf_name, NAME);
    assert_eq!(class.cpp_class_name, "AcMapResourceManager");
    value
}

fn synthetic(version: DxfVersion, zombie: bool) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    document.dwg_source_version = Some(version);
    document.classes.clear();
    for number in 500..540 {
        let mut reserved = DxfClass::new(
            &format!("TEST_RESERVED_{number}"),
            &format!("TestReserved{number}"),
        );
        reserved.class_number = number;
        document.classes.push_preserving(reserved);
    }
    let mut class = DxfClass::new(NAME, "AcMapResourceManager");
    class.application_name = "AcMapSharedComponents".into();
    class.was_zombie = zombie;
    class.class_number = 540;
    document.classes.push_preserving(class);
    let owner = document.header.named_objects_dict_handle;
    let handle = document.allocate_handle();
    let extension = document.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = extension;
    dictionary.owner = handle;
    document
        .objects
        .insert(extension, ObjectType::Dictionary(dictionary));
    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(version).unwrap(), version);
    writer.write_object_type(540);
    if version < DxfVersion::AC1024 {
        writer.save_position_for_size();
    }
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0); // No EED.
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    writer.write_bit_long(1);
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    writer.write_bit(false);
    writer.write_handle(DwgReferenceType::HardOwnership, extension.value());
    if version >= DxfVersion::AC1027 {
        writer.write_bit(false);
    }
    // Deliberately non-byte-aligned application payload, with a text stream
    // and an object reference beyond the common owner/reactor/xdictionary.
    for byte in [0x40u8, 0x61, 0xa5] {
        for bit in 0..8 {
            writer.write_bit(byte & (0x80 >> bit) != 0);
        }
    }
    for bit in [true, false, true, true] {
        writer.write_bit(bit);
    }
    for byte in [0x5au8, 0x80] {
        for bit in 0..8 {
            writer.write_text_bit(byte & (0x80 >> bit) != 0);
        }
    }
    writer.write_handle(DwgReferenceType::HardPointer, owner.value());
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        handle,
        ObjectType::Unknown {
            type_name: NAME.into(),
            handle,
            owner,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(version),
        },
    );
    if let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&owner) {
        dictionary.add_entry("AcMapResourceManager", handle);
    }
    (document, handle)
}

#[test]
fn zombie_map_preserves_payload_text_and_references_in_both_formats() {
    for version in [
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let (source, handle) = synthetic(version, true);
        let bytes = DwgWriter::write_to_vec(&source).unwrap();
        let source = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let expected = manager(&source, handle);
        assert_eq!(expected.payload.bit_count, 28);
        assert_eq!(expected.payload.data(), [0x40, 0x61, 0xa5, 0xb0]);
        assert_eq!(expected.text_payload.bit_count, 16);
        assert_eq!(expected.text_payload.data(), [0x5a, 0x80]);
        assert_eq!(expected.object_ids.len(), 1);
        assert_eq!(expected.reactors, [expected.owner]);
        assert!(expected.xdictionary_handle.is_some());
        let dxf = DxfWriter::new(&source).write_to_vec().unwrap();
        let dxf = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        let actual = manager(&dxf, handle);
        assert_eq!(actual.payload, expected.payload);
        assert_eq!(actual.text_payload, expected.text_payload);
        assert_eq!(actual.object_ids, expected.object_ids);
        assert_eq!(actual.owner, expected.owner);
        assert_eq!(actual.reactors, expected.reactors);
        assert_eq!(actual.xdictionary_handle, expected.xdictionary_handle);
        let bytes = DwgWriter::write_to_vec(&source).unwrap();
        let same = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(manager(&same, handle), expected);
        let bytes = DwgWriter::write_to_vec(&dxf).unwrap();
        let back = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(manager(&back, handle).payload, expected.payload);
        assert_eq!(manager(&back, handle).text_payload, expected.text_payload);
        assert_eq!(manager(&back, handle).object_ids, expected.object_ids);
    }
}

#[test]
fn live_map_class_keeps_its_original_unknown_record() {
    let (source, handle) = synthetic(DxfVersion::AC1032, false);
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let source = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert!(matches!(
        &source.objects[&handle],
        ObjectType::Unknown {
            raw_dwg_data: Some(_),
            ..
        }
    ));
}

#[test]
#[ignore = "requires ARQ_NATIVE_TABLE_ROOT with the original ProgeCAD sources"]
fn native_dwg_to_dxf_preserves_map_resource_manager() {
    let root = std::env::var("ARQ_NATIVE_TABLE_ROOT").unwrap();
    let source = DwgReader::from_file(format!("{root}/tables-native-ac1032.dwg"))
        .unwrap()
        .read()
        .unwrap();
    let handle = Handle::from(676u64);
    let expected = manager(&source, handle);
    assert_eq!(expected.payload.bit_count, 12);
    assert_eq!(expected.payload.data(), [0x40, 0x60]);
    assert_eq!(expected.owner, Handle::from(675u64));
    assert_eq!(expected.reactors, [expected.owner]);
    assert!(expected.object_ids.is_empty());
    assert_eq!(expected.text_payload.bit_count, 0);
    eprintln!("decoded native manager: {expected:?}");
    let bytes = DxfWriter::new(&source).write_to_vec().unwrap();
    if let Ok(path) = std::env::var("ARQ_MAP_OUTPUT") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut file, &bytes).unwrap();
    }
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let actual = manager(&back, handle);
    assert_eq!(actual.payload, expected.payload);
    assert_eq!(actual.owner, expected.owner);
    assert_eq!(actual.reactors, expected.reactors);
    assert_eq!(actual.object_ids, expected.object_ids);
    assert_eq!(actual.text_payload, expected.text_payload);
    assert_eq!(actual.dwg_version, 33);
    assert_eq!(actual.maintenance_version, 55);
    assert!(!actual.from_dxf);
    let ObjectType::Dictionary(owner) = &back.objects[&expected.owner] else {
        panic!("missing owner dictionary")
    };
    assert_eq!(owner.get("AcMapResourceManager"), Some(handle));
}

#[test]
fn dxf_proxy_class_indexes_follow_file_order_and_keep_duplicate_names() {
    let dxf = "0\nSECTION\n2\nCLASSES\n0\nCLASS\n1\nMAPRESOURCEMANAGEROBJECT\n2\nAcMapResourceManager\n3\nAcMapSharedComponents\n280\n1\n281\n0\n0\nCLASS\n1\nMAPRESOURCEMANAGEROBJECT\n2\nOtherMapManager\n3\nOtherApplication\n280\n1\n281\n0\n0\nENDSEC\n0\nEOF\n";
    let document = DxfReader::from_reader(Cursor::new(dxf))
        .unwrap()
        .read()
        .unwrap();
    let classes: Vec<_> = document.classes.iter().collect();
    assert_eq!(classes.len(), 2);
    assert_eq!(classes[0].class_number, 500);
    assert_eq!(classes[0].cpp_class_name, "AcMapResourceManager");
    assert_eq!(classes[1].class_number, 501);
    assert_eq!(classes[1].cpp_class_name, "OtherMapManager");
}
