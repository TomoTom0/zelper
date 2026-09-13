# zelper tests 索引

テスト構成の管理対象はこのREADME。**検証条件の正本は`tests/design/`のテスト設計条件書**（`<stem>.toml`・test-structure skill準拠）。条件id・given/expect・出典・excluded記録は各条件書と`tests/design/README.md`を参照。テスト階層・L4統合test・fail-first履歴は docs/testing/test-plan.md。

## 構成（file → 条件書の索引）

- `design/` — テスト設計条件書16file（正本）+ README.md（条件書一覧・R番号→条件id対応表・進捗・範囲明示）
- `unit/` — L1 純粋ロジック
  - `selector.rs` — selector.toml（対象解決。10条件）
  - `parser.rs` — backend-parser.toml（zellij出力parse。6条件）
  - `error_map.rs` — json-contract.tomlの一部（error class↔exit status対応表・JSON envelope。4条件分）
  - `layout_planner.rs` — layout-planner.toml（remap v2 planner grouping。15条件）
  - `layout_generator.rs` — layout-generator.toml（remap v2 生成KDL。8条件）
- `cli/` — L3 CLI契約（assert_cmd + fake zellij shim。shimはtest実行時に生成: argv記録・fixture応答・FAKE_* envで応答差し替え）
  - `list_read_send.rs` — cli-grammar（6）+ read-send（5）+ list-display（10）+ json-contract（2）の23条件
  - `remap.rs` — remap-cli.toml（8条件）
  - `docs.rs` — docs-verb.toml（7条件。`[[test]]`宣言必須: tests/直下以外の自動target化に注意）
- `fake_backend/` — L2 決定的fake backend
  - `fake.rs` — FakeBackend本体（状態持ち・呼び出し記録・失敗注入。test支援資産でtag対象外）
  - `rename_add_remove.rs` — rename.toml（3）+ add-remove.toml（7）の10条件
  - `resize.rs` — resize.toml（7条件）
  - `companion_seed.rs` — companion-seed.toml（14条件。tempdir実file）
  - `remap.rs` — remap-sequence.toml（20条件）
- `fixtures/zellij/` — 実zellij 0.44.3出力fixture（panes.json / tabs.json）

lib内test 9件（`src/app/list.rs` 3件・`src/zellij/process.rs` 6件）はlist-display.toml・backend-process.tomlが管轄（src/のままtag付け。条件数の内訳13条件・6条件に含む）。

## L4統合テスト（実zellij）

正本は docs/testing/test-plan.md §3「L4統合test」（harness構成・シナリオ要件・実施記録S1〜S11・S-v2-1〜6・既知の検証限界）。runner/harness・実行記録はrepo管理外の検証作業dir（エフェメラル）。

## 実行

```bash
cargo test          # L1〜L3（141テスト。条件書141条件とcovers tagで1:1）
mise run verify-conditions   # 条件書の機械検証（id対応・網羅・schema・付け漏れ--complete込み）
cargo clippy --all-targets && cargo fmt --check   # lint
# L4はrepo管理外のharnessで実行（構成はdocs/testing/test-plan.md §1・要件は§3）
# 注: zellij session内で実行してもテストは隔離済み（ZELLIJ_SESSION_NAMEを除去）
```

## 規則

- **テスト追加時は条件書の更新が先行**（update-tests skillの二段階委任: 条件書→test code+tag付け）。tag付け忘れは`mise run verify-conditions`（--complete検査）がfailで検出する
- 実装前fail-firstの履歴はtest-plan §5
- fixtureは実機出力からの抽出物とし、改変時は出典をコメントで明記
- テスト追加時はこのREADMEの該当行を更新する
