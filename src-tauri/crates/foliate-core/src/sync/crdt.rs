use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const SYNC_SCHEMA_VERSION: u32 = 1;
pub const TOMBSTONE_MAX_AGE_MS: i64 = 90 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressRecord {
    pub cfi: String,
    pub fraction: f64,
    pub updated_at_ms: i64,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRecord {
    pub book_hash: String,
    pub kind: String,
    pub cfi: String,
    pub color: String,
    pub quote_text: String,
    pub note_text: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionRecord {
    pub name: String,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookCollectionRecord {
    pub book_hash: String,
    pub collection_id: String,
    pub updated_at_ms: i64,
    pub deleted_at_ms: Option<i64>,
}

fn schema_version() -> u32 {
    SYNC_SCHEMA_VERSION
}

/// Canonical sync document. All maps are keyed by stable identifiers:
/// `book_hash`, annotation UUID, collection UUID, and
/// `book_hash:collection_id` respectively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncDocument {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub exported_at_ms: i64,
    #[serde(default)]
    pub progress: BTreeMap<String, ProgressRecord>,
    #[serde(default)]
    pub annotations: BTreeMap<String, AnnotationRecord>,
    #[serde(default)]
    pub collections: BTreeMap<String, CollectionRecord>,
    #[serde(default)]
    pub book_collections: BTreeMap<String, BookCollectionRecord>,
}

impl Default for SyncDocument {
    fn default() -> Self {
        Self {
            schema_version: SYNC_SCHEMA_VERSION,
            exported_at_ms: 0,
            progress: BTreeMap::new(),
            annotations: BTreeMap::new(),
            collections: BTreeMap::new(),
            book_collections: BTreeMap::new(),
        }
    }
}

impl SyncDocument {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SYNC_SCHEMA_VERSION {
            return Err(CoreError::SyncSchema(self.schema_version));
        }
        Ok(())
    }
}

/// Merge two documents deterministically.
///
/// This is a last-writer-wins map merge, not a full OR-Set: deletions are
/// represented by tombstones (`deleted_at_ms`) that participate in the same
/// timestamp comparison. A tombstone with the newest `updated_at_ms` wins,
/// which is what propagates a delete from one device to another.
pub fn merge_documents(local: &SyncDocument, remote: &SyncDocument) -> Result<SyncDocument> {
    local.validate()?;
    remote.validate()?;

    let mut merged = local.clone();
    for (key, remote_value) in &remote.progress {
        match merged.progress.get(key) {
            Some(local_value) if local_progress_wins(local_value, remote_value) => {}
            _ => {
                merged.progress.insert(key.clone(), remote_value.clone());
            }
        }
    }

    merge_lww_map(&mut merged.annotations, &remote.annotations);
    merge_lww_map(&mut merged.collections, &remote.collections);
    merge_lww_map(&mut merged.book_collections, &remote.book_collections);

    Ok(merged)
}

fn local_progress_wins(local: &ProgressRecord, remote: &ProgressRecord) -> bool {
    match local.updated_at_ms.cmp(&remote.updated_at_ms) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match local.device_id.cmp(&remote.device_id) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => {
                let local_key = (local.cfi.as_str(), local.fraction.to_bits());
                let remote_key = (remote.cfi.as_str(), remote.fraction.to_bits());
                local_key >= remote_key
            }
        },
    }
}

fn merge_lww_map<T>(local: &mut BTreeMap<String, T>, remote: &BTreeMap<String, T>)
where
    T: Clone + Serialize + HasUpdatedAt,
{
    for (key, remote_value) in remote {
        match local.get(key) {
            Some(local_value) if !lww_remote_wins(local_value, remote_value) => {}
            _ => {
                local.insert(key.clone(), remote_value.clone());
            }
        }
    }
}

pub trait HasUpdatedAt {
    fn updated_at_ms(&self) -> i64;
}

