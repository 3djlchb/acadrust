//! A drawing without a "Standard" dimension style (LibreDWG's
//! `example_2000.dwg` has only ISO-25) still gets the default one when read,
//! and its DIMTXSTY must name a text style of that drawing, not the handle the
//! default text style had in a new document. When it named whatever the file
//! held at that handle (there the ByBlock linetype), ProgeCAD 2026's AUDIT
//! reported two errors on the saved drawing: "Object of class
//! AcDbLinetypeTableRecord can't be cast to AcDbTextStyleTableRecord".

use std::io::Cursor;

use acadrust::tables::{DimStyle, TableEntry};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A drawing like `example_2000.dwg`: its text style "Standard" under a
/// handle other than a new document's, and ISO-25 as its only dimension
/// style.
fn without_a_standard_dimension_style() -> Vec<u8> {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1015;
    let moved = doc.allocate_handle();
    let old = doc.header.current_text_style_handle;
    for style in doc.text_styles.iter_mut() {
        if style.handle == old {
            style.set_handle(moved);
        }
    }
    doc.header.current_text_style_handle = moved;
    doc.header.dim_text_style_handle = moved;
    let _ = doc.dim_styles.remove("Standard");
    let mut iso = DimStyle::new("ISO-25");
    iso.set_handle(doc.allocate_handle());
    iso.dimtxsty_handle = moved;
    doc.header.current_dimstyle_handle = iso.handle;
    doc.dim_styles.add(iso).unwrap();
    DwgWriter::write_to_vec(&doc).unwrap()
}

#[test]
fn every_dimension_style_read_names_a_text_style_of_its_drawing() {
    let back = DwgReader::from_stream(Cursor::new(without_a_standard_dimension_style()))
        .read()
        .unwrap();
    assert!(back.dim_styles.iter().count() >= 1);
    for style in back.dim_styles.iter() {
        let named = back
            .text_styles
            .iter()
            .find(|t| t.handle == style.dimtxsty_handle)
            .map(|t| t.name.clone());
        assert_eq!(
            named.as_deref(),
            Some("Standard"),
            "{}: DIMTXSTY {:X}",
            style.name,
            style.dimtxsty_handle.value()
        );
    }
}
