use super::validate_all_conformance_vectors;
use std::{fs, path::Path};

const VECTOR: &str = r#"{"suite":"inventory-control","contract_version":"1.0.0","vectors":[{"id":"inventory-control-001","kind":"synthetic.operation","input":{},"expected":{}}]}"#;

fn vectors_root(root: &Path) -> std::path::PathBuf {
    root.join("contracts/conformance/vectors")
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("disposable inventory root");
    fs::create_dir_all(vectors_root(root.path())).expect("fixed vector scaffold");
    root
}

fn write_vector(path: &Path) {
    fs::write(path, VECTOR).expect("synthetic vector");
}

#[test]
fn valid_nested_regular_vectors_and_ignored_files_validate() {
    let root = fixture();
    let vectors = vectors_root(root.path());
    fs::create_dir(vectors.join("a")).expect("first vector directory");
    fs::create_dir(vectors.join("z")).expect("last vector directory");
    write_vector(&vectors.join("a/first.json"));
    write_vector(&vectors.join("middle.json"));
    write_vector(&vectors.join("z/last.json"));
    fs::write(vectors.join("README.txt"), "ordinary ignored file").expect("ignored file");

    validate_all_conformance_vectors(root.path(), "1.0.0").expect("valid nested inventory");

    assert_eq!(
        fs::read_to_string(vectors.join("middle.json")).unwrap(),
        VECTOR
    );
}

#[test]
fn lexical_depth_first_error_order_is_preserved() {
    let root = fixture();
    let vectors = vectors_root(root.path());
    fs::create_dir(vectors.join("a")).expect("first vector directory");
    for relative in ["z.json", "b.json", "a/first.json"] {
        fs::write(
            vectors.join(relative),
            VECTOR.replace("inventory-control", ""),
        )
        .expect("invalid synthetic vector");
    }

    let error = validate_all_conformance_vectors(root.path(), "1.0.0")
        .expect_err("invalid inventory must fail");
    assert!(error.contains("a/first.json"), "{error}");
    assert!(error.contains("suite must not be empty"), "{error}");
}

#[cfg(unix)]
fn assert_link_rejected(root: &Path, link: &Path) {
    let error = validate_all_conformance_vectors(root, "1.0.0")
        .expect_err("linked conformance inventory must be rejected");
    assert!(error.contains("symlink"), "{error}");
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn linked_vector_file_is_rejected_without_changing_its_target() {
    let root = fixture();
    let foreign = root.path().join("foreign.json");
    write_vector(&foreign);
    let link = vectors_root(root.path()).join("linked.json");
    std::os::unix::fs::symlink(&foreign, &link).expect("fixture file link");

    assert_link_rejected(root.path(), &link);
    assert_eq!(fs::read_to_string(&foreign).unwrap(), VECTOR);
    assert!(fs::symlink_metadata(&foreign).unwrap().is_file());
}

#[cfg(unix)]
#[test]
fn linked_vector_directory_is_rejected_without_entering_its_target() {
    let root = fixture();
    let foreign = root.path().join("foreign");
    fs::create_dir(&foreign).expect("foreign fixture directory");
    write_vector(&foreign.join("vector.json"));
    let link = vectors_root(root.path()).join("linked");
    std::os::unix::fs::symlink(&foreign, &link).expect("fixture directory link");

    assert_link_rejected(root.path(), &link);
    assert_eq!(
        fs::read_to_string(foreign.join("vector.json")).unwrap(),
        VECTOR
    );
    assert!(fs::symlink_metadata(&foreign).unwrap().is_dir());
}

#[cfg(unix)]
#[test]
fn linked_inventory_root_is_rejected_without_changing_its_target() {
    let root = fixture();
    let foreign = root.path().join("foreign");
    fs::create_dir(&foreign).expect("foreign fixture directory");
    write_vector(&foreign.join("vector.json"));
    let link = vectors_root(root.path());
    fs::remove_dir(&link).expect("empty fixture vector directory");
    std::os::unix::fs::symlink(&foreign, &link).expect("fixture root link");

    assert_link_rejected(root.path(), &link);
    assert_eq!(
        fs::read_to_string(foreign.join("vector.json")).unwrap(),
        VECTOR
    );
}

#[cfg(unix)]
#[test]
fn linked_inventory_parent_is_rejected_without_changing_its_target() {
    let root = fixture();
    write_vector(&vectors_root(root.path()).join("vector.json"));
    let link = root.path().join("contracts");
    let foreign = root.path().join("foreign");
    fs::rename(&link, &foreign).expect("move owned fixture scaffold");
    std::os::unix::fs::symlink(&foreign, &link).expect("fixture parent link");

    assert_link_rejected(root.path(), &link);
    assert_eq!(
        fs::read_to_string(foreign.join("conformance/vectors/vector.json")).unwrap(),
        VECTOR
    );
}

#[cfg(unix)]
#[test]
fn dangling_ignored_suffix_link_is_rejected_without_repairing_it() {
    let root = fixture();
    write_vector(&vectors_root(root.path()).join("vector.json"));
    let missing = root.path().join("absent");
    let link = vectors_root(root.path()).join("ignored.txt");
    std::os::unix::fs::symlink(&missing, &link).expect("dangling fixture link");

    assert_link_rejected(root.path(), &link);
    assert!(!missing.exists());
    assert_eq!(fs::read_link(&link).unwrap(), missing);
}

#[cfg(unix)]
#[test]
fn nonregular_vector_is_rejected_before_reading_it() {
    let root = fixture();
    let socket = vectors_root(root.path()).join("socket.json");
    let short_socket = root.path().join("s");
    let _listener =
        std::os::unix::net::UnixListener::bind(&short_socket).expect("disposable socket fixture");
    fs::rename(&short_socket, &socket).expect("move owned socket into vector inventory");

    let error = validate_all_conformance_vectors(root.path(), "1.0.0")
        .expect_err("nonregular vector must fail admission");
    assert!(error.contains("regular file"), "{error}");
    use std::os::unix::fs::FileTypeExt;
    assert!(
        fs::symlink_metadata(&socket)
            .unwrap()
            .file_type()
            .is_socket()
    );
}
