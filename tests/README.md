# zelper tests 索引

テスト構成の管理対象はこのREADME。**検証条件の正本は`tests/design/`のテスト設計条件書**（`<stem>.toml`・test-structure skill準拠）。条件id・given/expect・出典・excluded記録は各条件書と`tests/design/README.md`を参照。テスト階層・L4統合test・fail-first履歴は docs/testing/test-plan.md。

## 構成（file → 条件書の索引）

- `design/` — テスト設計条件書16file・171条件（正本）+ README.md（条件書一覧・R番号→条件id対応表・進捗・範囲明示）
- `unit/` — L1 純粋ロジック
  - `selector.rs` — selector.toml（対象解決。10条件）
  - `parser.rs` — backend-parser.toml（zellij出力parse。8条件）
  - `error_map.rs` — json-contract.tomlの一部（error class↔exit status対応表・JSON envelope。4条件分）
  - `layout_planner.rs` — layout-planner.toml（remap v2 planner grouping。24条件。TASK-75分8条件〔正規形TabTemplate列・累積slot配分・k=S基準・tab名規則・N_t=0無効・pane_focus_slot・空group・T=1後方互換〕込み・test作成済み）
  - `layout_generator.rs` — layout-generator.toml（remap v2 生成KDL。11条件。boolean属性quoteなし要求・CR74-7分children block不在LayoutInvalid要求は実装・test追従済み。TASK-75分per-tab-kdl-from-normalized-templates込み）
- `cli/` — L3 CLI契約（assert_cmd + fake zellij shim。shimはtest実行時に生成: argv記録・fixture応答・FAKE_* envで応答差し替え。shim公開はstaging+atomic rename・binary内直列化でETXTBSY対策: CR59-5/TASK-73。fail-first期のfail testによるmutex毒化にもlock取得側で回復）
  - `list_read_send.rs` — cli-grammar（6）+ read-send（5）+ list-display（10）+ json-contract（2）の23条件
  - `remap.rs` — remap-cli.toml（11条件。TASK-75新規2件〔dry-run multi-tab拡張・JSON n=S互換〕+ E2E vb改修1件〔preflight warning伝達経路: preflight-warning-paths-stderr-and-json〕。r50へfocus-pane-id種別追加）
  - `docs.rs` — docs-verb.toml（7条件。`[[test]]`宣言必須: tests/直下以外の自動target化に注意）
- `fake_backend/` — L2 決定的fake backend
  - `fake.rs` — FakeBackend本体（状態持ち・呼び出し記録・失敗注入。test支援資産でtag対象外。TASK-75分: move phase後のtab id入れ替え/消失注入〔TR75-4〕を追加）
  - `rename_add_remove.rs` — rename.toml（3）+ add-remove.toml（7）の10条件
  - `resize.rs` — resize.toml（7条件）
  - `companion_seed.rs` — companion-seed.toml（14条件。tempdir実file）
  - `remap.rs` — remap-sequence.toml（32条件。CR74系追従済み + TASK-75新規9件〔multi-tab移動系列・空group new-tab --layout-string・focus-pane-id・最終go-to・per-tab検証・leftover・M=0・tab id再解決〕 + r41へerror.data拡張assert・r47/template-invalidへfocus-pane-id/new-tab不発生assert・CR75-1でtab-id-resolution-lenient-before-applyへ再解決位置空応答retry系列追加）
- `fixtures/zellij/` — 実zellij 0.44.3出力fixture 7file: panes.json / tabs.json（汎用）・panes-three.json（3 pane基本remap）・panes-remap.json / tabs-remap.json（remap dry-run）・panes-hetero.json / tabs-hetero.json（M=15・T=3 dry-run検証用の合成fixture。TASK-75分追加）

lib内test 10件（`src/app/list.rs` 3件・`src/zellij/process.rs` 7件）はlist-display.toml・backend-process.tomlが管轄（src/のままtag付け。条件数の内訳13条件・7条件に含む。process.rsのTASK-75分1件はfocus-pane-id-argv）。

## L4統合テスト（実zellij）

正本は docs/testing/test-plan.md §3「L4統合test」（harness構成・シナリオ要件・実施記録S1〜S11・S-v2-1〜6・既知の検証限界）。runner/harness・実行記録はrepo管理外の検証作業dir（エフェメラル）。

## 実行

