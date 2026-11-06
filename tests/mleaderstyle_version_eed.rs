//! Before R2010 an MLEADERSTYLE does not carry its class version in its own
//! data: AutoCAD writes it as EED of the `ACAD_MLEADERVER` application,
//! `1070 2`, and readers (ProgeCAD) need it to read the version-2 fields
//! (text angle type, text always left) that follow. Without it, ProgeCAD
//! 2026 refuses an AC1018 drawing with "Object improperly read".

use std::io::Cursor;

use acadrust::objects::ObjectType;
use acadrust::tables::TableEntry;
use acadrust::types::DxfVersion;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn written_and_read(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn styles(doc: &CadDocument) -> Vec<acadrust::Handle> {
    doc.objects
        .iter()
        .filter(|(_, o)| matches!(o, ObjectType::MultiLeaderStyle(_)))
        .map(|(h, _)| *h)
        .collect()
}

fn version_eed(doc: &CadDocument, h: acadrust::Handle) -> Vec<XDataValue> {
    doc.object_xdata(h)
        .and_then(|x| {
            x.records()
                .iter()
                .find(|r| r.application_name == "ACAD_MLEADERVER")
        })
        .map(|r: &ExtendedDataRecord| r.values.clone())
        .unwrap_or_default()
}

#[test]
fn pre_r2010_mleaderstyles_carry_their_version_as_eed() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let back = written_and_read(version);
        assert!(
            back.app_ids.get("ACAD_MLEADERVER").is_some(),
            "{version:?}: no APPID"
        );
        let found = styles(&back);
        assert!(!found.is_empty(), "{version:?}: no MLEADERSTYLE");
        for h in found {
            assert_eq!(
                version_eed(&back, h),
                [XDataValue::Integer16(2)],
                "{version:?} {h:?}"
            );
        }
    }
}

#[test]
fn a_version_eed_already_there_is_kept_once() {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1018;
    let h = styles(&doc)[0];
    let mut app = acadrust::tables::AppId::new("ACAD_MLEADERVER");
    app.set_handle(doc.allocate_handle());
    doc.app_ids.add(app).unwrap();
    let mut record = ExtendedDataRecord::new("ACAD_MLEADERVER");
    record.add_value(XDataValue::Integer16(2));
    doc.set_object_xdata_record(h, record);
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_eq!(version_eed(&back, h), [XDataValue::Integer16(2)]);
    assert_eq!(
        back.app_ids
            .iter()
            .filter(|a| a.name == "ACAD_MLEADERVER")
            .count(),
        1
    );
}

#[test]
fn r2010_and_later_keep_the_version_in_the_object() {
    // From R2010 the class version is a field of the object: no EED added.
    let back = written_and_read(DxfVersion::AC1024);
    for h in styles(&back) {
        assert!(version_eed(&back, h).is_empty(), "{h:?}");
    }
}
