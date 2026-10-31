//! Native DXF round-trip content preserves modern table instance styles.
use crate::entities::EntityType;
use crate::objects::{DictionaryCloningFlags, ObjectType, XRecord, XRecordEntry};
use crate::types::DxfVersion;
use crate::CadDocument;
use std::borrow::Cow;

pub(super) fn prepare(document: &mut Cow<'_, CadDocument>, version: DxfVersion) {
    if version < DxfVersion::AC1024 {
        return;
    }
    let tables = document
        .entities()
        .filter_map(|entity| {
            let EntityType::Table(table) = entity else {
                return None;
            };
            if table.base_style.is_none() && table.rows.iter().all(|row| row.style.is_none()) {
                return None;
            }
            if let Some(dictionary) = document.extension_dictionary_handle(table.common.handle) {
                let Some(ObjectType::Dictionary(value)) = document.objects.get(&dictionary) else {
                    return None;
                };
                // Preserve foreign or pre-existing round-trip records verbatim.
                if value.owner != table.common.handle || value.get("ACAD_XREC_ROUNDTRIP").is_some()
                {
                    return None;
                }
            }
            Some(table.as_ref().clone())
        })
        .collect::<Vec<_>>();
    if tables.is_empty() {
        return;
    }
    let doc = document.to_mut();
    for mut content in tables {
        let dictionary = doc.ensure_extension_dictionary(content.common.handle);
        let record_handle = doc.allocate_handle();
        let content_handle = doc.allocate_handle();
        content.common.handle = content_handle;
        content.common.owner_handle = record_handle;
        content.common.xdictionary_handle = None;
        // Reactors belong to the original drawing entity, not the new helper object.
        content.common.reactors.clear();
        let mut record = XRecord::named("ACAD_XREC_ROUNDTRIP");
        record.handle = record_handle;
        record.owner = dictionary;
        record.reactors = vec![dictionary];
        record.cloning_flags = DictionaryCloningFlags::KeepExisting;
        record.entries = vec![
            XRecordEntry::string(102, "ACAD_ROUNDTRIP_2008_TABLE_ENTITY"),
            XRecordEntry::handle(360, content_handle),
            XRecordEntry::int16(70, 2),
            XRecordEntry::int32(90, 1),
            XRecordEntry::point3d(10, 0.0, 0.0, 0.0),
            XRecordEntry::int32(90, 0),
            XRecordEntry::int32(90, 4),
        ];
        doc.objects
            .insert(content_handle, ObjectType::TableContent(content));
        doc.objects
            .insert(record_handle, ObjectType::XRecord(record));
        if let Some(ObjectType::Dictionary(value)) = doc.objects.get_mut(&dictionary) {
            value.add_entry("ACAD_XREC_ROUNDTRIP", record_handle);
        }
    }
}
