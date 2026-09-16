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

## 条件書一覧（16 file・172条件。§2.2確定構成133条件 + TASK-61でselectorへ2条件追加 + TASK-59でbackend-processへ6条件追加 + TASK-74で5fileへ6条件追加 + TASK-74 E2E再現回帰でlayout-generatorへ1条件追加 + TASK-74段階8コードレビューCR74対応でremap-sequenceへ1条件追加・既存条件更新 + TASK-75で5fileへ21条件追加・既存条件更新 + TASK-75 E2E vb改修でremap-cliへ1条件追加 + PR#6レビュー対応でremap-sequenceへ1条件追加・companion-seedへ既存条件更新）

設計書§2.2の確定構成（133条件）に、移行後の未test項目条件化（TASK-61: selector +2・TASK-59: backend-process +6）とTASK-74の条件書更新（remap照合key変更。6条件追加・9条件更新・excluded 4件追加）とTASK-75の条件書更新（multi-tab layout全体再現〔DD-10 v2.2〕。21条件追加・既存条件更新・excluded 5件追加〔remap-sequence 4・json-contract 1〕）とTASK-75のE2E vb改修（複数tab focus鋳型検出warningの伝達経路。remap-cliへ1条件追加）を加えた現行構成。verified列はmeta検証記録（verified = true）の実績値（TASK-75更新5fileは2026-09-16に実装・test 171/171 green・E2E acceptance 80条件PASS・CR75-1〜3対応完了を受けてtrueへ更新。TASK-74更新のbackend-parserのみsrc実装分のverified再記録が未実施のためfalseのまま残置〔cargo test --test parser は全pass〕）。

| # | stem | source_file | 対応test target | 条件数 | verified |
|---|---|---|---|---|---|
| 1 | cli-grammar | src/cli.rs | cli_list_read_send（部分6件） | 6 | true（2026-09-13） |
| 2 | selector | src/selector.rs | selector（10件） | 10 | true（2026-09-13） |
| 3 | backend-parser | src/zellij/parser.rs | parser（8件） | 8 | false（TASK-74更新・再検証待ち） |
| 4 | json-contract | src/output/json.rs | error_map（4件）+ cli_list_read_send（部分2件） | 6 | true（2026-09-13） |
| 5 | read-send | src/app/read.rs | cli_list_read_send（部分5件） | 5 | true（2026-09-13） |
| 6 | list-display | src/app/list.rs | cli_list_read_send（部分10件）+ lib src/app/list.rs（3件） | 13 | true（2026-09-13） |
| 7 | rename | src/app/rename.rs | fake_rename_add_remove（部分3件） | 3 | true（2026-09-13） |
| 8 | add-remove | src/app/remove.rs | fake_rename_add_remove（部分7件） | 7 | true（2026-09-13） |
| 9 | resize | src/app/resize.rs | fake_resize（7件） | 7 | true（2026-09-13） |
| 10 | layout-planner | src/app/remap.rs | layout_planner（24件） | 24 | true（2026-09-16） |
| 11 | layout-generator | src/layout/generator.rs | layout_generator（11件） | 11 | true（2026-09-16） |
| 12 | companion-seed | src/companion.rs | companion_seed（14件） | 14 | true（2026-09-16） |
| 13 | remap-sequence | src/app/remap.rs | fake_remap（33件） | 33 | true（2026-09-16） |
| 14 | remap-cli | src/app/remap.rs | cli_remap（11件。TASK-75分2件+vb回帰1件） | 11 | true（2026-09-16） |
| 15 | docs-verb | src/app/docs.rs | cli_docs（7件） | 7 | true（2026-09-13） |
| 16 | backend-process | src/zellij/process.rs | lib src/zellij/process.rs（7件） | 7 | true（2026-09-16） |

source_fileの代表とnote欄記載の対応（設計書§2.2）:

- json-contract: note = 「src/error.rs（ErrorClass・exit code対応）も検証対象」
- read-send: note = 「src/app/send.rs も検証対象」
- add-remove: note = 「src/app/add.rs も検証対象」
- remap-cli: note = 「廃止flag・version gateは src/cli.rs・src/zellij/process.rs も検証対象」

