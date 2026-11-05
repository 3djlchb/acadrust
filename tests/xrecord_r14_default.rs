use acadrust::objects::{DictionaryCloningFlags, ObjectType, XRecord};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter};
use std::io::Cursor;

#[test]
fn r14_implicit_xrecord_cloning_rule_survives_modern_conversion() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1014);
    let mut record = XRecord::new();
    record.handle = doc.allocate_handle();
    record.cloning_flags = DictionaryCloningFlags::KeepExisting;
    record.add_string(1, "Owned cell");
    let handle = record.handle;
    doc.objects.insert(handle, ObjectType::XRecord(record));
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let mut back = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let ObjectType::XRecord(record) = &back.objects[&handle] else { panic!("XRECORD missing") };
    // R14 has no encoded cloning field; native ProgeCAD exposes KeepExisting.
    assert_eq!(record.cloning_flags, DictionaryCloningFlags::KeepExisting);
    back.version = DxfVersion::AC1032;
    let modern = DxfReader::from_reader(Cursor::new(DxfWriter::new(&back).write_to_vec().unwrap())).unwrap().read().unwrap();
    let ObjectType::XRecord(record) = &modern.objects[&handle] else { panic!("XRECORD missing") };
    assert_eq!(record.cloning_flags, DictionaryCloningFlags::KeepExisting);
}
