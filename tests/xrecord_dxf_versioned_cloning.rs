use acadrust::objects::{DictionaryCloningFlags, ObjectType, XRecord, XRecordEntry, XRecordValue};
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter, Handle};
use std::io::Cursor;

fn record(doc: &CadDocument, handle: Handle) -> &XRecord {
    match &doc.objects[&handle] {
        ObjectType::XRecord(value) => value,
        _ => panic!("Expected XRECORD"),
    }
}

#[test]
fn r14_dxf_extnames_payload_has_no_r2000_cloning_prefix() {
    for version in [DxfVersion::AC1014, DxfVersion::AC1015, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(version);
        let mut xr = XRecord::new();
        xr.handle = doc.allocate_handle();
        xr.cloning_flags = DictionaryCloningFlags::KeepExisting;
        xr.entries = vec![
            XRecordEntry::string(102, "EXTNAMES"),
            XRecordEntry::string(1, "STANDARD"),
            XRecordEntry::string(2, "Standard"),
        ];
        let handle = xr.handle;
        let expected = xr.entries.clone();
        doc.objects.insert(handle, ObjectType::XRecord(xr));
        let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let lines: Vec<_> = text.lines().collect();
        let subclass = lines
            .iter()
            .enumerate()
            .position(|(index, line)| {
                *line == "AcDbXrecord" && index > 0 && lines[index - 1].trim() == "100"
            })
            .unwrap();
        let next_code = lines[subclass + 1].trim();
        assert_eq!(
            next_code,
            if version < DxfVersion::AC1015 {
                "102"
            } else {
                "280"
            },
            "{version:?}"
        );
        let back = DxfReader::from_reader(Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(record(&back, handle).entries, expected, "{version:?}");
        assert_eq!(
            record(&back, handle).cloning_flags,
            DictionaryCloningFlags::KeepExisting
        );
    }
}

#[test]
fn native_r14_dxf_retains_a_leading_280_payload_entry() {
    let text = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1014\n0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n0\nXRECORD\n5\nABC\n330\n0\n100\nAcDbXrecord\n280\n7\n1\nPayload\n0\nENDSEC\n0\nEOF\n";
    let doc = DxfReader::from_reader(Cursor::new(text.as_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let xr = record(&doc, Handle::new(0xABC));
    assert_eq!(xr.cloning_flags, DictionaryCloningFlags::KeepExisting);
    assert_eq!(
        xr.entries,
        vec![
            XRecordEntry {
                code: 280,
                value: XRecordValue::Byte(7)
            },
            XRecordEntry::string(1, "Payload")
        ]
    );
}

#[test]
fn modern_dxf_distinguishes_cloning_metadata_from_a_280_payload() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    let mut xr = XRecord::new();
    xr.handle = doc.allocate_handle();
    xr.cloning_flags = DictionaryCloningFlags::UseClone;
    xr.entries = vec![
        XRecordEntry {
            code: 280,
            value: XRecordValue::Byte(7),
        },
        XRecordEntry::string(1, "Payload"),
    ];
    let handle = xr.handle;
    let expected = xr.entries.clone();
    doc.objects.insert(handle, ObjectType::XRecord(xr));
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(
        record(&back, handle).cloning_flags,
        DictionaryCloningFlags::UseClone
    );
    assert_eq!(record(&back, handle).entries, expected);
}
