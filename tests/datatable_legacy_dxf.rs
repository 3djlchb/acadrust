use acadrust::{DwgReader, DxfReader, DxfVersion, DxfWriter};
use std::io::Cursor;

#[test]
fn legacy_ascii_dxf_refuses_datatable_before_writing_any_bytes() {
    let mut doc = DwgReader::from_stream(Cursor::new(include_bytes!("datatable/point_object_id.dwg"))).read().unwrap();
    for version in [DxfVersion::AC1012, DxfVersion::AC1014, DxfVersion::AC1015] {
        for explicit in [false, true] {
            doc.version = if explicit { DxfVersion::AC1032 } else { version };
            let writer = if explicit { DxfWriter::new(&doc).with_version(version) } else { DxfWriter::new(&doc) };
            let mut bytes = b"unchanged".to_vec();
            let error = writer.write_to_writer(&mut bytes).expect_err("native readers empty this legacy DATATABLE body");
            assert!(error.to_string().contains("DATATABLE"));
            assert_eq!(bytes, b"unchanged");
            assert_eq!(doc.version, if explicit { DxfVersion::AC1032 } else { version });
        }
    }
}

#[test]
fn explicit_modern_dxf_keeps_the_table_without_mutating_the_source_version() {
    let mut doc = DwgReader::from_stream(Cursor::new(include_bytes!("datatable/point_object_id.dwg"))).read().unwrap();
    doc.version = DxfVersion::AC1015;
    let bytes = DxfWriter::new(&doc).with_version(DxfVersion::AC1018).write_to_vec().unwrap();
    let back = DxfReader::from_reader(Cursor::new(bytes)).unwrap().read().unwrap();
    assert_eq!(back.version, DxfVersion::AC1018);
    assert_eq!(doc.version, DxfVersion::AC1015);
    assert!(back.objects.values().any(|o| matches!(o, acadrust::objects::ObjectType::ClassObject(c) if matches!(c.data, acadrust::objects::ClassObjectData::DataTable(_)))));
}