impl HasUpdatedAt for AnnotationRecord {
    fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

impl HasUpdatedAt for CollectionRecord {
    fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

impl HasUpdatedAt for BookCollectionRecord {
    fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

/// Higher `updated_at_ms` wins. Progress ties use the lexicographically higher
/// `device_id`; other records use a canonical JSON comparison. Every replica
/// therefore picks the same value, and identical records keep the local copy.
fn lww_remote_wins<T>(local: &T, remote: &T) -> bool
where
    T: Serialize + HasUpdatedAt,
{
    match remote.updated_at_ms().cmp(&local.updated_at_ms()) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            let local_json = serde_json::to_string(local).unwrap_or_default();
            let remote_json = serde_json::to_string(remote).unwrap_or_default();
            remote_json > local_json
        }
    }
}

/// Drop tombstones older than the retention window. Live records always stay.
pub fn prune_tombstones(document: &mut SyncDocument, now_ms: i64, max_age_ms: i64) {
    let cutoff = now_ms.saturating_sub(max_age_ms);
    document
        .annotations
        .retain(|_, record| keep_tombstone(record.deleted_at_ms, cutoff));
    document
        .collections
        .retain(|_, record| keep_tombstone(record.deleted_at_ms, cutoff));
    document
        .book_collections
        .retain(|_, record| keep_tombstone(record.deleted_at_ms, cutoff));
}

fn keep_tombstone(deleted_at_ms: Option<i64>, cutoff: i64) -> bool {
    deleted_at_ms.is_none_or(|deleted| deleted > cutoff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::now_ms;

    fn progress(device: &str, updated: i64, fraction: f64) -> ProgressRecord {
        ProgressRecord {
            cfi: format!("epubcfi(/6/{updated})"),
            fraction,
            updated_at_ms: updated,
            device_id: device.into(),
        }
    }

    fn annotation(updated: i64, deleted: Option<i64>) -> AnnotationRecord {
        AnnotationRecord {
            book_hash: "hash".into(),
            kind: "highlight".into(),
            cfi: "epubcfi(/6/2)".into(),
            color: "yellow".into(),
            quote_text: "quote".into(),
            note_text: String::new(),
            created_at_ms: 1,
            updated_at_ms: updated,
            deleted_at_ms: deleted,
        }
    }

    #[test]
    fn newer_progress_wins() {
        let mut local = SyncDocument::default();
        local
            .progress
            .insert("book".into(), progress("win-a", 100, 0.1));
        let mut remote = SyncDocument::default();
        remote
            .progress
            .insert("book".into(), progress("and-b", 200, 0.2));
        let merged = merge_documents(&local, &remote).unwrap();
        assert_eq!(merged.progress["book"].fraction, 0.2);
        assert_eq!(merged.progress["book"].device_id, "and-b");
    }

    #[test]
    fn progress_ties_break_on_device_id() {
        let mut local = SyncDocument::default();
        local
            .progress
            .insert("book".into(), progress("win-a", 100, 0.1));
        let mut remote = SyncDocument::default();
        remote
            .progress
            .insert("book".into(), progress("win-b", 100, 0.9));
        let merged = merge_documents(&local, &remote).unwrap();
        assert_eq!(merged.progress["book"].device_id, "win-b");
        assert_eq!(merged.progress["book"].fraction, 0.9);
        let reverse = merge_documents(&remote, &local).unwrap();
        assert_eq!(reverse, merged);
    }

    #[test]
    fn tombstone_with_newer_timestamp_wins() {
        let mut local = SyncDocument::default();
        local
            .annotations
            .insert("ann".into(), annotation(100, None));
        let mut remote = SyncDocument::default();
        remote
            .annotations
            .insert("ann".into(), annotation(200, Some(200)));
        let merged = merge_documents(&local, &remote).unwrap();
        assert_eq!(merged.annotations["ann"].deleted_at_ms, Some(200));
    }

    #[test]
    fn equal_timestamps_converge_regardless_of_direction() {
        let mut a = SyncDocument::default();
        a.annotations.insert("ann".into(), annotation(100, None));
        let mut b = SyncDocument::default();
        b.annotations
            .insert("ann".into(), annotation(100, Some(100)));
        let ab = merge_documents(&a, &b).unwrap();
        let ba = merge_documents(&b, &a).unwrap();
        assert_eq!(ab, ba);
    }

    #[test]
    fn merge_preserves_unknown_remote_records() {
        let local = SyncDocument::default();
        let mut remote = SyncDocument::default();
        remote.collections.insert(
            "c1".into(),
            CollectionRecord {
                name: "One".into(),
                updated_at_ms: 10,
                deleted_at_ms: None,
            },
        );
        remote
            .annotations
            .insert("unknown-book-ann".into(), annotation(10, None));
        let merged = merge_documents(&local, &remote).unwrap();
        assert_eq!(merged.collections.len(), 1);
        assert_eq!(merged.annotations.len(), 1);
    }

    #[test]
    fn prunes_only_old_tombstones() {
        let now = now_ms();
        let mut doc = SyncDocument::default();
        doc.annotations.insert(
            "old".into(),
            annotation(1, Some(now - TOMBSTONE_MAX_AGE_MS - 1)),
        );
        doc.annotations
            .insert("new".into(), annotation(1, Some(now - 1000)));
        doc.annotations.insert("live".into(), annotation(1, None));
        prune_tombstones(&mut doc, now, TOMBSTONE_MAX_AGE_MS);
        assert!(!doc.annotations.contains_key("old"));
        assert!(doc.annotations.contains_key("new"));
        assert!(doc.annotations.contains_key("live"));
    }

    #[test]
    fn rejects_unknown_schema() {
        let doc = SyncDocument {
            schema_version: 2,
            ..Default::default()
        };
        assert!(matches!(
            merge_documents(&SyncDocument::default(), &doc),
            Err(CoreError::SyncSchema(2))
        ));
    }
}