test file側の振分（1 fileが複数条件書に跨る分。設計書§2.3）:

- `tests/cli/list_read_send.rs` 23件 = cli-grammar 6 / read-send 5 / list-display 10 / json-contract 2
- `tests/fake_backend/rename_add_remove.rs` 10件 = rename 3 / add-remove 7
- lib内test 9件（src/app/list.rs 3件・src/zellij/process.rs 6件）= list-display・backend-processへ帰属

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

R12のmulti-tab行（layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only）はTASK-75（DD-10 v2.2）で期待値を反転: 反復単位が「layout最初のtab subtree（N）」から「layout全体（S = sum(N_t)）」へ一般化された。stable id（R12対応表の安定参照）は維持し、条件の期待値のみv2.2正へ更新（2026-09-15）。

R番号条件の拡張として生じた内容kebab idの条件（設計書§3.3。source欄でR番号を関連参照）:
design-review §4.7 C6（R20拡張）= layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars・design-review §4.7 C7（R43関連）= remap-cli.both-removed-flags-together-still-usage-error。このほかC番号由来条件はlayout-planner.toml / companion-seed.toml / remap-sequence.toml / remap-cli.tomlのsource欄（§4.7 C1〜C7・§4.2 MR-16/MR-32・§4.6 R1〜R3・§4.8）で特定できる

## 進捗記録

段階適用（設計書§9）の進捗。**全段完了・移行完了（2026-09-13）**: 検証条件の正本は条件書15file（133条件・133tag・excluded 33件。これは移行完了時点の値であり、現行値とは異なる。以降の追加は「移行後の条件追加」参照）へ移譲済み。test-plan §2は対応表へ縮小（旧§2.7本文のR番号定義は条件書source欄とR番号対応表へ）、L4要件はtest-plan §3「L4統合test」節へ集約、`tests/README.md`は索引化、mise taskは`--complete`付きへ切替済み。

| 段 | 対象 | 状態 | 完了日 |
|---|---|---|---|
| 第0段 | script・mise task・README骨子・対照表 | 完了 | 2026-09-13 |
| 第1段 | selector・backend-parser・json-contract（20条件） | 完了（レビュー指摘CMR3-1〜7対応済み。design-review §4.14） | 2026-09-13 |
| 第2段 | cli-grammar・read-send・list-display・rename・add-remove・resize・docs-verb（48条件） | 完了（レビュー指摘CMR4-1〜11対応済み。design-review §4.15） | 2026-09-13 |
| 第3段 | layout-planner・layout-generator・companion-seed・remap-sequence・remap-cli（65条件） | 完了（レビュー指摘CMR5-1〜4対応済み。design-review §4.16） | 2026-09-13 |
| 第4段 | test-plan縮小・L4節集約・tests/README索引化・meta最終確認・`--complete`切替 | 完了 | 2026-09-13 |

第2段の内訳（48条件・48tag）: cli-grammar 6 / read-send 5 / list-display 13（lib内3件含む） / rename 3 / add-remove 7 / resize 7 / docs-verb 7。excluded 21件（実装時14件 + 第2段レビューCMR4-2/4/9の指摘対応で7件追加。§8.3既知項目の対応: (a)(b)(c)(f)(g)(h)と出典走査で追加発見の未test要求。TASK-62〜68起票済み）。

第3段の内訳（65条件・65tag）: layout-planner 15 / layout-generator 8 / companion-seed 14 / remap-sequence 20 / remap-cli 8。R番号対応はR1〜R50の全行を上表へ記入済み（分割: R9×2・R12×3・R21×2・R22×2・R50×2）。excluded 8件（設計書§8.2確定分4件〔remap-sequence 3・remap-cli 1。「zellij不在」は第2段のcli-grammar.tomlに記録済み〕+ 出典走査で追加発見の未test要求3件〔remap --tab絞り込み・layout適用phase失敗報告・XDG未設定既定path。TASK-69〜71起票〕+ 第3段レビューCMR5-4で追加のcompanion setup失敗1件〔TASK-70へ併載〕）。

