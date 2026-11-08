//! Starting the fanctl binary, fixture trees, and a model of the driver.

pub(crate) mod driver;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The colour variables an agent shell sets, removed from every child so the
/// suite passes there.
const COLOR_VARIABLES: [&str; 5] = [
    "CLICOLOR",
    "CLICOLOR_FORCE",
    "FORCE_COLOR",
    "NO_COLOR",
    "COLORTERM",
];

/// The binary under test, with a clean colour environment and no `FANCTL_`
/// settings inherited from the caller.
pub(crate) fn fanctl() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fanctl"));
    for name in COLOR_VARIABLES {
        command.env_remove(name);
    }
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("FANCTL_") {
            command.env_remove(name);
        }
    }
    command
}

/// A path below `testdata/`.
pub(crate) fn testdata(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(relative)
}

/// The text of a file below `testdata/`.
pub(crate) fn testdata_text(relative: &str) -> String {
    let path = testdata(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// A writable copy of a fixture tree below `testdata/sysfs/`, removed when
/// the returned directory drops.
pub(crate) fn sysfs_copy(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temporary directory: {error}"));
    copy_tree(&testdata(&format!("sysfs/{name}")), dir.path());
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
    let entries =
        fs::read_dir(from).unwrap_or_else(|error| panic!("list {}: {error}", from.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("list {}: {error}", from.display()));
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", entry.path().display()));
        }
    }
}
