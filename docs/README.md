# zelper docs 索引

ドキュメント構成の管理対象はこのREADME。実態と常に一致させる。

## 構成

- `design/` — 設計
  - `first/260821/` — 初回ハンドオフパッケージ（requirements.md / basic-design.md / development-plan.md / README.md）
  - `requirements-traceability.md` — Phase 2/8。要件分類とP0→DD章マップ、制約と設計指示
  - `detailed-design.md` — Phase 3。DD-1〜DD-12（CLI grammar / domain / backend / output / 操作設計 / remap v2・resize algorithm / safety）
  - `design-review.md` — Phase 4 + TASK-31整備時レビュー。DR-1〜DR-10 / T31R-1〜9（設計）/ CR-1〜5（実装・codex）の指摘とdisposition
  - `conditions-migration.md` — TASK-29。テスト構成のtest-structure skill準拠（条件書方式）への移行設計（15条件書・covers tag・verify script・5段階適用）。移行完了（2026-09-13・TASK-58第4段。進捗はtests/design/README.md）
  - `remap-v2-matching-fix-task74.md` — TASK-74。remap適用経路のrun一致照合修正（DD-10 v2.1）の設計記録: 原因確定（spike S・実験a/b/cの証拠F1〜F8）・照合key変更（terminal_command）・default_tab_template反映・poll空応答retry・test-first計画・E2E acceptance手順
  - `remap-v3-multi-tab-task75.md` — TASK-75。remapのmulti-tab layout全体再現（DD-10 v2.2）の設計記録: 正規形TabTemplate列（反復単位=layout全体・S=sum(N_t)）・visual order逐次充填の累積slot配分則・tab名生成規則（anchor名保持例外）・tab/pane focus再現（focus-pane-id）・空groupのnew-tab経路・検証計画・E2E acceptance手順・実験記録（tmp/task75/）
- `research/` — 調査
  - `zellij-capabilities.md` — Phase 1成果物（E6・remap v2 L4まで反映）。zellij 0.44.3実機検証済み機能マトリクス、override-layout詳細、overflow実験結果、Plugin APIによるcross-tab移動の実証（E6）とremap v2実機検証（L4 S-v2-1〜6）、remap設計への帰結
  - `research_agent-docs-formats.md` — TASK-24成果物。agent向け配布形式の標準・仕様調査（AGENTS.md / Agent Skills / Claude Code skills / llms.txt。出典付き）
- `testing/` — テスト設計
  - `test-plan.md` — Phase 5（2026-09-13条件書移行後は縮小版）。テスト階層と実行環境（L1〜L4・podman構成）、領域別仕様の対応表（条件定義の正本はtests/design/の条件書・R番号定義も移譲）、L4統合test（要件・harness・実施記録S1〜S11・S-v2-1〜6・S-v3-1〜5）、tests/構成、fail-first履歴
- `usage/` — zelperの使い方・agent向け配布物（`zelper docs`の出力正本）
  - `README.md` — 配布物索引（出力コマンドとの対応）
  - `distribution.md` — 適用手順（ユーザー実行）・形式比較の結論・保守規則
  - `llm.md` — LLM向けusage参照（`zelper docs llm usage`）
  - `skill/SKILL.md` — Agent Skills形式のskill配布物（`zelper docs llm skill`）
  - `snippet.md` — AGENTS.md / CLAUDE.md追記用snippet（`zelper docs llm snippet`）

## 規則

- docs/直下へのファイル配置は禁止。カテゴリディレクトリ配下に置く。新カテゴリはディレクトリ作成とこのREADMEの更新をセットで行う
- 命名: 恒久文書は`<kind>_<topic>.md`（timestampなし）、蓄積型は`<date>_<kind>_<topic>_<rel>.md`
- 例外: 外部標準がファイル名を規定する配布物（`usage/skill/SKILL.md`等）はその標準名を優先する。配布物はコピーして使う前提のため命名規則を適用しない
- development-plan.mdが成果物パスを明示的に規定する場合（`design/detailed-design.md`等）はそのパスを優先する
- 一時ファイル（検証スクリプト・実験ログ等）はdocs/ではなく`tmp/`へ。Phase 1の実験の一次記録はrepo管理外の作業dirに残置（成果は`research/zellij-capabilities.md`に集約）