```bash
cargo test          # L1〜L3（171テスト。TASK-75実装完了により全green。内訳は下記TASK-75記録）
mise run verify-conditions   # 条件書の機械検証（id対応・網羅・schema・付け漏れ--complete込み。171条件・171tag・46excludedでexit 0）
cargo clippy --all-targets && cargo fmt --check   # lint
# L4はrepo管理外のharnessで実行（構成はdocs/testing/test-plan.md §1・要件は§3）
# 注: zellij session内で実行してもテストは隔離済み（ZELLIJ_SESSION_NAMEを除去）
```

TASK-74（2026-09-15）: 条件書のみ先行更新（5file・新規6条件+更新9条件・fail-first段階）。E2E acceptance不合格を受けlayout-generatorへ回帰条件1件追加（合計148条件）・excluded 41件の内訳は`design/README.md`進捗記録参照。E2E run3のboolean属性quote障害（生成KDLの borderless="true" がzellij 0.44.3 parser拒否。tmp/task74/acceptance/b_remap.json）を受けremap-cli.r49・layout-generator.template-children-substitution・template-children-substitution-nested-baseの3条件へquoteなし真偽値（borderless=true）・size=1 quoteなしの要求を明記（条件数不変・文面更新のみ。fail-first）。（その後の実装・test追従でboolean属性quote障害は解消済み——当時記録。段階8コードレビューCR74-1〜8対応として条件書を再更新〔149条件〕。内容は`design/README.md`進捗記録参照）

TASK-75（2026-09-15）: test追従（update-tests skill後段・TASK-75第2段階。src/無変更）。fake_backend/remap.rsへ9 test fn・cli/remap.rsへ2 test fn追加（tag付け済み）+ 既存期待値更新（r41へerror.dataのt/n_slots/block key assert・r47へfocus-pane-id不発生・template-invalidへfocus-pane-id/new-tab不発生・r50へfocus-pane-id種別）。fake.rsへtab id入れ替え/消失注入（TR75-4）を追加。fixturesへpanes-hetero.json/tabs-hetero.json（M=15・T=3合成）を追加。fail-first 12件（新規10件+期待値更新2件）: すべて現行実装がmulti-tab一般化（正規形TabTemplate列・S基準配分・空group new-tab・focus-pane-id・targets再解決・error.data/JSON拡張）未実装に起因。例外: leftover-tabs-reported-not-closedは非close・生存・run成功の回帰固定のみで現行pass（leftover_tabs報告のassertはrun()戻り値拡張後に追加予定・test comment記載）。B系統10条件（layout-planner 8・layout-generator 1・backend-process 1）はnormalize_tab_templates・新plan_v2 signature・focus_pane backend methodが未実装のためtest未作成（実装直前に作成）

TASK-75（2026-09-16）実装・E2E・レビュー完了: 条件書先行（149→170条件）→ test追従（fake_remap 9・cli_remap 2追加。fail-first 12件）→ v2.2実装とB系統10 test fn作成（layout_planner 8・layout_generator 1・lib〔focus-pane-id-argv〕1）によるfail-first解消 → E2E acceptance（1巡目55 PASS/18 FAILから要因A〔focus対象位置ベース化〕・要因B〔空groupのnew-tab --layout-string経路〕改修を経て80条件全PASS。実施記録S-v3-1〜5・container通算15回・tmp/task75/acceptance/・設計書§5.2/§7.1）→ vb系列退化改修（複数tab focus鋳型検出warning。cli_remapへdry_run_multiple_focus_warning_paths追加でtest総数170→171・remap-cli.tomlへpreflight-warning-paths-stderr-and-json新規で170→171条件）→ 段階8コードレビューCR75-1〜3対応（CR75-1: resolve_target lenient poll統一+tab-id-resolution-lenient-before-applyへ再解決位置空応答retry系列追加。CR75-2: TASK-69/70起票済み。CR75-3: 設計書§4.4性能特性新設。design-review §4.24）まで完了。最終形: cargo test 171/171 green・verify-conditions exit 0（16file・171条件・171tag・46excluded）。条件書meta 5file（layout-planner・layout-generator・remap-sequence・remap-cli・backend-process）のverified=true化とsource_hash更新済み（tests/design/README.md進捗記録参照）

## 規則

- **テスト追加時は条件書の更新が先行**（update-tests skillの二段階委任: 条件書→test code+tag付け）。tag付け忘れは`mise run verify-conditions`（--complete検査）がfailで検出する
- 実装前fail-firstの履歴はtest-plan §5
- fixtureは実機出力からの抽出物とし、改変時は出典をコメントで明記
- テスト追加時はこのREADMEの該当行を更新する
