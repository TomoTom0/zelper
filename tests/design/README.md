# tests/design/ — テスト設計条件書

検証条件の正本は`tests/design/`の各条件書（`<stem>.toml`）である（test-structure skill準拠）。
要件定義・設計から条件を執筆し、testを`// [covers:<id>]`tagで条件に紐付ける。
移行計画・適用仕様は`docs/design/conditions-migration.md`（旧正本`docs/testing/test-plan.md` §2から段階的に正本を移譲）。

機械検証（id対応・網羅・schema・source_lines自動同期・excluded構造）の実行方法:

```
mise run verify-conditions
```

- mise taskは`--strict-source --complete`付き（第4段で切替。検査(f)付け漏れを含む全検査。設計書§5.1/§9.6）
- 実行は`uv run --no-project scripts/design/verify-conditions.py`（直接python実行しない）

## 条件書一覧（15 file・135条件。§2.2確定構成133条件 + TASK-61でselectorへ2条件追加）

設計書§2.2の確定構成（133条件）に、移行後の未test項目条件化（TASK-61: selector +2）を加えた現行構成。verified列はmeta検証記録（verified = true）の実績値。

| # | stem | source_file | 対応test target | 条件数 | verified |
|---|---|---|---|---|---|
| 1 | cli-grammar | src/cli.rs | cli_list_read_send（部分6件） | 6 | true（2026-09-13） |
| 2 | selector | src/selector.rs | selector（10件） | 10 | true（2026-09-13） |
| 3 | backend-parser | src/zellij/parser.rs | parser（6件） | 6 | true（2026-09-13） |
| 4 | json-contract | src/output/json.rs | error_map（4件）+ cli_list_read_send（部分2件） | 6 | true（2026-09-13） |
| 5 | read-send | src/app/read.rs | cli_list_read_send（部分5件） | 5 | true（2026-09-13） |
| 6 | list-display | src/app/list.rs | cli_list_read_send（部分10件）+ lib src/app/list.rs（3件） | 13 | true（2026-09-13） |
| 7 | rename | src/app/rename.rs | fake_rename_add_remove（部分3件） | 3 | true（2026-09-13） |
| 8 | add-remove | src/app/remove.rs | fake_rename_add_remove（部分7件） | 7 | true（2026-09-13） |
| 9 | resize | src/app/resize.rs | fake_resize（7件） | 7 | true（2026-09-13） |
| 10 | layout-planner | src/app/remap.rs | layout_planner（15件） | 15 | true（2026-09-13） |
| 11 | layout-generator | src/layout/generator.rs | layout_generator（8件） | 8 | true（2026-09-13） |
| 12 | companion-seed | src/companion.rs | companion_seed（14件） | 14 | true（2026-09-13） |
| 13 | remap-sequence | src/app/remap.rs | fake_remap（20件） | 20 | true（2026-09-13） |
| 14 | remap-cli | src/app/remap.rs | cli_remap（8件） | 8 | true（2026-09-13） |
| 15 | docs-verb | src/app/docs.rs | cli_docs（7件） | 7 | true（2026-09-13） |

source_fileの代表とnote欄記載の対応（設計書§2.2）:

- json-contract: note = 「src/error.rs（ErrorClass・exit code対応）も検証対象」
- read-send: note = 「src/app/send.rs も検証対象」
- add-remove: note = 「src/app/add.rs も検証対象」
- remap-cli: note = 「廃止flag・version gateは src/cli.rs・src/zellij/process.rs も検証対象」

test file側の振分（1 fileが複数条件書に跨る分。設計書§2.3）:

- `tests/cli/list_read_send.rs` 23件 = cli-grammar 6 / read-send 5 / list-display 10 / json-contract 2
- `tests/fake_backend/rename_add_remove.rs` 10件 = rename 3 / add-remove 7
- lib内test 3件（src/app/list.rs）= list-displayへ帰属

## R番号→条件id対応表

remap v2 matrix R1〜R50（旧test-plan §2.7）と条件idの対応。第3段完了時に全R番号を網羅（設計書§9.5）。同一R番号から複数条件が生じる場合は複数idを並べる（R9・R12・R21・R22・R50）。

