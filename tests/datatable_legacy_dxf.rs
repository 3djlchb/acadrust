use acadrust::objects::{ClassObjectData, DataTable, ObjectType};
use acadrust::{CadDocument, DwgReader, DxfReader, DxfVersion, DxfWriter};
use std::io::Cursor;

fn table(doc: &CadDocument) -> &DataTable {
    doc.objects.values().find_map(|object| match object {
        ObjectType::ClassObject(class) => match &class.data {
            ClassObjectData::DataTable(table) => Some(table),
            _ => None,
        },
        _ => None,
    }).expect("native table missing")
}

#[test]
fn legacy_native_uppercase_subclass_keeps_all_cells_on_read() {
    let text = include_str!("datatable/point_object_id.dxf").replace("\r\n", "\n");
    let source = DxfReader::from_reader(Cursor::new(text.clone().into_bytes())).unwrap().read().unwrap();
    let legacy = text.replace("AC1032", "AC1015").replace("100\nAcDbDataTable\n", "100\nACDBDATATABLE\n");
    assert_ne!(legacy, text);
    assert!(legacy.contains("100\nACDBDATATABLE\n"));
    let back = DxfReader::from_reader(Cursor::new(legacy)).unwrap().read().unwrap();
    assert_eq!(table(&back), table(&source));
}

#[test]
fn writer_uses_the_native_versioned_subclass_without_changing_the_source() {
    let mut doc = DwgReader::from_stream(Cursor::new(include_bytes!("datatable/point_object_id.dwg"))).read().unwrap();
    for version in [DxfVersion::AC1014, DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        for explicit in [false, true] {
            doc.version = if explicit { DxfVersion::AC1032 } else { version };
            let writer = if explicit { DxfWriter::new(&doc).with_version(version) } else { DxfWriter::new(&doc) };
            let text = String::from_utf8(writer.write_to_vec().unwrap()).unwrap();
            let marker = if version < DxfVersion::AC1018 { "ACDBDATATABLE" } else { "AcDbDataTable" };
            assert!(text.replace("\r\n", "\n").contains(&format!("100\n{marker}\n")));
            let back = DxfReader::from_reader(Cursor::new(text)).unwrap().read().unwrap();
            assert_eq!(back.version, version);
            assert_eq!(table(&back), table(&doc));
            assert_eq!(doc.version, if explicit { DxfVersion::AC1032 } else { version });
        }
    }
}
