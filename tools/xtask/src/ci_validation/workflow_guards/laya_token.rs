//! Where the Laya smoke's Hugging Face token may travel.
//!
//! The token only serves the model download in `restore-test-model`. Steps of
//! a composite action inherit the `env` of the step that calls it, so binding
//! the token on the caller would expose it to every step inside the action
//! chain (the `prepare-automation` cargo build, third-party actions and the
//! product run included). Instead the caller passes it as the `hf_token`
//! input, each action forwards it as an input, and only the download step
//! binds it in its `env`.
use super::{Node, handoffs as h};
use crate::command::DynResult;
use std::path::Path;

const LAYA: &str = ".github/actions/run-laya-product-smoke/action.yml";
const SMOKE_INPUTS: &str = ".github/actions/restore-smoke-inputs/action.yml";
const TEST_MODEL: &str = ".github/actions/restore-test-model/action.yml";
const FORWARD: &str = "${{ inputs.hf_token }}";

pub(super) fn check(root: &Path) -> DynResult<()> {
    let load = |path: &str| -> DynResult<Node> {
        Ok(super::workflow_yaml::parse(&std::fs::read_to_string(
            root.join(path),
        )?)?)
    };
    action_chain(&load(LAYA)?, &load(SMOKE_INPUTS)?, &load(TEST_MODEL)?)
}

fn action_chain(laya: &Node, smoke_inputs: &Node, test_model: &Node) -> DynResult<()> {
    forwards(
        laya,
        "id",
        "restore",
        "./.github/actions/restore-smoke-inputs",
    )?;
    forwards(
        smoke_inputs,
        "id",
        "resolve-model",
        "./.github/actions/restore-test-model",
    )?;
    declares_input(test_model)?;
    let steps = h::steps(h::member(test_model, "runs")?)?;
    let (download, step) = h::step(steps, "name", "Download integration model")?;
    h::binding(h::member(step, "env")?, "MODEL_HF_TOKEN", FORWARD)?;
    only_step(steps, download, "restore-test-model")
}

// The action declares an optional, empty-by-default `hf_token` input, hands it
// to `target` through `with:` on one step, and no step mentions it otherwise.
fn forwards(action: &Node, key: &str, value: &str, target: &str) -> DynResult<()> {
    declares_input(action)?;
    let steps = h::steps(h::member(action, "runs")?)?;
    let (index, step) = h::step(steps, key, value)?;
    h::binding(step, "uses", target)?;
    h::binding(h::member(step, "with")?, "hf_token", FORWARD)?;
    let rest = step.entries().iter().any(|(name, node)| {
        if name == "with" {
            node.entries()
                .iter()
                .any(|(input, node)| input != "hf_token" && (leaks(input) || mentions(node)))
        } else {
            leaks(name) || mentions(node)
        }
    });
    if rest {
        return Err(format!("{target}: the HF token may only be forwarded as hf_token").into());
    }
    only_step(steps, index, target)
}

fn declares_input(action: &Node) -> DynResult<()> {
    let input = h::member(h::member(action, "inputs")?, "hf_token")?;
    h::binding(input, "required", "false")?;
    h::binding(input, "default", "")
}

fn only_step(steps: &[Node], allowed: usize, action: &str) -> DynResult<()> {
    if steps
        .iter()
        .enumerate()
        .any(|(index, step)| index != allowed && mentions(step))
    {
        return Err(format!("{action}: the HF token reaches a step that does not need it").into());
    }
    Ok(())
}

fn leaks(text: &str) -> bool {
    let text = text.to_ascii_uppercase();
    text.contains("HF_TOKEN") || text.contains("HUGGING_FACE_HUB_TOKEN")
}

fn mentions(node: &Node) -> bool {
    match node {
        Node::Scalar(text) => leaks(text),
        Node::Seq(items) => items.iter().any(mentions),
        Node::Map(entries) => entries
            .iter()
            .any(|(name, node)| leaks(name) || mentions(node)),
    }
}

