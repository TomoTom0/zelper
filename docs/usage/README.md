# usage 索引

zelperの使い方・agent向け配布物。`zelper docs` サブコマンドの出力正本（インストール済みバイナリに埋め込み。repoが無い環境からでも取り出せる）。

## 配布物

| ファイル | 出力コマンド | 内容 | 対象 |
|---|---|---|---|
| `llm.md` | `zelper docs llm usage` | LLM向けusage参照（全verb文法・排他規則・JSON契約・error class/exit対応・誤用対） | LLM/agent（全tool） |
| `skill/SKILL.md` | `zelper docs llm skill` | Agent Skills形式skill | Claude Code / Codex / Copilot / Gemini CLI / Cursor 等（Agent Skills準拠） |
| `snippet.md` | `zelper docs llm snippet` | AGENTS.md / CLAUDE.md追記用の最小ガイド | AGENTS.md対応tool（Claude CodeはCLAUDE.md経由） |
| repo root `README.md` | `zelper docs readme` | 人間向け完全usage・互換性・制限 | 全員 |

skillが使える環境ではsnippetは不要（snippetはskillの最小版）。llm.mdはskill/snippetがカバーしない機械的詳細（JSON envelopeの全フィールド・error class全種・排他規則全リスト）を含む完全参照。

## 関連

- `distribution.md` — 適用手順（ユーザー実行）・形式比較の結論・保守規則
