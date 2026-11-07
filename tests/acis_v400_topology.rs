//! ACIS 4.0 (SAT 400) lumps and shells keep their next pointers.
//!
//! ACIS 7.0 added one pointer to each topology record, right after its
//! attribute and id, which `SatLump` and `SatShell` read as token 0 (a lump
//! is `$pattern $next $shell $body`, a shell `$pattern $next $subshell
//! $face $wire $lump`). A SAT 400 record lacks it: written down to 400, a
//! record must drop token 0, and read from 400, it must get the placeholder
//! back at 0. Dropping or inserting at 1 for lumps and 2 for shells instead
//! loses a body's second lump and a lump's void, the shell after its first:
//! saved in DWG 2000, a solid with a cavity is read by other applications
//! as a solid one.

use acadrust::entities::acis::types::{SatDocument, SatLump, SatPointer, SatShell, SatVersion};

/// A body of two lumps; the first has an outer shell and a void.
const TWO_LUMPS_V700: &str = "700 0 1 0\n\
    @8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
    1e-06 9.9999999999999995e-07\n\
    body $-1 -1 $-1 $1 $-1 $-1 #\n\
    lump $-1 -1 $-1 $2 $3 $0 #\n\
    lump $-1 -1 $-1 $-1 $5 $0 #\n\
    shell $-1 -1 $-1 $4 $-1 $-1 $-1 $1 #\n\
    shell $-1 -1 $-1 $-1 $-1 $-1 $-1 $1 #\n\
    shell $-1 -1 $-1 $-1 $-1 $-1 $-1 $2 #\n\
    End-of-ACIS-data\n";

/// The same body in SAT 400.
const TWO_LUMPS_V400: &str = "400 0 1 0\n\
    8 acadrust 8 ACIS 4.0 24 Thu Jan 01 00:00:00 2023\n\
    1e-06 9.9999999999999995e-07\n\
    body $-1 $1 $-1 $-1 #\n\
    lump $-1 $2 $3 $0 #\n\
    lump $-1 $-1 $5 $0 #\n\
    shell $-1 $4 $-1 $-1 $-1 $1 #\n\
    shell $-1 $-1 $-1 $-1 $-1 $1 #\n\
    shell $-1 $-1 $-1 $-1 $-1 $2 #\n\
    End-of-ACIS-data\n";

#[track_caller]
fn assert_topology(doc: &SatDocument) {
    let lump = SatLump::from_record(&doc.records[1]).expect("a lump");
    assert_eq!(lump.next_lump(), SatPointer::new(2), "the second lump");
    assert_eq!(lump.shell(), SatPointer::new(3));
    assert_eq!(lump.body(), SatPointer::new(0));
    let outer = SatShell::from_record(&doc.records[3]).expect("a shell");
    assert_eq!(outer.next_shell(), SatPointer::new(4), "the void");
    assert_eq!(outer.subshell(), SatPointer::NULL);
    assert_eq!(outer.lump(), SatPointer::new(1));
    let last = SatShell::from_record(&doc.records[5]).expect("a shell");
    assert_eq!(last.lump(), SatPointer::new(2));
}

#[test]
fn v700_lumps_and_shells_read_their_next_pointers() {
    assert_topology(&SatDocument::parse(TWO_LUMPS_V700).unwrap());
}

#[test]
fn v400_lumps_and_shells_read_their_next_pointers() {
    let doc = SatDocument::parse(TWO_LUMPS_V400).unwrap();
    assert_eq!(doc.header.version, SatVersion::new(4, 0, 0));
    assert_topology(&doc);
}

#[test]
fn written_down_to_v400_lumps_and_shells_keep_their_next_pointers() {
    let mut doc = SatDocument::parse(TWO_LUMPS_V700).unwrap();
    doc.header.version = SatVersion::V4_0;
    let text = doc.to_sat_string();
    let records: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("lump") || l.starts_with("shell"))
        .collect();
    assert_eq!(
        records,
        [
            "lump $-1 $2 $3 $0 #",
            "lump $-1 $-1 $5 $0 #",
            "shell $-1 $4 $-1 $-1 $-1 $1 #",
            "shell $-1 $-1 $-1 $-1 $-1 $1 #",
            "shell $-1 $-1 $-1 $-1 $-1 $2 #",
        ],
        "{text}"
    );
    assert_topology(&SatDocument::parse(&text).unwrap());
}
