//! Preserve the unloaded Xref marker emitted by native ProgeCAD DXF exports.
//! Hand-written records keep the external-application observation reproducible
//! without distributing any private drawing.

use acadrust::tables::BlockRecord;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn read(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn fixture(flags: i16, unloaded: Option<i16>, path: &str) -> Vec<u8> {
    let marker = unloaded.map(|v| format!("71\n{v}\n")).unwrap_or_default();
    format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nSITE\n70\n{flags}\n{marker}10\n0\n20\n0\n30\n0\n1\n{path}\n0\nENDBLK\n0\nENDSEC\n0\nEOF\n").into_bytes()
}

#[test]
fn native_dxf_unloaded_marker_is_read_for_attachments_and_overlays() {
    for flags in [4, 12] {
        let doc = read(fixture(flags, Some(1), "child.dwg"));
        let block = doc.block_records.get("SITE").unwrap();
        assert!(
            block.flags.is_xref_unloaded,
            "BLOCK 71=1 must survive reading"
        );
        assert!(block.flags.is_xref);
        assert_eq!(block.flags.is_xref_overlay, flags & 8 != 0);
        assert_eq!(block.xref_path, "child.dwg");
    }
}

#[test]
fn unresolved_or_loaded_xrefs_are_not_inferred_to_be_unloaded() {
    for flags in [4, 100, 12, 108] {
        for unloaded in [None, Some(0)] {
            let doc = read(fixture(flags, unloaded, "child.dwg"));
            assert!(
                !doc.block_records
                    .get("SITE")
                    .unwrap()
                    .flags
                    .is_xref_unloaded
            );
        }
    }
    let local = read(fixture(0, Some(1), ""));
    assert!(
        !local
            .block_records
            .get("SITE")
            .unwrap()
            .flags
            .is_xref_unloaded,
        "a marker on a local block must not invent an external reference"
    );
}

#[test]
fn loaded_and_unloaded_xrefs_roundtrip_in_every_dxf_version() {
    for version in [
        DxfVersion::AC1009,
        DxfVersion::AC1012,
        DxfVersion::AC1014,
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        for overlay in [false, true] {
            for unloaded in [false, true] {
                let mut doc = CadDocument::with_version(version);
                let mut block = BlockRecord::new("SITE");
                block.handle = doc.allocate_handle();
                block.block_entity_handle = doc.allocate_handle();
                block.block_end_handle = doc.allocate_handle();
                block.flags.is_xref = true;
                block.flags.is_xref_overlay = overlay;
                block.flags.is_xref_unloaded = unloaded;
                block.xref_path = "child.dwg".to_owned();
                doc.block_records.add(block).unwrap();
                // Binary R12 is deliberately unsupported by the codec.
                for binary in [false, true] {
                    if binary && version == DxfVersion::AC1009 {
                        continue;
                    }
                    let mut writer = DxfWriter::new(&doc);
                    writer.set_binary(binary);
                    let back = read(writer.write_to_vec().unwrap());
                    let block = back.block_records.get("SITE").unwrap();
                    assert_eq!(
                        block.flags.is_xref_unloaded, unloaded,
                        "{version:?}, overlay={overlay}"
                    );
                    assert_eq!(block.flags.is_xref_overlay, overlay);
                    assert!(block.flags.is_xref);
                    assert_eq!(block.xref_path, "child.dwg");
                    assert!(block.entity_handles.is_empty());
                }
            }
        }
    }
}
