//! Execute the compute-changes output block and parse it as the runner does.
use super::workflows::bash;
use std::{collections::BTreeMap, fs};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.github/actions/compute-changes/derive-outputs.sh"
));

const SCALARS: [(&str, &str); 18] = [
    ("affected_crates", "AFFECTED_CRATES"),
    ("test_crates", "TEST_CRATES"),
    ("all_rust", "ALL_RUST"),
    ("ui_changed", "UI_CHANGED"),
    ("website_changed", "WEBSITE_CHANGED"),
    ("website_docs_changed", "WEBSITE_DOCS_CHANGED"),
    ("cli_surface_changed", "CLI_SURFACE_CHANGED"),
    ("docs_only", "DOCS_ONLY"),
    ("rust_changed", "RUST_CHANGED"),
    ("backend_changed", "BACKEND_CHANGED"),
    ("inference_artifact_required", "INFERENCE_ARTIFACT_REQUIRED"),
    ("backend_recipe_changed", "BACKEND_RECIPE_CHANGED"),
    ("windows_cpu_build_required", "WINDOWS_CPU_BUILD_REQUIRED"),
    ("windows_gpu_build_required", "WINDOWS_GPU_BUILD_REQUIRED"),
    ("sdk_smoke_required", "SDK_SMOKE_REQUIRED"),
    ("runner_contract_required", "RUNNER_CONTRACT_REQUIRED"),
    ("linux_test_groups_json", "LINUX_TEST_GROUPS_JSON"),
    ("changed_files", "CHANGED_FILES"),
];

fn output_block() -> String {
    let begin = "# Set outputs using GITHUB_OUTPUT";
    let (_, remainder) = SOURCE.split_once(begin).expect("output block start");
    format!("{begin}{remainder}")
}

/// Parse a `$GITHUB_OUTPUT` file with the runner's `name=value` and
/// `name<<delimiter` ... `delimiter` rules, rejecting malformed lines and
/// repeated names.
fn parse(contents: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut lines = contents.lines();
    while let Some(line) = lines.next() {
        let (name, value) = if let Some((name, delimiter)) = line.split_once("<<") {
            assert!(!name.contains('='), "malformed heredoc line {line:?}");
            let mut value = Vec::new();
            loop {
                let next = lines
                    .next()
                    .unwrap_or_else(|| panic!("unterminated {name} value"));
                if next == delimiter {
                    break;
                }
                value.push(next);
            }
            (name, value.join("\n"))
        } else {
            let (name, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed output line {line:?}"));
            (name, value.to_owned())
        };
        assert!(
            fields.insert(name.to_owned(), value).is_none(),
            "repeated output {name}"
        );
    }
    fields
}

fn run(changed_files: &str) -> BTreeMap<String, String> {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("github_output");
    fs::write(&output, b"").unwrap();
    let output_path = output.to_str().unwrap();
    let mut values: Vec<(&str, String)> = SCALARS
        .iter()
        .filter(|(_, variable)| *variable != "CHANGED_FILES")
        .map(|(name, variable)| (*variable, format!("script-{name}")))
        .collect();
    values.push(("CHANGED_FILES", changed_files.to_owned()));
    values.push(("GITHUB_OUTPUT", output_path.to_owned()));
    let values: Vec<(&str, &str)> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (ok, _, error) = bash(root.path(), &output_block(), &values);
    assert!(ok, "{error}");
    parse(&fs::read_to_string(&output).unwrap())
}

fn expected(changed_files: &str) -> BTreeMap<String, String> {
    SCALARS
        .iter()
        .map(|(name, _)| {
            let value = if *name == "changed_files" {
                changed_files.to_owned()
            } else {
                format!("script-{name}")
            };
            ((*name).to_owned(), value)
        })
        .collect()
}

#[test]
fn changed_paths_that_look_like_output_commands_stay_inside_changed_files() {
    let changed = "EOF\nall_rust=forged\nx<<EOF\nnormal.rs";
    assert_eq!(run(changed), expected(changed));
}

#[test]
fn empty_change_list_still_writes_every_output() {
    assert_eq!(run(""), expected(""));
}
