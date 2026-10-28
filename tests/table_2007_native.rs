//! Native pre-R2010 table decoding and rewriting regression.
use acadrust::entities::table::BorderPropertyFlags;
use acadrust::entities::{CellStyle, EntityType, Table};
use acadrust::types::{Color, DxfVersion, LineWeight, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

fn tables(doc: &CadDocument) -> Vec<&Table> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

#[test]
fn flat_dwg_border_presence_survives_rewriting() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1021] {
        for (flags, properties, invisible) in [
            (0x800, BorderPropertyFlags::LINE_WEIGHT, false),
            (0x8000, BorderPropertyFlags::INVISIBILITY, true),
            (
                0x8800,
                BorderPropertyFlags::LINE_WEIGHT | BorderPropertyFlags::INVISIBILITY,
                true,
            ),
        ] {
            let mut doc = CadDocument::with_version(version);
            let mut table = Table::new(Vector3::ZERO, 1, 1);
            table.set_cell_text(0, 0, "after border payload");
            let mut style = CellStyle::new();
            style.legacy_override_bits = true;
            style.override_flags = flags;
            style.right_border.line_weight = LineWeight::from_value(50);
            style.right_border.invisible = invisible;
            style.right_border.override_flags = properties;
            table.rows[0].cells[0].style = Some(style);
            doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
            let back = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
                .read()
                .unwrap();
            let table = tables(&back)[0];
            let cell = &table.rows[0].cells[0];
            assert_eq!(cell.text_value(), "after border payload");
            let style = cell.style.as_ref().unwrap();
            assert_eq!(style.override_flags, flags);
            assert_eq!(style.right_border.override_flags, properties);
            assert_eq!(style.right_border.invisible, invisible);
            if properties.contains(BorderPropertyFlags::LINE_WEIGHT) {
                assert_eq!(style.right_border.line_weight, LineWeight::from_value(50));
            }
        }
    }
}

#[test]
#[ignore = "requires ARQ_NATIVE_TABLE_ROOT with the original ProgeCAD sources"]
fn native_2007_reads_all_four_tables_strictly() {
    let root = std::env::var("ARQ_NATIVE_TABLE_ROOT").unwrap();
    let source = DwgReader::from_file(format!("{root}/tables-native-ac1021.dwg"))
        .unwrap()
        .read_with_stats()
        .expect("native 2007 must read without recovery");
    assert!(!source.stats.recovery_mode);
    assert!(!source.stats.recovered());
    assert!(source.stats.stream_completed);
    let doc = source.document;
    let native = DxfReader::from_file(format!("{root}/tables-native-ac1024.dxf"))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(tables(&doc).len(), 4);
    for table in tables(&doc) {
        let expected = tables(&native)
            .into_iter()
            .find(|t| t.insertion_point == table.insertion_point)
            .unwrap();
        assert_eq!(table.rows.len(), 5);
        assert_eq!(table.columns.len(), 3);
        for (a, b) in table.columns.iter().zip(&expected.columns) {
            assert_eq!(a.width, b.width);
        }
        for (a, b) in table.rows.iter().zip(&expected.rows) {
            assert!((a.height - b.height).abs() < 1e-10);
            assert_eq!(a.cells.len(), 3);
            for (a, b) in a.cells.iter().zip(&b.cells) {
                assert_eq!(a.text_value(), b.text_value());
                assert_eq!(
                    (a.merge_width, a.merge_height),
                    (b.merge_width, b.merge_height)
                );
            }
        }
        if table.insertion_point.x == 480.0 {
            for index in [6, 7] {
                let border = &table.rows[index / 3].cells[index % 3]
                    .style
                    .as_ref()
                    .unwrap()
                    .right_border;
                assert_eq!(border.color, Color::from_index(1));
                assert_eq!(border.line_weight, LineWeight::from_value(50));
                assert!(!border.invisible);
                assert_eq!(
                    border.override_flags,
                    BorderPropertyFlags::COLOR | BorderPropertyFlags::LINE_WEIGHT
                );
            }
        }
    }
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream(Cursor::new(bytes.clone()))
        .read_with_stats()
        .unwrap();
    assert!(!back.stats.recovered());
    assert_eq!(tables(&back.document).len(), 4);
    for before in tables(&doc) {
        let after = tables(&back.document)
            .into_iter()
            .find(|t| t.common.handle == before.common.handle)
            .unwrap();
        for (a, b) in before.rows.iter().zip(&after.rows) {
            for (a, b) in a.cells.iter().zip(&b.cells) {
                assert_eq!(a.text_value(), b.text_value());
                assert_eq!(a.style, b.style);
            }
        }
    }
    if let Some(path) = std::env::var_os("ARQ_2007_TABLE_OUTPUT") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        std::io::Write::write_all(&mut file, &bytes).unwrap();
    }
}
