//! Where the smoke jobs' Hugging Face token may travel.
//!
//! The token only serves the model download in `restore-test-model`. A job
//! `env` binding would hand it to every step of the job (third-party actions,
//! client installs, the product run and cleanup included), so a job that
//! restores a model passes it only as the `hf_token` input of its
//! `restore-smoke-inputs` / `restore-test-model` steps. `laya_token` then
//! keeps it on the download step inside those actions.
use super::laya_token::{leaks, mentions};
use super::{Node, field, handoffs as h};
use crate::command::DynResult;
use std::collections::BTreeMap;

const ACTIONS: [&str; 2] = [
    "./.github/actions/restore-smoke-inputs",
    "./.github/actions/restore-test-model",
];
const SECRET: &str = "${{ secrets.HF_TOKEN }}";
/// Jobs that receive the token secret: each of their model restore steps
/// must hand it over.
const CARRIERS: [(&str, &str); 4] = [
    ("smoke.yml", "smoke_tests"),
    ("smoke.yml", "smoke_tests_macos"),
    ("scripted-binary-smoke.yml", "scripted_binary_smoke"),
    ("sdk-smoke.yml", "sdk_smoke"),
];

pub(super) fn check(workflows: &BTreeMap<String, Node>) -> DynResult<()> {
    for (workflow, job) in CARRIERS {
        let document = workflows
            .get(workflow)
            .ok_or_else(|| format!("missing smoke workflow {workflow}"))?;
        scope(h::job(document, job)?, true)
            .map_err(|error| format!("{workflow} {job}: {error}"))?;
    }
    for (workflow, document) in workflows {
        for (job, node) in document.get("jobs").map(Node::entries).unwrap_or_default() {
            if CARRIERS.contains(&(workflow.as_str(), job.as_str())) {
                continue;
            }
            let restores = h::steps(node).is_ok_and(|steps| steps.iter().any(restore));
            if restores {
                scope(node, false).map_err(|error| format!("{workflow} {job}: {error}"))?;
            }
        }
    }
    Ok(())
}

fn restore(step: &Node) -> bool {
    field(step, "uses").is_some_and(|uses| ACTIONS.contains(&uses))
}

/// The token is absent from the job `env` and from every step except the model
/// restore steps, which may carry it only as `with.hf_token`. A carrier job
/// passes it to each of them.
fn scope(job: &Node, carrier: bool) -> DynResult<()> {
    if job.get("env").is_some_and(mentions) {
        return Err("the HF token must not be bound on the whole job".into());
    }
    let steps = h::steps(job)?;
    if !steps.iter().any(restore) {
        return Err("no model restore step receives the HF token".into());
    }
    for step in steps {
        if !restore(step) {
            if mentions(step) {
                return Err("the HF token reaches a step that does not restore a model".into());
            }
            continue;
        }
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
            return Err("the HF token may only be passed as the hf_token input".into());
        }
        match (
            carrier,
            step.get("with").and_then(|with| with.get("hf_token")),
        ) {
            (true, _) => h::binding(h::member(step, "with")?, "hf_token", SECRET)?,
            (false, None) => {}
            (false, Some(_)) => {
                return Err("this job has no HF token secret to pass".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflows() -> BTreeMap<String, Node> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        std::fs::read_dir(root.join(".github/workflows"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_str().unwrap().to_owned(),
                    super::super::workflow_yaml::parse(&std::fs::read_to_string(path).unwrap())
                        .unwrap(),
                )
            })
            .collect()
    }
    fn job_mut<'a>(
        workflows: &'a mut BTreeMap<String, Node>,
        workflow: &str,
        job: &str,
    ) -> &'a mut Node {
        h::mutable(
            h::mutable(workflows.get_mut(workflow).unwrap(), "jobs"),
            job,
        )
    }
    fn steps_mut(job: &mut Node) -> &mut Vec<Node> {
        let Node::Seq(steps) = h::mutable(job, "steps") else {
            unreachable!()
        };
        steps
    }
    fn set(node: &mut Node, section: &str, key: &str, value: &str) {
        let Node::Map(fields) = node else {
            unreachable!()
        };
        if !fields.iter().any(|(name, _)| name == section) {
            fields.push((section.into(), Node::Map(Vec::new())));
        }
        let Node::Map(entries) = h::mutable(node, section) else {
            unreachable!()
        };
        entries.retain(|(name, _)| name != key);
        entries.push((key.into(), Node::Scalar(value.into())));
    }
    fn unset(node: &mut Node, section: &str, key: &str) {
        let Node::Map(entries) = h::mutable(node, section) else {
            unreachable!()
        };
        entries.retain(|(name, _)| name != key);
    }

    #[test]
    fn smoke_jobs_pass_the_token_only_to_model_restores() {
        check(&workflows()).unwrap();
    }

    #[test]
    fn token_cannot_return_to_the_job_env() {
        for (workflow, job) in CARRIERS {
            for key in ["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"] {
                let mut nodes = workflows();
                set(job_mut(&mut nodes, workflow, job), "env", key, SECRET);
                assert!(check(&nodes).is_err(), "{workflow} {job}: env.{key}");
            }
        }
    }

    #[test]
    fn token_cannot_reach_a_step_that_does_not_restore_a_model() {
        for (workflow, job) in CARRIERS {
            let count = steps_mut(job_mut(&mut workflows(), workflow, job)).len();
            for index in 0..count {
                let mut nodes = workflows();
                let step = &mut steps_mut(job_mut(&mut nodes, workflow, job))[index];
                if restore(step) {
                    continue;
                }
                set(step, "env", "HF_TOKEN", SECRET);
                assert!(check(&nodes).is_err(), "{workflow} {job}: step {index}");
            }
        }
    }

    #[test]
    fn restore_steps_hand_over_the_secret_as_hf_token_only() {
        for (workflow, job) in CARRIERS {
            let count = steps_mut(job_mut(&mut workflows(), workflow, job)).len();
            for index in 0..count {
                if !restore(&steps_mut(job_mut(&mut workflows(), workflow, job))[index]) {
                    continue;
                }
                let mutations: [&dyn Fn(&mut Node); 4] = [
                    &|step| unset(step, "with", "hf_token"),
                    &|step| set(step, "with", "hf_token", "${{ github.token }}"),
                    &|step| set(step, "env", "HF_TOKEN", SECRET),
                    &|step| set(step, "with", "model_url", SECRET),
                ];
                for (mutation, apply) in mutations.iter().enumerate() {
                    let mut nodes = workflows();
                    apply(&mut steps_mut(job_mut(&mut nodes, workflow, job))[index]);
                    assert!(
                        check(&nodes).is_err(),
                        "{workflow} {job}: step {index} mutation {mutation}"
                    );
                }
            }
        }
    }

    #[test]
    fn other_model_restoring_jobs_carry_no_token() {
        let mut nodes = workflows();
        let job = job_mut(&mut nodes, "ci-linux-runtime-slice.yml", "linux_runtime");
        let step = steps_mut(job)
            .iter_mut()
            .find(|step| restore(step))
            .unwrap();
        set(step, "with", "hf_token", SECRET);
        assert!(check(&nodes).is_err());
    }
}