残作業: なし（全段完了）。meta最終確認も済み（移行完了時点の全15fileのverified = true・source_hashが現行実装と一致。15file・133条件等は移行完了時点の値）。

移行後の条件追加: TASK-61（2026-09-13）: selector.tomlのexcluded 2件（--cwd一致filter・filter 0件→NoTarget）をtest fn追加により条件化（selector.filter-cwd-exact-match・selector.filter-zero-matches-no-target。133→135条件・excluded 33→31件）。あわせてCMR3-3の--name完全一致検証限界を--name部分一致負例の追加で解除。残るexcluded 31件のうち未test対応を約定するものはTASK-62〜71（順次条件化するか要件変更時に再判断）。

移行後の条件追加: TASK-59（2026-09-13）: backend-parser.toml移行時excluded 1件（zellijのerror出力→error class変換）をsrc/zellij/process.rs内test追加により条件化し、条件書backend-process.tomlを新規起票。6条件の内訳: 非zero exit→OperationFailed・spawn NotFound→ZellijUnavailable・spawn NotFound以外（権限なし等）→ZellijUnavailable・version parse不能→UnsupportedVersion・timeout超過→OperationFailed（TR59-1）・current-tab-info非単一tab出力→OperationFailed（TR59-3）。excluded 7件の内訳: check_capability最小version未満・系列超過（remap-cli.r46-below-minimum-and-future-series-exit4がcover）・check_capability受理・try_wait失敗・run()成功系・run_action argv組み立て・validate_exclusive伝播（add-remove.add-tab-layout-sources-conflictがcover。TR59-3）・parse系error伝播（backend-parser管轄。TR59-3）。条件135→141・excluded 31→37件（backend-parser側1件削除・backend-process側7件追加）。設計レビューTR59-1〜5はdesign-review §4.19。test fn追加・verify・meta検証記録まで完了。

移行後の条件更新: TASK-74（2026-09-15）: remap run一致照合修正（設計書 docs/design/remap-v2-matching-fix-task74.md・DD-10 v2.1）のtest-first前段として5条件書を更新。新規6条件: layout-planner.run-derives-from-invoked-with（run算出のterminal_command基準化。TR74-4直交2 pane込み）・layout-generator.template-children-substitution（default_tab_template children置換・children不在/leaf数不一致LayoutInvalid）・remap-sequence.empty-list-panes-retry-in-polls・remap-sequence.empty-response-retry-before-verify（副因Cの空応答retry。TR74-5必須合格条件）・backend-parser.terminal-command-field-parsed（TR74-4直交JSON・旧fixture互換）・backend-parser.empty-output-parses-to-none-not-error（parse_panes_opt/parse_tabs_opt。設計§3表外から§2.3/§2.5により再抽出）。更新9条件: layout-planner.r20系2条件（quoting warning対象をterminal_commandへ）・layout-generator.r13/r14/r15/r18（読み替え。期待値不変）・remap-sequence.r38（(b)系検証失敗文面へcwd起因案内）・remap-cli.r48/r49（invoked_with追加・KDL preview bare slot/bar leaf）。excluded 4件追加（remap-sequence）: TASK-74 E2E acceptance必須合格条件・E2E観察条件（j>=1 bar spawn・実機空応答）・cwd明示pane照合外れ実機確認（TR74-7）・hold flag/EditFile pane実機確認。条件141→147・excluded 37→41件。この更新はテストコード・実装に先行（fail-first段階。5fileのmeta verifiedは一時false。テストコード追従・実装後に再検証）。

