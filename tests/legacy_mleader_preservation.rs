//! Existing legacy style classes must survive same-version rewrites.
use acadrust::objects::{MultiLeaderStyle, ObjectType};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

fn rewrite(source: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(source).unwrap()))
        .read()
        .unwrap()
}
fn assert_styles(source: &CadDocument, back: &CadDocument) {
    let root = source.header.named_objects_dict_handle;
    let ObjectType::Dictionary(original_root) = &source.objects[&root] else {
        panic!()
    };
    let dictionary = original_root.get("ACAD_MLEADERSTYLE").unwrap();
    let ObjectType::Dictionary(written_root) = &back.objects[&root] else {
        panic!()
    };
    assert_eq!(
        written_root.get("ACAD_MLEADERSTYLE"),
        Some(dictionary),
        "style dictionary removed"
    );
    assert_eq!(
        source.objects.get(&dictionary),
        back.objects.get(&dictionary),
        "style ownership changed"
    );
    let styles: Vec<_> = source
        .objects
        .iter()
        .filter_map(|(h, o)| {
            if let ObjectType::MultiLeaderStyle(style) = o {
                Some((h, style))
            } else {
                None
            }
        })
        .collect();
    assert!(!styles.is_empty());
    for (handle, style) in styles {
        let Some(ObjectType::MultiLeaderStyle(saved)) = back.objects.get(handle) else {
            panic!("missing style at {handle:?}")
        };
        assert_eq!(style, saved, "style properties changed at {handle:?}");
    }
    assert_eq!(
        source.header.current_mleader_style_name,
        back.header.current_mleader_style_name
    );
}
#[test]
fn same_version_r2000_retains_styles_without_changing_the_source() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.dwg_source_version = Some(DxfVersion::AC1015);
    // Class registration is part of a real source drawing's provenance.
    let modern = CadDocument::new();
    doc.classes
        .add_or_update(modern.classes.get_by_name("MLEADERSTYLE").unwrap().clone());
    let root = doc.header.named_objects_dict_handle;
    let ObjectType::Dictionary(root_object) = &doc.objects[&root] else {
        panic!()
    };
    let dictionary = root_object.get("ACAD_MLEADERSTYLE").unwrap();
    let extra = doc.allocate_handle();
    let mut style = MultiLeaderStyle::new("Distinct");
    style.handle = extra;
    style.owner_handle = dictionary;
    style.text_height = 7.25;
    style.arrowhead_size = 3.125;
    style.property_changed = true;
    doc.objects
        .insert(extra, ObjectType::MultiLeaderStyle(style));
    let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dictionary) else {
        panic!()
    };
    d.add_entry("Distinct", extra);
    // Names are dictionary keys; vertical attachment is absent before R2010.
    // Model an existing legacy record, not unrepresentable modern defaults.
    for object in doc.objects.values_mut() {
        if let ObjectType::MultiLeaderStyle(style) = object {
            style.name.clear();
            style.text_bottom_attachment = acadrust::objects::TextAttachmentType::TopOfTopLine;
        }
    }
    let original = doc.objects.clone();
    let back = rewrite(&doc);
    assert_eq!(doc.objects, original);
    assert_styles(&doc, &back);
}
#[test]
fn a_real_r2000_downgrade_keeps_its_existing_policy() {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1015;
    let back = rewrite(&doc);
    assert!(!back
        .objects
        .values()
        .any(|o| matches!(o, ObjectType::MultiLeaderStyle(_))));
}
fn native(relative: &str) {
    let root = std::path::PathBuf::from(
        std::env::var_os("ARQ_MLEADER_CORPUS").expect("set public corpus root"),
    );
    let source = DwgReader::from_file(root.join(relative))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(source.dwg_source_version, Some(source.version));
    assert_styles(&source, &rewrite(&source));
    let mut edited = source.clone();
    for object in edited.objects.values_mut() {
        if let ObjectType::MultiLeaderStyle(style) = object {
            style.text_height += 1.125;
            style.arrowhead_size += 0.75;
            style.property_changed = true;
        }
    }
    let before = edited.objects.clone();
    assert_styles(&edited, &rewrite(&edited));
    assert_eq!(edited.objects, before);
}
#[test]
#[ignore = "requires ARQ_MLEADER_CORPUS"]
fn native_r2000_constraints_style_identity_and_properties() {
    native("2000/Constraints.dwg");
}
#[test]
#[ignore = "requires ARQ_MLEADER_CORPUS; R14 evaluated separately"]
fn native_r14_constraints_style_identity_and_properties() {
    native("r14/Constraints.dwg");
}

#[test]
fn new_legacy_documents_do_not_gain_the_same_source_exception() {
    for version in [DxfVersion::AC1014, DxfVersion::AC1015] {
        let source = CadDocument::with_version(version);
        assert_eq!(source.dwg_source_version, None);
        let back = rewrite(&source);
        assert!(!back
            .objects
            .values()
            .any(|o| matches!(o, ObjectType::MultiLeaderStyle(_))));
    }
}
