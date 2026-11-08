//! The committed skill and `fanctl instructions` are renders of the
//! templates under `.ctl/operator/`, with the command table taken from the
//! Clap grammar. Regenerate both with `UPDATE_OPERATOR_DOCS=1`.

use std::fs;
use std::path::Path;

use fanctl::cli::Cli;
use minijinja::{Value, context};

const CONTRACT: &str = ".ctl/operator/contract.md.jinja";

fn crate_file(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn render(template: &str, context: &Value) -> String {
    let mut environment = ctl_core::surface::environment()
        .unwrap_or_else(|error| panic!("operator environment: {error}"));
    environment
        .add_template_owned("fanctl/contract.md.jinja", crate_file(CONTRACT))
        .unwrap_or_else(|error| panic!("parse {CONTRACT}: {error}"));
    environment
        .template_from_named_str(template, &crate_file(template))
        .unwrap_or_else(|error| panic!("parse {template}: {error}"))
        .render(context)
        .unwrap_or_else(|error| panic!("render {template}: {error}"))
}

fn assert_committed(relative: &str, rendered: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    if std::env::var_os("UPDATE_OPERATOR_DOCS").is_some() {
        fs::write(&path, rendered).unwrap_or_else(|error| panic!("write {relative}: {error}"));
    }
    assert_eq!(rendered, crate_file(relative), "{relative} is stale");
}

fn surface() -> ctl_core::Surface {
    ctl_core::Surface::new::<Cli>("fan")
}

#[test]
fn the_skill_is_the_committed_render() {
    let rendered = render(
        ".ctl/operator/SKILL.md.jinja",
        &context! { surface => surface(), version => env!("CARGO_PKG_VERSION") },
    );
    assert_committed("skills/fanctl/SKILL.md", &rendered);
}

#[test]
fn the_instructions_are_the_committed_render() {
    let rendered = render(
        ".ctl/operator/instructions.md.jinja",
        &context! { surface => surface() },
    );
    assert_committed("src/instructions.md", &rendered);
}