移行後の条件追加: TASK-74 E2E再現回帰（2026-09-15）: E2E acceptance (ii) 不合格（tmp/task74/acceptance/verdict.txt: bar paneがcolumn内cell化しbar leafが生成KDLのtop-level siblingにならない。b_dump.kdl実測）を受け、layout-generator.template-children-substitution-nested-baseを新規起票。既存template-children-substitutionのfixtureがtemplate直接組み立てのflat 2-slot baseであったため、実layout（layout { } wrapper）からのdefault_tab_template_subtree抽出経路とbase subtreeの入れ子が検証できていなかったtest gapへの対応（9-pane.kdl相当の3x3入れ子base+children+compact-bar templateで、bar leafのtop-level sibling出現・run注入はterminal slotのみを期待）。新規id切りの根拠: 本条件書は1条件=1test fnの1:1対応運用であり、抽出経路+入れ子baseの回帰は既存条件（置換機構）と性質・evidence chain（E2E実測）が別のため。fail-first: 現行実装（commit cde4cd4）のdefault_tab_template_subtreeは文書root直下しか探索せず実layoutからは常にtemplate=Noneが返るため、期待に機械的にfailする（単体純粋関数での実挙動確認済み）。条件147→148。test追加は後段のtest更新作業で実施（tag未付けのためcoverage検査failが期待状態）。（追記: nested-base testはb935e0eで追加済み・tag付け完了。以降の記述は当時の記録）

移行後の条件更新: TASK-74 E2E run3 boolean属性quote障害（2026-09-15）: E2E run3（tmp/task74/acceptance/b_remap.json・verdict.txt）で「生成KDLのboolean属性が borderless="true" とquote付き文字列化されzellij 0.44.3 layout parserに拒否される（borderless should be either true or false, found "true"）」障害が確定。既存test期待値がquote付き形式を内包（nested-base: bar leafのborderlessをas_string比較）または形式未assert（r49: template付きlayoutのbar leaf属性形式を検証せず）であったためL1〜L3では検出できなかった。設計書§2.2生成KDL例（borderless=true はquoteなし真偽値・size=1 もquoteなし）を正本として、remap-cli.r49・layout-generator.template-children-substitution・layout-generator.template-children-substitution-nested-baseの3条件へ「boolean属性はquoteなし真偽値形式（borderless=true）・数値属性（size=1）もquoteなし整数で出力されること（quote付き文字列形式はzellij 0.44.3 parser拒否のため不可）」を明記（fail-first: 現行実装〔run3 binary〕はboolean属性をquote付き文字列化して出力）。条件数は不変（148）・文面更新のみ。test側期待値の追従（as_string比較の修正等）は後段のtest更新作業で実施。参考: この時点でcoverage 148/148・cargo test 148件全pass（quote付き期待値を内包するtestが現行実装と一致してpassしていたことが検出漏れの実証）。

移行後の条件更新: TASK-74段階8コードレビューCR74-1〜8対応（2026-09-15）: 段階8コードレビュー指摘8件（全件修正/対応に確定）へのテスト前段として条件書を更新。新規1条件: remap-sequence.template-invalid-detected-before-mutation（CR74-1: template系LayoutInvalid〔children marker不在・leaf数不一致〕をrun()の状態変更前〔dry-run分岐と同形の位置のinstance_kdls計算〕で検証。probe pipe・toggle・break/move・rename・companion setupが0件。レビュー時点実装はexecute step 6発火でDD-10.7前提・設計§2.2「事前中断」違反）。更新5条件: remap-sequence.empty-list-panes-retry-in-polls・empty-response-retry-before-verify（CR74-2/3: 空応答注入をrun冒頭snapshotで消費されないpipe以降armの機構〔empty_panes_after_pipe等〕へ改め、probe poll・5-a/5-b poll・検証冒頭が実際に空を跨ぐ系列で検証。probe pollのtitle判定はlenient戻り値で直接行い1 tick 1回呼び出し〔strict list_panes再取得の2重呼び出しでない〕）、remap-sequence.r36-polling-timeout-reports-groups-and-latency-note・poll-deadline-not-extended-by-slow-backend-call（CR74-4: probe/move各timeout文面へF8機構〔screen繁忙の1s list応答timeoutによる空出力〕の注記を要求。誤診防止）、layout-generator.template-children-substitution（CR74-7: children block自体の不在〔braceなしdefault_tab_template node〕もLayoutInvalid。None扱いでbar欠落のまま通さない）。CR74-5（廃止規則のstale commentのterminal_command基準更新）・CR74-6（normalize_zellij_kdlの行単位字句処理が複数行文字列内bare true/falseを破壊しうる既存制限のdoc明記）はsrc/test側のcomment・doc修正であり条件化対象外。CR74-8: tests/README.md（期待値更新待ち等の記載）とlayout-generator・remap-cliのmeta note（「現行実装はquote付きでfail-first」等）を当時記録である旨へ補正（同趣旨のlayout-planner・backend-parser・remap-sequenceのmeta noteも同時に補正）。条件148→149。新規1条件はtag未付けのためcoverage検査failが期待状態・更新5条件はtest側期待値の追従待ち（test追加・追従は後段のtest更新作業）。

