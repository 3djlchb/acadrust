use std::{io::Cursor, path::PathBuf};

use acadrust::io::dwg::dwg_stream_readers::{
    handle_reader::read_handles, object_reader::DwgObjectReader,
};
use acadrust::io::dwg::DwgReferenceType;
use acadrust::objects::{ClassObject, ClassObjectData, DataTable, LayerFilter, ObjectType};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};

fn table(doc: &CadDocument) -> (&ClassObject, &DataTable) {
    doc.objects
        .values()
        .find_map(|o| match o {
            ObjectType::ClassObject(object) => match &object.data {
                ClassObjectData::DataTable(table) => Some((object, table)),
                _ => None,
            },
            _ => None,
        })
        .expect("DATATABLE must be decoded, not opaque or omitted")
}

#[test]
fn hard_owned_cells_keep_their_values_targets_and_native_reference_kind() {
    let mut doc =
        DwgReader::from_stream(Cursor::new(include_bytes!("datatable/point_object_id.dwg")))
            .read()
            .unwrap();
    let table_handle = table(&doc).0.handle;
    let mut child = ClassObject::new(ClassObjectData::LayerFilter(LayerFilter {
        names: vec!["Owned cell".into()],
    }));
    child.handle = doc.allocate_handle();
    child.owner = table_handle;
    let target = child.handle;
    doc.objects
        .insert(target, ObjectType::ClassObject(child.clone()));
    if let ObjectType::ClassObject(object) = doc.objects.get_mut(&table_handle).unwrap() {
        if let ClassObjectData::DataTable(table) = &mut object.data {
            table.columns[1].value_type = 6;
            table.columns[1].rows[0].handle = target;
        }
    }
    let expected = table(&doc).1.clone();
    let bytes = DwgWriter::write_to_vec(&doc).expect("native hard-ownership cells are supported");
    let back = DwgReader::from_stream(Cursor::new(&bytes)).read().unwrap();
    assert_eq!(table(&back).1, &expected);
    assert_eq!(
        back.objects[&target],
        ObjectType::ClassObject(child.clone())
    );
    assert_eq!(table(&doc).1, &expected, "writing never mutates the caller");

    let mut reader = DwgReader::from_stream(Cursor::new(&bytes));
    let header = reader.read_file_header().unwrap();
    let handles =
        read_handles(&reader.get_section_buffer("AcDb:Handles", &header).unwrap()).unwrap();
    let offset = handles[&table_handle.value()];
    let objects = DwgObjectReader::new(
        reader
            .get_section_buffer("AcDb:AcDbObjects", &header)
            .unwrap(),
        DxfVersion::AC1032,
        handles,
    )
    .unwrap();
    let (code, mut record) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_non_entity_data(&mut record, code);
    assert_eq!(record.read_bit_short(), 2);
    assert_eq!(record.read_bit_long(), 2);
    assert_eq!(record.read_bit_long(), 1);
    record.read_variable_text();
    assert_eq!(record.read_bit_long(), 4);
    record.read_variable_text();
    record.read_3bit_double();
    assert_eq!(record.read_bit_long(), 6);
    record.read_variable_text();
    assert_eq!(
        record.read_typed_handle(),
        (target.value(), DwgReferenceType::HardOwnership)
    );

    for writer in [DxfWriter::new(&doc), DxfWriter::new_binary(&doc)] {
        let bytes = writer.write_to_vec().unwrap();
        let back = DxfReader::from_reader(Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(table(&back).1, &expected);
        assert_eq!(
            back.objects[&target],
            ObjectType::ClassObject(child.clone())
        );
    }
    let ascii = DxfWriter::new(&doc).write_to_vec().unwrap();
    let text = String::from_utf8(ascii).unwrap();
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    assert!(lines
        .chunks_exact(2)
        .any(|p| p[0] == "360" && p[1].eq_ignore_ascii_case(&format!("{:X}", target.value()))));
}

fn native_roundtrip(name: &str, handle: u64, owner: u64) {
    let root = PathBuf::from(
        std::env::var_os("ARQ_DATATABLE_CORPUS")
            .expect("set ARQ_DATATABLE_CORPUS to the fetched LibreDWG corpus"),
    );
    let doc = DwgReader::from_file(root.join(name))
        .unwrap()
        .read()
        .unwrap();
    let (object, expected) = table(&doc);
    assert_eq!(object.handle, Handle::new(handle));
    assert_eq!(object.owner, Handle::new(owner));
    assert_eq!(expected.flags, 2);
    assert_eq!(expected.row_count, 180);
    assert_eq!(
        expected
            .columns
            .iter()
            .map(|c| c.value_type)
            .collect::<Vec<_>>(),
        [2, 1, 1, 6]
    );
    assert_eq!(expected.columns[0].rows[0].real, 1127.0);
    assert!(expected.columns.iter().all(|c| c.rows.len() == 180));
    for (index, row) in expected.columns[3].rows.iter().enumerate() {
        assert_eq!(row.handle, Handle::new(handle + 1 + index as u64));
        assert!(doc.objects.contains_key(&row.handle));
    }
    let mut copy = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert_eq!(table(&copy), (object, expected));
    for _ in 0..2 {
        copy = DxfReader::from_reader(Cursor::new(DxfWriter::new(&copy).write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(table(&copy), (object, expected));
        for row in &expected.columns[3].rows {
            assert_eq!(copy.objects.get(&row.handle), doc.objects.get(&row.handle));
        }
    }
}

#[test]
#[ignore = "needs ARQ_DATATABLE_CORPUS"]
fn native_r2000_datatable_survives_dwg_and_repeated_dxf_conversion() {
    native_roundtrip("example_2000.dwg", 0xB2C, 0xADB);
}

#[test]
#[ignore = "needs ARQ_DATATABLE_CORPUS"]
fn native_r14_datatable_survives_dwg_and_repeated_dxf_conversion() {
    native_roundtrip("example_r14.dwg", 0xD62, 0xCAB);
}
