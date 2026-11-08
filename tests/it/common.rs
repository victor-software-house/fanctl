//! Starting the fanctl binary, fixture trees, and a model of the driver.

pub(crate) mod driver;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ctl_core::fixture::{Root, Suite};

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

/// A writable copy of a fixture tree below `testdata/sysfs/`, as one
/// scenario of `suite`, removed when the returned root drops.
pub(crate) fn sysfs_copy<'suite>(suite: &'suite Suite, name: &str) -> Root<'suite> {
    let root = suite.scenario(name);
    copy_tree(&testdata(&format!("sysfs/{name}")), Path::new(""), &root);
    root
}

/// Copy every file below `from`/`relative` to the same place in `root`.
fn copy_tree(from: &Path, relative: &Path, root: &Root<'_>) {
    let dir = from.join(relative);
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|error| panic!("list {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("list {}: {error}", dir.display()));
        let target = relative.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(from, &target, root);
        } else {
            let body = fs::read(entry.path())
                .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
            root.file(&target, body);
        }
    }
}