移行後の条件更新: TASK-75（2026-09-15）: remap multi-tab layout全体再現（設計書 docs/design/remap-v3-multi-tab-task75.md・DD-10 v2.2・requirements §2.6追記分）のtest-first前段として5条件書を更新。新規21条件: layout-planner 8（normalize-tab-templates-doc-order-and-tab-attributes〔正規形TabTemplate列: 文書順tab列挙・tab属性抽出・tab無しlayout T=1〕・multi-tab-allocation-cumulative-slots〔累積slot式 i→(b,t,s)〕・k-uses-total-slots〔k = max(1, ceil(M/S))〕・tab-names-from-templates-and-block-suffix〔幹=鋳型名/base・-<b+1>接尾・名無し鋳型のzelper命名〕・zero-slot-tab-template-invalid〔N_t=0鋳型LayoutInvalid〕・pane-focus-slot-first-focused-leaf〔文書順最初・複数時warning〕・empty-group-planned-for-all-blocks-tabs・t1-formula-level-backward-compat〔TR75-3: 計画レベル一致の保証範囲明記〕）・layout-generator 1（per-tab-kdl-from-normalized-templates〔bar全tab・per-tab KDL〕）・remap-sequence 9（move-phase-generalized-block-tab-order・empty-group-tab-created-via-new-tab-then-rename〔TR75-5昇格〕・focus-pane-id-after-override-per-tab〔go-to→override→focus-pane-id・occupied限定・失敗warning〕・final-go-to-honors-template-focus〔系列a/b/c〕・verify-per-tab-pane-count〔(b) N_t毎・anchor含む全生成tab〕・pane-focus-verified-via-is-focused〔(d)〕・leftover-tabs-reported-not-closed・new-tab-only-needs-no-companion〔TR75-5: break系0件・preflight同一depth〕・tab-id-resolution-lenient-before-apply〔TR75-4: targets配列・再解決〕）・remap-cli 2（dry-run-reports-s-and-per-tab-slots・json-n-field-holds-total-slots〔n=S互換〕）・backend-process 1（focus-pane-id-argv〔TR75-10薄い実装〕）。更新: layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only（期待値反転: S=sum(N_t)基準へ。stable id維持）・r1〜r11/run-derives（正規形TabTemplate列入力とblocks/groups用語へ読み替え・T=1後方互換で期待値不変）・layout-generator既存10条件（正規形subtree入力・template引数廃止の読み替え・期待値不変）・remap-sequence.r29（move_needed/probe要否分離）・r33〜r35・r37〜r41（group (b,t)用語・N_t一般化・error.data拡張）・r47（dry-run非破壊へfocus-pane-id追加）・template-invalid-detected-before-mutation（v2.2のLayoutInvalid種〔N_t=0〕と発火位置〔normalize/plan時点〕へ拡大）・remap-cli.r48/r49（T=1期待値不変の注記）・r50（操作列種別へfocus-pane-id・最終go-to追加）。excluded 5件追加（remap-sequence: TASK-75 E2E acceptance〔設計書§5.2管理・TASK-74先例〕・E2E観察/dump完全一致比較〔TR75-8〕・pane title/anchor位置保証外・swap/floating配置/new_tab_template対象外維持、json-contract: remap data内部構成拡張はremap-cli管轄〔stable fields不変〕）。--tabのexcludedへv2.2鋳型0単tab適用（D10）を追記。条件149→170・excluded 41→46件。この更新はテストコード・実装に先行（fail-first段階。新規21条件はtag未付けのためcoverage検査failが期待状態・更新条件はtest側期待値の追従待ち。backend-processのmeta verifiedは一時falseへ。テストコード追従・実装後に再検証）。

