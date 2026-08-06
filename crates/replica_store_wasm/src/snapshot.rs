use radroots_replica_store::ReplicaStoreExportManifestRs;

pub const EXPORT_MANIFEST_FIELD: &str = "manifest_rs";
pub const EXPORT_DB_BYTES_FIELD: &str = "db_bytes";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportManifestSummary {
    pub export_version: String,
    pub replica_store_version: String,
    pub backup_format_version: String,
    pub schema_hash: String,
    pub schema_table_count: usize,
    pub migration_count: usize,
    pub table_count_count: usize,
}

pub fn synced_export_error(pending_count: usize, expected_count: usize) -> Option<String> {
    if pending_count == 0 {
        None
    } else {
        Some(format!(
            "replica store export requires synced state (pending {pending_count}/{expected_count})"
        ))
    }
}

pub fn export_manifest_summary(manifest: &ReplicaStoreExportManifestRs) -> ExportManifestSummary {
    ExportManifestSummary {
        export_version: manifest.export_version.clone(),
        replica_store_version: manifest.replica_store_version.clone(),
        backup_format_version: manifest.backup_format_version.clone(),
        schema_hash: manifest.schema_hash.clone(),
        schema_table_count: manifest.schema.len(),
        migration_count: manifest.migrations.len(),
        table_count_count: manifest.table_counts.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EXPORT_DB_BYTES_FIELD, EXPORT_MANIFEST_FIELD, export_manifest_summary, synced_export_error,
    };

    fn manifest() -> radroots_replica_store::ReplicaStoreExportManifestRs {
        radroots_replica_store::ReplicaStoreExportManifestRs {
            export_version: "1".to_owned(),
            replica_store_version: "0.1.0".to_owned(),
            backup_format_version: "1".to_owned(),
            schema_hash: "schema-hash".to_owned(),
            schema: Vec::new(),
            migrations: Vec::new(),
            table_counts: Vec::new(),
        }
    }

    #[test]
    fn export_snapshot_field_names_are_stable() {
        assert_eq!(EXPORT_MANIFEST_FIELD, "manifest_rs");
        assert_eq!(EXPORT_DB_BYTES_FIELD, "db_bytes");
    }

    #[test]
    fn synced_export_error_allows_empty_pending_queue() {
        assert_eq!(synced_export_error(0, 4), None);
    }

    #[test]
    fn synced_export_error_reports_pending_and_expected_counts() {
        assert_eq!(
            synced_export_error(2, 4).expect("error"),
            "replica store export requires synced state (pending 2/4)"
        );
    }

    #[test]
    fn export_manifest_summary_preserves_versions_and_counts() {
        let summary = export_manifest_summary(&manifest());
        assert_eq!(summary.export_version, "1");
        assert_eq!(summary.replica_store_version, "0.1.0");
        assert_eq!(summary.backup_format_version, "1");
        assert_eq!(summary.schema_hash, "schema-hash");
        assert_eq!(summary.schema_table_count, 0);
        assert_eq!(summary.migration_count, 0);
        assert_eq!(summary.table_count_count, 0);
    }
}
