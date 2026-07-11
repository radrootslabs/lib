use std::fs;
use std::path::Path;

#[test]
fn generic_event_source_does_not_reintroduce_old_core_nostr_event_names() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = read_sources(&src);

    for forbidden in [
        "RadrootsNostrEvent",
        "RadrootsNostrEventRef",
        "RadrootsNostrEventPtr",
        "RadrootsFrozenEventDraft",
        "RadrootsSignedNostrEvent",
        "RadrootsSignedNostrEventParts",
    ] {
        assert!(
            !source.contains(forbidden),
            "radroots_event generic source must not expose old core event name `{forbidden}`"
        );
    }
}

fn read_sources(root: &Path) -> String {
    let mut source = String::new();
    for entry in fs::read_dir(root).expect("read source directory") {
        let entry = entry.expect("source entry");
        let path = entry.path();
        if path.is_dir() {
            source.push_str(read_sources(&path).as_str());
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            source.push_str(fs::read_to_string(path).expect("read source file").as_str());
        }
    }
    source
}