移行後のtest追従: TASK-75（2026-09-15）: 上記条件書更新の後段として、実行時fail-first可能な範囲（fake backend L2・CLI L3）のtestを作成（src/無変更）。test fn追加11件（tag付け済み）: fake_remap 9（move-phase-generalized-block-tab-order・empty-group-tab-created-via-new-tab-then-rename・focus-pane-id-after-override-per-tab・final-go-to-honors-template-focus・verify-per-tab-pane-count・pane-focus-verified-via-is-focused・leftover-tabs-reported-not-closed・new-tab-only-needs-no-companion・tab-id-resolution-lenient-before-apply）+ cli_remap 2（dry-run-reports-s-and-per-tab-slots・json-n-field-holds-total-slots）。既存期待値更新: remap-sequence.r41（error.dataへt=1・n_slots=[3]・mapping block keyのassert）・r47（bannedへfocus-pane-id追加）・template-invalid-detected-before-mutation（bannedへfocus-pane-id/new-tab追加）・remap-cli.r50（種別へfocus-pane-id追加）。test基盤拡張: fake.rsへmove phase後のtab id入れ替え/消失注入（TR75-4。step 6直前list-tabs経路で消費）、cli/remap.rsのFAKE_ZELLIJ_LOCKを毒化回復式へ（fail-first期のfail testが他testを巻き込まない。fake_backendのXDG_LOCKと同一構成）、fixturesへpanes-hetero.json/tabs-hetero.json（M=15・T=3・anchor単独tab合成fixture）を追加。fail-first実績: cargo test 160件中12件fail（fake_remap 9・cli_remap 3）で、すべて現行実装がv2.2一般化（multi-tab配分・空group new-tab・focus-pane-id・最終go-to・targets再解決・error.data/JSONのS/N_t拡張）未実装に起因。例外1件: leftover-tabs-reported-not-closedは現行pass（run()戻り値が()のためleftover_tabs報告のL2観測経路が実装まで確定せず、非close・生存・run成功の回帰固定のみ。報告assertは実装段階の戻り値/JSON拡張時に追加）。B系統10条件（layout-planner 8・layout-generator 1・backend-process 1）はnormalize_tab_templates・新plan_v2 signature・focus_pane backend method未実装のためtest未作成（coverage検査fail 10件が期待状態・実装直前に作成）。verify-conditionsの残failは当該10条件のcoverageのみを確認済み（source_lines自動同期は検査pass時にのみ書込むため、B系統tag付け後に一括反映される）。