| R番号 | 条件書 | 条件id |
|---|---|---|
| R1 | layout-planner | layout-planner.r1-one-pane-into-three-slots-k1-empty-two |
| R2 | layout-planner | layout-planner.r2-three-into-three-no-move-no-plugin |
| R3 | layout-planner | layout-planner.r3-four-panes-into-three-slots-k2 |
| R4 | layout-planner | layout-planner.r4-six-into-three-two-full-instances |
| R5 | layout-planner | layout-planner.r5-seven-into-three-third-instance-partial |
| R6 | layout-planner | layout-planner.r6-cross-tab-groups-follow-visual-order |
| R7 | layout-planner | layout-planner.r7-empty-session-all-slots-empty |
| R8 | remap-sequence | remap-sequence.r8-floating-pane-in-scope-blocks-or-embeds |
| R9 | layout-planner / layout-generator | layout-planner.r9-fewer-panes-than-slots-keeps-k1・layout-generator.r9-fewer-panes-normalizes-declared-commands |
| R10 | layout-planner | layout-planner.r10-assignment-follows-visual-order-regardless-of-input-order |
| R11 | layout-planner | layout-planner.r11-zero-slot-layout-invalid-and-missing-layout-not-found |
| R12 | layout-planner | layout-planner.r12-slot-count-uses-first-tab-subtree-excluding-plugin-leaves・layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only・layout-planner.r12-zellij-bare-bools-and-property-nodes-parse |
| R13 | layout-generator | layout-generator.r13-occupied-slot-injects-command-and-args |
| R14 | layout-generator | layout-generator.r14-single-word-shell-command-still-injected |
| R15 | layout-generator | layout-generator.r15-shell-pane-stays-bare |
| R16 | layout-generator | layout-generator.r16-empty-slots-normalized-to-bare-and-cwd-never-injected |
| R17 | layout-generator | layout-generator.r17-newline-separated-quoted-output-is-reparseable |
| R18 | layout-generator | layout-generator.r18-plugin-leaf-preserved-without-consuming-slot |
| R19 | layout-generator | layout-generator.r19-same-run-panes-share-instance-slot-order-unspecified |
| R20 | layout-planner | layout-planner.r20-quoted-chars-in-pane-command-warn-but-do-not-fail |
| R21 | companion-seed | companion-seed.r21-missing-file-creates-fresh-node・companion-seed.r21-ensure-creates-dir-and-file-when-missing |
| R22 | companion-seed | companion-seed.r22-node-with-all-permissions-is-no-change・companion-seed.r22-ensure-leaves-complete-file-untouched |
| R23 | companion-seed | companion-seed.r23-missing-permissions-appended-keeping-others |
| R24 | companion-seed | companion-seed.r24-absent-node-pure-text-append-with-newline-guard |
| R25 | companion-seed | companion-seed.r25-unparsable-existing-file-is-preflight-error |
| R26 | companion-seed | companion-seed.r26-write-is-atomic-single-complete-content |
| R27 | companion-seed | companion-seed.r27-conflicting-rewrite-is-retried-on-new-content |
| R28 | companion-seed | companion-seed.r28-persistent-conflict-aborts-without-writing |
| R29 | remap-sequence | remap-sequence.r29-probe-only-when-move-needed |
| R30 | remap-sequence | remap-sequence.r30-probe-polled-then-state-change-proceeds |
| R31 | remap-sequence | remap-sequence.r31-probe-timeout-aborts-before-any-state-change |
| R32 | remap-sequence | remap-sequence.r32-probe-nonce-unique-per-run |
| R33 | remap-sequence | remap-sequence.r33-break-id-moves-inbound-before-outbound |
| R34 | remap-sequence | remap-sequence.r34-break-id-completion-group0-all-on-anchor |
| R35 | remap-sequence | remap-sequence.r35-break-new-membership-then-rename-tab |
| R36 | remap-sequence | remap-sequence.r36-polling-timeout-reports-groups-and-latency-note |
| R37 | remap-sequence | remap-sequence.r37-all-source-panes-survive-on-assigned-tabs |
| R38 | remap-sequence | remap-sequence.r38-instance-tab-pane-count-mismatch-fails-verification |
| R39 | remap-sequence | remap-sequence.r39-new-tab-names-and-anchor-survives |
| R40 | remap-sequence | remap-sequence.r40-empty-slots-filled-with-default-shell |
| R41 | remap-sequence | remap-sequence.r41-verification-failure-reports-diff-and-progress |
| R42 | remap-sequence | remap-sequence.r42-focus-restore-failure-is-not-operation-failure |
| R43 | remap-cli | remap-cli.r43-removed-flags-are-usage-error-exit2 |
| R44 | remap-cli | remap-cli.r44-removed-flag-json-keeps-stdout-empty |
| R45 | remap-cli | remap-cli.r45-removed-flags-absent-from-help |
| R46 | remap-cli | remap-cli.r46-below-minimum-and-future-series-exit4 |
| R47 | remap-sequence | remap-sequence.r47-dry-run-never-mutates-even-when-move-needed |
| R48 | remap-cli | remap-cli.r48-dry-run-json-reports-source-mnk-and-partition |
| R49 | remap-cli | remap-cli.r49-dry-run-shows-kdl-preview-per-instance |
| R50 | remap-cli / remap-sequence | remap-cli.r50-dry-run-lists-planned-operation-kinds・remap-sequence.r50-dry-run-includes-floating-pane-in-plan-without-toggle |