/// A Laya smoke job passes the token only as the action step's `hf_token`
/// input. Rows without a token secret (Windows) pass none.
pub(super) fn caller_scope(
    job: &Node,
    steps: &[Node],
    invoke: usize,
    token: bool,
) -> DynResult<()> {
    if job.get("env").is_some_and(mentions) {
        return Err("Laya smoke must not bind HF_TOKEN on the whole job".into());
    }
    let action = &steps[invoke];
    if action.get("env").is_some_and(mentions) {
        return Err("Laya smoke must pass HF_TOKEN as the hf_token input, not env".into());
    }
    if steps
        .iter()
        .enumerate()
        .any(|(index, step)| index != invoke && mentions(step))
    {
        return Err("Laya smoke HF_TOKEN must reach only the Laya smoke action step".into());
    }
    let inputs = h::member(action, "with")?;
    let rest = inputs
        .entries()
        .iter()
        .any(|(input, node)| input != "hf_token" && (leaks(input) || mentions(node)));
    if rest {
        return Err("Laya smoke HF_TOKEN may only be passed as hf_token".into());
    }
    match (token, inputs.get("hf_token")) {
        (true, _) => h::binding(inputs, "hf_token", "${{ secrets.HF_TOKEN }}"),
        (false, None) => Ok(()),
        (false, Some(_)) => Err("this Laya smoke row has no HF token to pass".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Node {
        super::super::workflow_yaml::parse(source).unwrap()
    }
    fn chain() -> [Node; 3] {
        [
            parse(include_str!(
                "../../../../../.github/actions/run-laya-product-smoke/action.yml"
            )),
            parse(include_str!(
                "../../../../../.github/actions/restore-smoke-inputs/action.yml"
            )),
            parse(include_str!(
                "../../../../../.github/actions/restore-test-model/action.yml"
            )),
        ]
    }
    fn admitted(nodes: &[Node; 3]) -> bool {
        action_chain(&nodes[0], &nodes[1], &nodes[2]).is_ok()
    }
    fn step_mut<'a>(action: &'a mut Node, key: &str, value: &str) -> &'a mut Node {
        let Node::Seq(steps) = h::mutable(h::mutable(action, "runs"), "steps") else {
            unreachable!()
        };
        steps
            .iter_mut()
            .find(|step| super::super::field(step, key) == Some(value))
            .unwrap()
    }
    fn bind(step: &mut Node, section: &str, key: &str, value: &str) {
        let Node::Map(fields) = step else {
            unreachable!()
        };
        if !fields.iter().any(|(name, _)| name == section) {
            fields.push((section.into(), Node::Map(Vec::new())));
        }
        let Node::Map(entries) = h::mutable(step, section) else {
            unreachable!()
        };
        entries.retain(|(name, _)| name != key);
        entries.push((key.into(), Node::Scalar(value.into())));
    }
    fn unbind(step: &mut Node, section: &str, key: &str) {
        let Node::Map(entries) = h::mutable(step, section) else {
            unreachable!()
        };
        entries.retain(|(name, _)| name != key);
    }

    #[test]
    fn token_travels_as_an_input_down_to_the_download_step() {
        assert!(admitted(&chain()));
    }

    #[test]
    fn token_cannot_reach_another_step_of_the_chain() {
        for (action, key, value) in [
            (0, "name", "Run Laya startup and golden reads"),
            (0, "name", "Validate bounded Laya smoke inputs"),
            (1, "name", "Prepare hosted smoke automation"),
            (1, "name", "Prepare external Python SDK source"),
            (1, "name", "Extract and verify composed product"),
            (2, "name", "Check restored integration model"),
            (2, "name", "Verify integration model integrity"),
        ] {
            for (section, name, binding) in [
                ("env", "HF_TOKEN", FORWARD),
                ("env", "TOKEN", FORWARD),
                ("with", "token", FORWARD),
            ] {
                let mut nodes = chain();
                bind(
                    step_mut(&mut nodes[action], key, value),
                    section,
                    name,
                    binding,
                );
                assert!(!admitted(&nodes), "{value}: {section}.{name}");
            }
        }
    }

    #[test]
    fn handoff_steps_cannot_bind_the_token_in_env_or_drop_it() {
        for (action, id) in [(0, "restore"), (1, "resolve-model")] {
            let mut nodes = chain();
            bind(
                step_mut(&mut nodes[action], "id", id),
                "env",
                "HF_TOKEN",
                FORWARD,
            );
            assert!(!admitted(&nodes), "{id}: env binding");
            let mut nodes = chain();
            unbind(step_mut(&mut nodes[action], "id", id), "with", "hf_token");
            assert!(!admitted(&nodes), "{id}: dropped handoff");
            let mut nodes = chain();
            bind(
                step_mut(&mut nodes[action], "id", id),
                "with",
                "hf_token",
                "${{ secrets.HF_TOKEN }}",
            );
            assert!(!admitted(&nodes), "{id}: secret instead of the input");
        }
        let mut nodes = chain();
        unbind(
            step_mut(&mut nodes[2], "name", "Download integration model"),
            "env",
            "MODEL_HF_TOKEN",
        );
        assert!(!admitted(&nodes), "download step lost the token");
    }

    #[test]
    fn hf_token_input_stays_optional_and_empty_by_default() {
        for action in 0..3 {
            for (field, value) in [("required", "true"), ("default", "${{ github.token }}")] {
                let mut nodes = chain();
                h::replace(
                    &mut nodes[action],
                    &["inputs", "hf_token", field],
                    Node::Scalar(value.into()),
                );
                assert!(!admitted(&nodes), "action {action}: {field}");
            }
        }
    }
}
