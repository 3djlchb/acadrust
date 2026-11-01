//! Same-version R2000 styles must not be treated as a downgrade.
use acadrust::objects::ObjectType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;
use std::path::PathBuf;

fn rewrite(source: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(source).unwrap()))
        .read()
        .unwrap()
}

#[test]
fn existing_r2000_style_dictionary_survives_without_mutating_the_source() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.dwg_source_version = Some(DxfVersion::AC1015);
    let original = doc.objects.clone();
    let (handle, style) = doc
        .objects
        .iter()
        .find_map(|(h, o)| match o {
            ObjectType::TableStyle(s) => Some((*h, s.clone())),
            _ => None,
        })
        .expect("default style");
    let back = rewrite(&doc);
    let Some(ObjectType::TableStyle(saved)) = back.objects.get(&handle) else {
        panic!("existing R2000 TABLESTYLE was removed");
    };
    assert_eq!(saved.name, style.name);
    assert_eq!(saved.horizontal_margin, style.horizontal_margin);
    assert_eq!(saved.vertical_margin, style.vertical_margin);
    assert_eq!(saved.owner_handle, style.owner_handle);
    assert_eq!(doc.objects, original);
}

fn native(relative: &str) {
    let root = PathBuf::from(std::env::var_os("ARQ_R2000_CORPUS").expect("set ARQ_R2000_CORPUS"));
    let path = root.join(relative);
    let source = DwgReader::from_file(&path).unwrap().read().unwrap();
    assert_eq!(source.version, DxfVersion::AC1015);
    assert_eq!(source.dwg_source_version, Some(source.version));
    eprintln!(
        "{} class {:?}",
        relative,
        source.classes.get_by_name("TABLESTYLE")
    );
    let styles = source
        .objects
        .iter()
        .filter_map(|(h, o)| match o {
            ObjectType::TableStyle(s) => Some((*h, s.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!styles.is_empty());
    let back = rewrite(&source);
    for (h, s) in styles {
        let Some(ObjectType::TableStyle(b)) = back.objects.get(&h) else {
            panic!("{relative}: TABLESTYLE {} missing at {h:?}", s.name)
        };
        assert_eq!(s.name, b.name);
        assert_eq!(s.owner_handle, b.owner_handle);
        assert_eq!(s.horizontal_margin, b.horizontal_margin);
        assert_eq!(s.vertical_margin, b.vertical_margin);
        let dictionary = source
            .extension_dictionary_handle(h)
            .expect("native companion dictionary");
        assert_eq!(back.extension_dictionary_handle(h), Some(dictionary));
        assert_eq!(
            source.objects.get(&dictionary),
            back.objects.get(&dictionary)
        );
        let Some(ObjectType::Dictionary(d)) = source.objects.get(&dictionary) else {
            panic!()
        };
        for (_, child) in &d.entries {
            match (source.objects.get(child), back.objects.get(child)) {
                (Some(ObjectType::XRecord(a)), Some(ObjectType::XRecord(b))) => {
                    // Encoded transport bytes can change when handle streams are
                    // rewritten; compare every modeled field and the payload.
                    let (mut a, mut b) = (a.clone(), b.clone());
                    for record in [&mut a, &mut b] {
                        record.raw_dwg_data = None;
                        record.raw_dwg_handle_bits = 0;
                        record.raw_dwg_version = None;
                    }
                    assert_eq!(a, b, "round-trip XRecord changed");
                }
                (a, b) => assert_eq!(a, b, "companion changed"),
            }
        }
    }
}
#[test]
#[ignore = "requires ARQ_R2000_CORPUS with fetched public LibreDWG fixtures"]
fn native_r2000_example_retains_table_style_graph() {
    native("example_2000.dwg");
}
#[test]
#[ignore = "requires ARQ_R2000_CORPUS with fetched public LibreDWG fixtures"]
fn native_r2000_constraints_retains_table_style_graph() {
    native("2000/Constraints.dwg");
}

#[test]
fn a_real_downgrade_still_uses_the_existing_style_policy() {
    let source = CadDocument::new();
    let mut doc = rewrite(&source);
    assert_eq!(doc.dwg_source_version, Some(DxfVersion::AC1032));
    let handles = doc
        .objects
        .iter()
        .filter_map(|(h, o)| matches!(o, ObjectType::TableStyle(_)).then_some(*h))
        .collect::<Vec<_>>();
    assert!(!handles.is_empty());
    doc.version = DxfVersion::AC1015;
    let back = rewrite(&doc);
    assert!(handles.iter().all(|h| !back.objects.contains_key(h)));
}

#[test]
fn r2000_rewriting_retains_explicit_style_edits() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.dwg_source_version = Some(doc.version);
    let handle = doc
        .objects
        .iter()
        .find_map(|(h, o)| matches!(o, ObjectType::TableStyle(_)).then_some(*h))
        .unwrap();
    let Some(ObjectType::TableStyle(style)) = doc.objects.get_mut(&handle) else {
        panic!()
    };
    style.horizontal_margin = 3.5;
    style.vertical_margin = 2.25;
    let back = rewrite(&doc);
    let Some(ObjectType::TableStyle(style)) = back.objects.get(&handle) else {
        panic!()
    };
    assert_eq!(style.horizontal_margin, 3.5);
    assert_eq!(style.vertical_margin, 2.25);
}
