//! ドキュメント配布物の出力（文書化されたverb原則の例外）。
//! 正本はREADME.mdとdocs/usage/配下。include_str!でバイナリに埋め込み、repoが無い環境からでも取り出せる。

use crate::cli::{DocsTarget, LlmDocsResource};

const README_MD: &str = include_str!("../../README.md");
const LLM_USAGE_MD: &str = include_str!("../../docs/usage/llm.md");
const SKILL_MD: &str = include_str!("../../docs/usage/skill/SKILL.md");
const SNIPPET_MD: &str = include_str!("../../docs/usage/snippet.md");

pub fn run(target: &DocsTarget) {
    let text = match target {
        DocsTarget::Readme => README_MD,
        DocsTarget::Llm { resource } => match resource {
            LlmDocsResource::Usage => LLM_USAGE_MD,
            LlmDocsResource::Skill => SKILL_MD,
            LlmDocsResource::Snippet => SNIPPET_MD,
        },
    };
    print!("{text}");
}
