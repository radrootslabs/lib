use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct EmbeddedMigration {
    version: u32,
    name: String,
    up_file: String,
    down_file: String,
}

fn append_governed_text_tree(path: &Path, source: &mut String) {
    if !path.exists() {
        return;
    }
    if path.is_file() {
        let extension = path.extension().and_then(|value| value.to_str());
        if matches!(extension, Some("json" | "rs" | "sha256" | "sql" | "toml")) {
            source.push_str(
                &fs::read_to_string(path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
            );
            source.push('\n');
        }
        return;
    }

    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .map(|entry| entry.expect("read governed source entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        append_governed_text_tree(&entry, source);
    }
}

fn embedded_migrations(source: &str) -> Vec<EmbeddedMigration> {
    let registry = source
        .split_once("pub(crate) const EVENT_STORE_MIGRATIONS: &[EventStoreMigration] = &[")
        .expect("event-store migration registry declaration")
        .1
        .split_once("\n];")
        .expect("event-store migration registry terminator")
        .0;
    let mut migrations = Vec::new();
    let mut version = None;
    let mut name = None;
    let mut up_file = None;
    let mut down_file = None;

    for line in registry.lines().map(str::trim) {
        if line == "EventStoreMigration {" {
            assert!(
                version.is_none() && name.is_none() && up_file.is_none() && down_file.is_none(),
                "event-store migration registry entries must not overlap"
            );
        } else if let Some(value) = line.strip_prefix("version: ") {
            version = Some(
                value
                    .trim_end_matches(',')
                    .parse()
                    .expect("event-store migration version must be a u32"),
            );
        } else if let Some(value) = line.strip_prefix("name: \"") {
            name = Some(
                value
                    .strip_suffix("\",")
                    .expect("event-store migration name literal")
                    .to_owned(),
            );
        } else if let Some(value) = line.strip_prefix("up_sql: include_str!(\"../migrations/") {
            up_file = Some(
                value
                    .strip_suffix("\"),")
                    .expect("event-store up migration include")
                    .to_owned(),
            );
        } else if let Some(value) = line.strip_prefix("down_sql: include_str!(\"../migrations/") {
            down_file = Some(
                value
                    .strip_suffix("\"),")
                    .expect("event-store down migration include")
                    .to_owned(),
            );
        } else if line == "}," && version.is_some() {
            migrations.push(EmbeddedMigration {
                version: version.take().expect("event-store migration version"),
                name: name.take().expect("event-store migration name"),
                up_file: up_file.take().expect("event-store up migration include"),
                down_file: down_file
                    .take()
                    .expect("event-store down migration include"),
            });
        }
    }

    assert!(
        version.is_none() && name.is_none() && up_file.is_none() && down_file.is_none(),
        "event-store migration registry has an incomplete entry"
    );
    assert!(
        !migrations.is_empty(),
        "event-store migration registry must not be empty"
    );
    migrations
}

#[test]
fn event_store_migration_files_exactly_match_the_embedded_registry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migration_source = fs::read_to_string(root.join("src/migrations.rs"))
        .expect("read event-store migration registry source");
    let migrations = embedded_migrations(&migration_source);
    let mut registry_files = BTreeSet::new();

    for migration in migrations {
        let identity = format!("{:04}_{}", migration.version, migration.name);
        assert_eq!(
            migration.up_file,
            format!("{identity}.up.sql"),
            "event-store up migration filename must match its registry version and name"
        );
        assert_eq!(
            migration.down_file,
            format!("{identity}.down.sql"),
            "event-store down migration filename must match its registry version and name"
        );
        assert!(
            registry_files.insert(migration.up_file),
            "event-store registry declares a migration file more than once"
        );
        assert!(
            registry_files.insert(migration.down_file),
            "event-store registry declares a migration file more than once"
        );
    }

    let migration_dir = root.join("migrations");
    let disk_files = fs::read_dir(&migration_dir)
        .expect("read event-store migration directory")
        .map(|entry| {
            let entry = entry.expect("read event-store migration entry");
            assert!(
                entry
                    .file_type()
                    .expect("read migration entry type")
                    .is_file(),
                "event-store migration directory may contain only regular files: {}",
                entry.path().display()
            );
            entry
                .file_name()
                .into_string()
                .expect("event-store migration filename must be UTF-8")
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        disk_files, registry_files,
        "event-store migration directory and embedded registry must contain the same SQL files"
    );
}

#[test]
fn event_store_schema_does_not_reintroduce_old_nostr_table_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = root
        .parent()
        .and_then(Path::parent)
        .expect("event-store crate must be inside the workspace crates directory");
    let mut source = String::new();
    for relative in ["src", "migrations", "contracts", "tests/fixtures"] {
        append_governed_text_tree(&root.join(relative), &mut source);
    }
    for relative in [
        "crates/event/src",
        "crates/event_codec/src",
        "contracts/conformance/vectors/event_store",
    ] {
        append_governed_text_tree(&workspace.join(relative), &mut source);
    }

    for forbidden in ["nostr_events", "nostr_event_tags", "nostr_event_head"] {
        assert!(
            !source.contains(forbidden),
            "radroots_event_store governed sources must not reintroduce old table name `{forbidden}`"
        );
    }

    for required in [
        "event_envelopes",
        "event_envelope_tags",
        "event_envelope_head",
        "radroots_event_store_source_generation",
        "radroots_event_store_nip09_request",
        "radroots_event_store_addressable_head_transition",
    ] {
        assert!(
            source.contains(required),
            "radroots_event_store governed sources must retain table name `{required}`"
        );
    }
}
