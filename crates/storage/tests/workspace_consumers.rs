use std::{fs, path::PathBuf};

#[test]
fn ready_workspace_consumers_use_only_the_final_storage_boundary() {
    let storage_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = storage_root.parent().expect("crates directory");
    for (consumer, required) in [
        (
            "storage_sqlite",
            [
                "radroots_storage",
                "radroots_event_codec",
                "radroots_secrets",
            ]
            .as_slice(),
        ),
        (
            "sync",
            [
                "radroots_storage",
                "radroots_event",
                "radroots_event_codec",
                "radroots_protocol",
                "radroots_signing",
                "radroots_trade",
                "radroots_transport",
            ]
            .as_slice(),
        ),
    ] {
        let manifest = fs::read_to_string(workspace.join(consumer).join("Cargo.toml"))
            .expect("consumer manifest");
        for dependency in required {
            assert!(
                manifest.contains(&format!("{dependency} = {{ workspace = true")),
                "{consumer} does not use final dependency {dependency}"
            );
        }
        for legacy in [
            "radroots_event_store",
            "radroots_event_index",
            "radroots_outbox",
            "radroots_runtime_store",
        ] {
            assert!(
                !manifest.contains(legacy),
                "{consumer} still depends on legacy storage surface {legacy}"
            );
        }
    }
}