R番号条件の拡張として生じた内容kebab idの条件（設計書§3.3。source欄でR番号を関連参照）:
design-review §4.7 C6（R20拡張）= layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars・design-review §4.7 C7（R43関連）= remap-cli.both-removed-flags-together-still-usage-error。このほかC番号由来条件はlayout-planner.toml / companion-seed.toml / remap-sequence.toml / remap-cli.tomlのsource欄（§4.7 C1〜C7・§4.2 MR-16/MR-32・§4.6 R1〜R3・§4.8）で特定できる

## 進捗記録

段階適用（設計書§9）の進捗。**全段完了・移行完了（2026-09-13）**: 検証条件の正本は条件書15file（133条件・133tag・excluded 33件）へ移譲済み。test-plan §2は対応表へ縮小（旧§2.7本文のR番号定義は条件書source欄とR番号対応表へ）、L4要件はtest-plan §3「L4統合test」節へ集約、`tests/README.md`は索引化、mise taskは`--complete`付きへ切替済み。

| 段 | 対象 | 状態 | 完了日 |
|---|---|---|---|
| 第0段 | script・mise task・README骨子・対照表 | 完了 | 2026-09-13 |
| 第1段 | selector・backend-parser・json-contract（20条件） | 完了（レビュー指摘CMR3-1〜7対応済み。design-review §4.14） | 2026-09-13 |
| 第2段 | cli-grammar・read-send・list-display・rename・add-remove・resize・docs-verb（48条件） | 完了（レビュー指摘CMR4-1〜11対応済み。design-review §4.15） | 2026-09-13 |
| 第3段 | layout-planner・layout-generator・companion-seed・remap-sequence・remap-cli（65条件） | 完了（レビュー指摘CMR5-1〜4対応済み。design-review §4.16） | 2026-09-13 |
| 第4段 | test-plan縮小・L4節集約・tests/README索引化・meta最終確認・`--complete`切替 | 完了 | 2026-09-13 |

第2段の内訳（48条件・48tag）: cli-grammar 6 / read-send 5 / list-display 13（lib内3件含む） / rename 3 / add-remove 7 / resize 7 / docs-verb 7。excluded 21件（実装時14件 + 第2段レビューCMR4-2/4/9の指摘対応で7件追加。§8.3既知項目の対応: (a)(b)(c)(f)(g)(h)と出典走査で追加発見の未test要求。TASK-62〜68起票済み）。

第3段の内訳（65条件・65tag）: layout-planner 15 / layout-generator 8 / companion-seed 14 / remap-sequence 20 / remap-cli 8。R番号対応はR1〜R50の全行を上表へ記入済み（分割: R9×2・R12×3・R21×2・R22×2・R50×2）。excluded 8件（設計書§8.2確定分4件〔remap-sequence 3・remap-cli 1。「zellij不在」は第2段のcli-grammar.tomlに記録済み〕+ 出典走査で追加発見の未test要求3件〔remap --tab絞り込み・layout適用phase失敗報告・XDG未設定既定path。TASK-69〜71起票〕+ 第3段レビューCMR5-4で追加のcompanion setup失敗1件〔TASK-70へ併載〕）。

残作業: なし（全段完了）。meta最終確認も済み（全15fileのverified = true・source_hashが現行実装と一致）。

移行後の条件追加: TASK-61（2026-09-13）: selector.tomlのexcluded 2件（--cwd一致filter・filter 0件→NoTarget）をtest fn追加により条件化（selector.filter-cwd-exact-match・selector.filter-zero-matches-no-target。133→135条件・excluded 33→31件）。あわせてCMR3-3の--name完全一致検証限界を--name部分一致負例の追加で解除。残るexcluded 31件のうち未test対応を約定するものはTASK-62〜71（順次条件化するか要件変更時に再判断）。

移行限定資産の役割終了: 付け漏れ検出対照表（`tmp/20260913_bootstrap_conditions_task54/`）は使い捨てであり、`--complete`（検査(f)）有効化により用途を終えた（対照表diff運用は移行中のみ。設計書§5.5）。残置のみとし削除しない。

## 範囲明示（対象外資産・特別扱い）

- `tests/fake_backend/fake.rs`: test支援資産（fake backend実装）でtest fnなし。tag対象外・scanner走査には含まれるがtag検査の対象にならない
- `tests/fixtures/`（`tests/fixtures/zellij/*.json`等）: fixtures。scanner走査対象外
- L4統合test harness（podman・実zellij）: repo管理外。要件・実施記録は`docs/testing/test-plan.md`のL4節へ集約（設計書§6.1）。該当要求は各条件書の`[[excluded]]`から参照行で指す
- lib内test: `src/app/list.rs`の3件はsrc/のままtag付けし、list-display.tomlが管轄する（file移動しない。設計書§4.3）
- companion plugin実装（`plugin/src/`）: 現在test fnなし。test追加時はremap系の関連条件書へ条件化し、scanner走査対象（`plugin/src/**/*.rs`）に含まれる
