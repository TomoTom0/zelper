// L3: CLI契約テスト（docs verb。TASK-24/25）
// docs verbはzellij backendに触れない純粋出力のため、fake zellij不要。
// 正本（README.md・docs/usage/配下）と`zelper docs`出力の一致を検証する。
use assert_cmd::Command;

fn manifest_docs(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

// [covers:docs-verb.readme-outputs-readme-md]
#[test]
fn docs_readme_outputs_readme_md() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "readme"])
        .assert()
        .success()
        .stdout(manifest_docs("README.md"));
}

// [covers:docs-verb.llm-usage-outputs-usage-md]
#[test]
fn docs_llm_usage_outputs_usage_md() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "llm", "usage"])
        .assert()
        .success()
        .stdout(manifest_docs("docs/usage/llm.md"));
}

// [covers:docs-verb.llm-skill-outputs-skill-md]
#[test]
fn docs_llm_skill_outputs_skill_md() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "llm", "skill"])
        .assert()
        .success()
        .stdout(manifest_docs("docs/usage/skill/SKILL.md"));
}

// [covers:docs-verb.llm-snippet-outputs-snippet-md]
#[test]
fn docs_llm_snippet_outputs_snippet_md() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "llm", "snippet"])
        .assert()
        .success()
        .stdout(manifest_docs("docs/usage/snippet.md"));
}

// [covers:docs-verb.without-subcommand-is-usage-error]
#[test]
fn docs_without_subcommand_is_usage_error() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs"])
        .assert()
        .failure()
        .code(2);
}

// [covers:docs-verb.llm-without-resource-is-usage-error]
#[test]
fn docs_llm_without_resource_is_usage_error() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "llm"])
        .assert()
        .failure()
        .code(2);
}

// [covers:docs-verb.unknown-resource-is-usage-error]
#[test]
fn docs_unknown_resource_is_usage_error() {
    Command::cargo_bin("zelper")
        .unwrap()
        .args(["docs", "llm", "bogus"])
        .assert()
        .failure()
        .code(2);
}