移行後の条件更新: TASK-75実装・E2E・レビュー完了（2026-09-16）: v2.2実装（正規形TabTemplate列・blocks×groups配分・focus-pane-id・空groupのnew-tab --layout-string・targets再解決・leftover_tabs報告・error.data/JSONのS/N_t拡張）とB系統10条件のtest作成（layout_planner 8・layout_generator 1・lib 1）によりfail-first解消（cargo test 171/171 green・verify-conditions exit 0〔171条件・171tag・46excluded〕）。E2E acceptance（設計書§5.2・実施記録S-v3-1〜5・container通算15回・tmp/task75/acceptance/）は1巡目55 PASS/18 FAILから要因A・Bの改修を経て80条件全PASS。E2E要因A/B改修（2026-09-16・設計書§7.1）による5条件の文面改訂: focus-pane-id-after-override-per-tab・pane-focus-verified-via-is-focused（focus対象をmapping pane idから適用後のlist-panes位置〔visual order s番目〕ベースへ。occupied限定撤廃・already focused exit 2は成功扱い）・empty-group-tab-created-via-new-tab-then-rename・new-tab-only-needs-no-companion（空groupをnew-tab --layout-string経路へ。step 6のoverride-layout skip）・empty-response-retry-before-verify（精密arm法empty_*_nth_after_overrideの記述へ。step 6のfocus対象決定・存在確認pollが先頭N回空を消化するため）。vb系列退化（複数tab focus鋳型検出warningが未実装）の改修によりremap-cli.preflight-warning-paths-stderr-and-jsonを新規追加（170→171条件。stderr warning行とJSON data.warningsの伝達経路）。段階8コードレビューCR75-1〜3対応: CR75-1（P2）はtab-id-resolution-lenient-before-applyへ再解決位置のlist_panes空応答retry系列を追加（resolve_targetのlenient poll統一・red-first実証: tmp/20260916_test_task75_fix3_red.log）・CR75-2（P3）はTASK-69/70起票済みのため対応不要・CR75-3（P3）は設計書§4.4性能特性の新設（指摘の詳細はdesign-review §4.24）。あわせてmeta 5file（layout-planner・layout-generator・remap-sequence・remap-cli・backend-process）のverified=true化・verified_at/source_hashをTASK-75実装に対する値へ更新。

移行後の条件更新: PR#6レビュー対応（2026-09-16・TASK-76/77）: PR#6レビュー指摘2件への即時対応。TASK-76（floating source setのexited/held除外欠落）はremap-sequence.tomlへembed-floating-exited-held-excluded-from-floating-setを新規起票（171→172条件）しfake_remapへtest fn追加（exited & held・heldのみのfloating pane構成で--embed-floating実行し、toggle-embed・close-pane不発生・floatingのまま生存を検証）。TASK-77（SeedLockのDropでのlock file削除がflock+unlink競合で相互排除を崩す）はsrc/companion.rsのDropからremove_fileを廃止し、companion-seed.tomlを更新: concurrent-seeds条件のlock file期待を「削除」から「残留」へ反転、stale-lock-file-tolerated-and-removedをstale-lock-file-tolerated-and-retainedへ改称（条件数不変）、r26/c4（atomic書込・write失敗残留）へ恒久lock file .zelper-seed.lockの許容を追記。test追従4件（concurrent-seeds・stale-lockの期待反転、r26・c4の期待値へlock file追加）。最終形: cargo test 172/172 green・verify-conditions exit 0（16file・172条件・172tag・46excluded）。companion-seed・remap-sequenceのverified_at/source_hashを更新。

移行限定資産の役割終了: 付け漏れ検出対照表（`tmp/20260913_bootstrap_conditions_task54/`）は使い捨てであり、`--complete`（検査(f)）有効化により用途を終えた（対照表diff運用は移行中のみ。設計書§5.5）。残置のみとし削除しない。

## 範囲明示（対象外資産・特別扱い）

- `tests/fake_backend/fake.rs`: test支援資産（fake backend実装）でtest fnなし。tag対象外・scanner走査には含まれるがtag検査の対象にならない
- `tests/fixtures/`（`tests/fixtures/zellij/*.json`等）: fixtures。scanner走査対象外
- L4統合test harness（podman・実zellij）: repo管理外。要件・実施記録は`docs/testing/test-plan.md`のL4節へ集約（設計書§6.1）。該当要求は各条件書の`[[excluded]]`から参照行で指す
- lib内test: `src/app/list.rs`の3件はlist-display.tomlが、`src/zellij/process.rs`の6件はbackend-process.tomlが管轄する（いずれもsrc/のままtag付けしfile移動しない。設計書§4.3）
- companion plugin実装（`plugin/src/`）: 現在test fnなし。test追加時はremap系の関連条件書へ条件化し、scanner走査対象（`plugin/src/**/*.rs`）に含まれる
