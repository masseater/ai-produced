use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn song() -> PathBuf {
    repository().join("songs/001-mou-ikkai")
}

pub(crate) fn assert_same_tree(actual: &Path, expected: &Path) {
    for entry in fs::read_dir(expected).expect("expected dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let other = actual.join(entry.file_name());
        if path.is_dir() {
            assert_same_tree(&other, &path);
        } else {
            assert!(
                fs::read(&other).expect("generated file") == fs::read(&path).expect("committed"),
                "{} differs from {}",
                other.display(),
                path.display()
            );
        }
    }
}
