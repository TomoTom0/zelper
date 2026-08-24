# 配布形式と適用・保守

coding agentがzelperを文法エラー・誤用なく使いこなせるための配布物の、適用手順・形式比較の結論・保守規則。配布物一覧と出力コマンドの対応は `README.md`（索引）を参照。適用はユーザー自身が行う（agentによる利用環境への直接配置はしない）。

## 適用手順（ユーザー実行）

```bash
# Claude Code 個人環境（全プロジェクトで有効）
mkdir -p ~/.claude/skills/zelper
zelper docs llm skill > ~/.claude/skills/zelper/SKILL.md

# Claude Code プロジェクト環境（そのrepoのみ）
mkdir -p .claude/skills/zelper
zelper docs llm skill > .claude/skills/zelper/SKILL.md

# AGENTS.md への追記（snippet。AGENTS.md対応tool）
zelper docs llm snippet >> AGENTS.md

# CLAUDE.md への追記（snippet。Claude Code）
zelper docs llm snippet >> CLAUDE.md

# 参照（LLM向け完全usage / 人間向けREADME）
zelper docs llm usage
zelper docs readme
```

zelperをインストールしていない環境では、repoから直接コピーしてもよい:

```bash
cp -r docs/usage/skill ~/.claude/skills/zelper
```

skill適用後の確認: Claude Codeでskill一覧に `zelper` が表示されること（descriptionのtrigger語: zellij, pane, 多pane, remap等で自律発動）。

## 形式比較の結論（2026-08-24、TASK-24/25）

| 候補 | 判断 | 理由 |
|---|---|---|
| Agent Skills形式 | 採用（配布物の形式） | 主要tool広範対応。progressive disclosure（description常駐 -> SKILL.md -> 詳細）でcontext効率が良い。配布・version管理が容易 |
| AGENTS.md / CLAUDE.md snippet | 採用（補助） | skill非対応tool向けの最小経路。Claude CodeはAGENTS.md非対応のためCLAUDE.md経由 |
| zelper docsサブコマンド | 採用（配布経路） | 配布物の取り出し経路。インストール済みバイナリから常に正本どおりの内容を出力でき、repoのcloneを前提にしない。`completion` に次ぐ文書化された非verb例外としてDD-1の例外原則に追加。llm配下にusage/skill/snippetの構造を持つ |
| llms.txt | 不採用 | website（docs site）を持たないため適用不能。repo内markdownで同型の構造原則は既にREADMEが満たす |

詳細な調査記録（各標準の仕様・対応tool一覧・出典）: `docs/research/research_agent-docs-formats.md`

## 保守

- `llm.md`・`skill/SKILL.md`・`snippet.md` の内容はCLI文法（README.mdのcommand章・`src/cli.rs`・`src/error.rs`・`src/output/json.rs`）に追従させる。verb・option・exit status・error classを変更した際は全配布物（およびREADME.md）を更新する。正本はdocs/usage/配下で、`src/app/docs.rs` がinclude_str!で埋め込むため、正本を編集すれば再buildで `zelper docs` 出力も追従する
- snippet.mdは配布本体のみを置く（適用方法等のメタ説明はこの文書とREADME索引に集約。`zelper docs llm snippet` の出力がそのまま追記可能であることを保つ）
- 配布物を更新したら、適用済み環境にも再適用が必要（適用はユーザー実行）
