use super::support::TempDir;
use crate::storage::format::LOCK_FILE_NAME;
use crate::storage::store::Storage;

#[test]
fn storage_open_creates_root_and_owner_directories() {
    let root = TempDir::new();
    let storage = Storage::open(root.path()).unwrap();
    assert_eq!(storage.root(), root.path());

    let ops = storage.directory("operations").unwrap();
    assert_eq!(ops, root.path().join("operations"));
    assert!(ops.is_dir());
    assert!(root.path().join(LOCK_FILE_NAME).is_file());
}

#[test]
fn storage_rejects_unsafe_owner_names() {
    let root = TempDir::new();
    let storage = Storage::open(root.path()).unwrap();
    assert!(storage.directory("").is_err());
    assert!(storage.directory("../escape").is_err());
    assert!(storage.directory("ops/extra").is_err());
}

#[test]
fn storage_clone_shares_root() {
    let root = TempDir::new();
    let storage = Storage::open(root.path()).unwrap();
    let cloned = storage.clone();
    let path = cloned.directory("network").unwrap();
    assert!(path.starts_with(root.path()));
}

#[test]
fn reopen_removes_only_recognized_abandoned_publications() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let root = TempDir::new();
    let external = TempDir::new();
    let storage = Storage::open(root.path()).unwrap();
    let hash = "a".repeat(64);
    let abandoned = [
        format!("operations/mta_bus/blocks/.{hash}.12-0.part"),
        "operations/mta_bus/.journal.12-1.part".into(),
        "operations/mta_bus/checkpoint/.manifest.trst.12-2.part".into(),
        "operations/mta_bus/checkpoint/generations/3/.global.trst.12-3.part".into(),
        "operations/mta_bus/archive/.manifest.trst.12-4.part".into(),
        format!("alerts/records/.{hash}.12-5.part"),
        "alerts/.manifest.cbor.zst.12-6.part".into(),
        "alerts/.journal.12-7.part".into(),
        "alerts/sources/mta_bus/.index-g3.cbor.zst.12-8.part".into(),
        "alerts/sources/mta_bus/pages/.af-g3-0.cbor.zst.12-9.part".into(),
        "network/.current.trst.12-10.part".into(),
        format!("network/blobs/.{hash}.trst.12-11.part"),
        format!("network/shapes/mta_bus/.{hash}.trst.12-12.part"),
        "network/publications/.draft.build/.publication.trst.12-13.part".into(),
        "network/publications/.draft.build/sources/mta_bus/.manifest.trst.12-14.part".into(),
    ];
    for path in &abandoned {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"abandoned").unwrap();
    }
    let preserved = [
        format!("operations/mta_bus/blocks/{hash}.trsb"),
        "operations/mta_bus/blocks/.unknown.12-1.part".into(),
        "operations/mta_bus/blocks/.aaaaaaaa.12-1.part".into(),
        "network/.current.trst.x-1.part".into(),
        "network/.unrelated.12-1.part".into(),
        "network/other/.current.trst.12-1.part".into(),
        "other/.journal.12-1.part".into(),
        "alerts/manifest.cbor.zst".into(),
    ];
    for path in &preserved {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"keep").unwrap();
    }
    let outside_temp = external.path().join(".journal.12-1.part");
    fs::write(&outside_temp, b"outside").unwrap();
    symlink(external.path(), root.path().join("operations/njt_bus")).unwrap();
    let link = root.path().join("alerts/.journal.12-20.part");
    symlink(&outside_temp, &link).unwrap();
    drop(storage);

    let _reopened = Storage::open(root.path()).unwrap();
    for path in &abandoned {
        assert!(!root.path().join(path).exists(), "{path}");
    }
    for path in &preserved {
        assert_eq!(fs::read(root.path().join(path)).unwrap(), b"keep", "{path}");
    }
    assert_eq!(fs::read(&outside_temp).unwrap(), b"outside");
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
}

#[test]
fn rejected_second_open_preserves_active_publication() {
    let root = TempDir::new();
    let storage = Storage::open(root.path()).unwrap();
    let dir = storage.directory("alerts").unwrap();
    let temp = dir.join(".journal.123-4.part");
    std::fs::write(&temp, b"active").unwrap();

    assert!(Storage::open(root.path()).is_err());
    assert_eq!(std::fs::read(temp).unwrap(), b"active");
}
