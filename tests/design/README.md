# tests/design/ — テスト設計条件書

検証条件の正本は`tests/design/`の各条件書（`<stem>.toml`）である（test-structure skill準拠）。
要件定義・設計から条件を執筆し、testを`// [covers:<id>]`tagで条件に紐付ける。

機械検証（id対応・網羅・schema・source_lines自動同期・excluded構造）の実行方法:

```
mise run verify-conditions
```

- mise taskは`--strict-source --complete`付き（検査(f)付け漏れを含む全検査。設計書§5.1/§9.6）
- 実行は`uv run --no-project scripts/design/verify-conditions.py`（直接python実行しない）

## 条件書一覧（20 file・316条件）

verified列はmeta検証記録（verified = true）の実績値（backend-parserはsrc実装分のverified再記録が未実施のためfalseのまま残置〔cargo test --test parser は全pass〕。TASK-93テスト先行更新の4 file〔cli-grammar・list-display・backend-process・attach-detach〕はfail-first期間中のためfalse——実装・全条件緑化後に再記録。TASK-84のremove-session・TASK-95のrenameは実装工程で全条件緑化後にtrueへ再記録済み）。

| # | stem | source_file | 対応test target | 条件数 | verified |
|---|---|---|---|---|---|
| 1 | cli-grammar | src/cli.rs | cli_list_read_send（部分26件） | 26 | false（TASK-93 fail-first） |
| 2 | selector | src/selector.rs | selector（10件） | 10 | true |
| 3 | backend-parser | src/zellij/parser.rs | parser（8件） | 8 | false（再検証待ち） |
| 4 | json-contract | src/output/json.rs | error_map（4件）+ cli_list_read_send（部分2件） | 6 | true |
| 5 | read-send | src/app/read.rs | cli_list_read_send（部分11件） | 11 | true |
| 6 | list-display | src/app/list.rs | cli_list_read_send（部分11件）+ lib src/app/list.rs（3件） | 14 | false（TASK-93 fail-first） |
| 7 | rename | src/app/rename.rs | fake_rename_add_remove（pane 3件+session L2 6件）+ cli_list_read_send（session L3 7件） | 16 | true |
| 8 | add-remove | src/app/remove.rs | fake_rename_add_remove（部分7件） | 7 | true |
| 9 | resize | src/app/resize.rs | fake_resize（7件） | 7 | true |
| 10 | layout-planner | src/app/remap.rs | layout_planner（26件） | 26 | true |
| 11 | layout-generator | src/layout/generator.rs | layout_generator（14件） | 14 | true |
| 12 | companion-seed | src/companion.rs | companion_seed（14件） | 14 | true |
| 13 | remap-sequence | src/app/remap.rs | fake_remap（47件） | 47 | true |
| 14 | remap-cli | src/app/remap.rs | cli_remap（15件） | 15 | true |
| 15 | docs-verb | src/app/docs.rs | cli_docs（9件） | 9 | true |
| 16 | backend-process | src/zellij/process.rs | lib src/zellij/process.rs（9件） | 9 | false（TASK-93 fail-first） |
| 17 | watch | src/app/watch.rs | fake_watch（loop系6件）+ cli_list_read_send（部分10件） | 16 | true |
| 18 | attach-detach | src/app/attach.rs | fake_attach（L2・4件）+ cli_list_read_send（部分10件）+ lib src/zellij/process.rs（部分2件） | 16 | false（TASK-93 fail-first） |
| 19 | move | src/app/move_.rs | fake_move（24件）+ cli_list_read_send（部分5件・L3） | 24 | true |
| 20 | remove-session | src/app/remove.rs | fake_remove_session（5件）+ cli_list_read_send（部分16件・L3） | 21 | true |

source_fileの代表とnote欄記載の対応（設計書§2.2）:

- json-contract: note = 「src/error.rs（ErrorClass・exit code対応）も検証対象」
- read-send: note = 「src/app/send.rs も検証対象」
- add-remove: note = 「src/app/add.rs も検証対象」
- remap-cli: note = 「廃止flag・version gateは src/cli.rs・src/zellij/process.rs も検証対象」
- attach-detach: note = 「src/app/add.rs run_session・src/zellij/process.rs run_interactive（traitはsrc/zellij/mod.rs）・src/main.rs AddTarget::Session例外dispatchも検証対象」
- move: note = 「src/zellij/process.rs（move_pane / move_tab・MovePaneOp・TabMoveDirection）・src/companion.rs（remap.rsから切り出しのcompanion helper。TASK-88 §2.8）も検証対象。CLI parse表面・help出現はcli-grammar.tomlのmove系5条件が分担」
- remove-session: note = 「src/zellij/process.rs（delete_session trait実装。trait定義はsrc/zellij/mod.rs）・src/cli.rs（RemoveTarget::Session・clap pairwise conflicts + app層exactly-one）・src/main.rs（RemoveTarget::Session例外dispatch・probe backend）も検証対象」
- rename: note = 「src/zellij/process.rs（rename_session trait実装。trait定義はsrc/zellij/mod.rs）・src/cli.rs（RenameTarget::Session）・src/main.rs（RenameTarget::Session例外dispatch・probe backend）も検証対象。CLI parse文法・help出現はcli-grammar.tomlが分担」

test file側の振分（1 fileが複数条件書に跨る分。設計書§2.3）:

- `tests/cli/list_read_send.rs` 93件 = cli-grammar 26（うちmove系5件: TASK-88のparse排他・native argv/no-op JSON・fail exit5・tab swap argv列・verb set v2 help出現〔watch/attach/detachの継承分を含む〕。TASK-65: 残excluded 8件の条件化14件を含む——zellij不在実path exit4・plugin PANESPEC正例・remap positional path・--json重複・resize equalize×grow排他・session解決残り段〔--session/env優先・複数/0件error〕・rename/resize/add/remove parse→argv正例・--tab×--all併合・--dry-run×--yes注意行・completion zsh/fish種別）/ read-send 11 / list-display 11 / json-contract 2 / watch 10（文法・出力契約・exit分類。loop系6件はfake_watch）/ attach-detach 10（L3層。TASK-89+TASK-93の非TTY attach解決先行・--session不在名・list sessions 0個出力）/ remove-session 16（L3層。TASK-84テスト先行: selector 4者排他・不在NAME candidates〔EXITED込み〕・dry-run plan〔running tabs/panes・deadはnull+list_tabs_for不呼出〕・delete-session <name> -f argv・部分失敗exit 6 results[]・self保護〔NAME経路拒否・--all除外warning・selfのみNoTarget〕・--me系〔env必須・解決名実行・実行前warning・dry-run current注記〕・--dead〔EXITED行のみ〕・0-session NoTarget・probe dispatch・成功報告〔removed session行・(was dead)〕。shimのFAKE_FAIL_DELETE_SESSION / FAKE_MUTE_DELETE_SESSION・${FAKE_LOG}.deleted〔delete-session作用のlist-sessions応答反映〕・FAKE_SESSIONSのEXITED行）/ rename session 7（L3層。TASK-95テスト先行: 成功argv/報告〔action形式・renamed_session/name envelope〕・不在NAME candidates〔EXITED込み〕・NEW_NAME衝突Preflight〔running・EXITED両方〕・同一名Usage・不正新名Usage〔空文字・'/'含み・空白含み〕・EXITED名Preflight拒否・probe dispatch。shimの${FAKE_LOG}.renamed〔rename-session作用のlist-sessions応答反映〕。FAKE_FAIL_RENAME_SESSION / FAKE_MUTE_RENAME_SESSIONは機構のみ維持——行使はL2のfail_op/mute注入条件が担う〔設計§2.10行使分担〕）
- `tests/fake_backend/rename_add_remove.rs` 16件 = rename 9（pane 3 / session L2 6: TASK-95テスト先行——状態書き換え〔exited不変〕・mute黙失敗のpostcondition検出〔VerificationFailed〕・呼出失敗〔OperationFailed〕・事後list失敗class保持〔but failed to confirm〕・空白含み新名拒否〔世界modelで復元不能のためUsage事前拒否〕・postcondition両判定の独立保護〔コードレビュー指摘2。fake.rsのrename_post_inject注入〕。L3に漏れた検査優先順位固定〔不在/EXITEDがNEW_NAME衝突に優先〕の複合違反testを含む）/ add-remove 7
- `tests/fake_backend/watch.rs` 6件 = watch（loop系: t0即時poll・any-hit tie-break・--regex opt-in・--full転送・interval sleep/試行回数・pane障害除外継続。設計§5.2/§5.3の層帰属）
- `tests/fake_backend/move.rs` 24件 = move（全条件。L2中心・1条件=1 test fn。TASK-88実装済み。REVIEW-3でprobe送信失敗・tab交換途中消滅の2条件を追加。polling timeout系3条件は10s deadlineを実時間消費）
- `tests/fake_backend/remove_session.rs` 5件 = remove-session（L2層。TASK-84テスト先行: 常時gate・postcondition 2系〔still listed・list失敗class保持〕・NAME重複除去・EXITED受理。fake.rsのsession状態管理拡張〔delete_sessionでsessionsから行削除・mute_delete_session / fail_list_sessions_after_delete注入・set_sessions fixture差し替え〕も参照）
- `tests/cli/docs.rs` 9件 = docs-verb（配布物出力一致7 + TASK-88のmove文法記載・verb set v2=12配布物記載2）
- lib内test 14件（src/app/list.rs 3件・src/zellij/process.rs 11件〔TASK-89のrun_interactive系2件・TASK-90のrc0+stderr系1件・TASK-93のlist-sessions 0件decode 1件を含む〕）= list-display・backend-process・attach-detachへ帰属
- `tests/fake_backend/attach.rs` 4件 + `tests/cli/list_read_send.rs` 10件 + lib src/zellij/process.rs 2件 = attach-detach 16件（TASK-89実装済み+TASK-93テスト先行更新。層帰属は設計書§5.2/§5.3・低-9修正: run_interactive経路はsrc内#[cfg(test)]のみ〔L3実binaryはTTY検査が先に立ち到達しない。TASK-93後は解決が先行——不在名/0件はNoTarget exit 3・有効名のみTTY検査へ〕・attachのTTY検査はL3実binary非TTY envelope検証+L2 tty_ok注入。detach/add sessionはL3 shim〔FAKE_DETACH_FAIL/FAKE_SESSIONS_CREATE・FAKE_SESSIONS_EMPTY〔0件文面はstderr出力〕・不在session名rc=1応答〕。add session一覧取得失敗2系はL2 fake.rsのfail_op・fail_list_sessions_after_create注入）

## R番号→条件id対応表

remap v2 matrix R1〜R50（旧test-plan §2.7）と条件idの対応。全R番号を網羅（設計書§9.5）。同一R番号から複数条件が生じる場合は複数idを並べる（R9・R12・R21・R22・R50）。

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

R12のmulti-tab行（layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only）は期待値がDD-10 v2.2基準: 反復単位は「layout最初のtab subtree（N）」でなく「layout全体（S = sum(N_t)）」。stable id（R12対応表の安定参照）は維持し、条件の期待値はv2.2正。

R番号条件の拡張として生じた内容kebab idの条件（設計書§3.3。source欄でR番号を関連参照）:
quoting warningの文字種拡張（R20拡張）= layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars・廃止option同時指定（R43関連）= remap-cli.both-removed-flags-together-still-usage-error。このほかレビュー指摘由来条件（toggle対象限定・TOCTOU直列化・polling deadline再検査・新規tab名postcondition・advisory lock・write失敗残留・廃止option即時削除・help非表示・bare bool parse等）はlayout-planner.toml / companion-seed.toml / remap-sequence.toml / remap-cli.tomlのsource欄で特定できる

TASK-81（DD-10 2026-09-19改訂: cwd注入・reconciliation・dup close・dry-run緩和・検証(e')(e'')・error.data拡張）由来の新条件18件（R番号なし。layout-generator 3・layout-planner 2・remap-sequence 9・remap-cli 4）は各条件書のsource欄（DD-10.x〔TASK-81〕参照）で特定できる。test追従・tag付けは完了済み（2026-09-20・TASK-81コードレビューC1対応の1件追加を含む全条件でverify-conditions pass）

## 範囲明示（対象外資産・特別扱い）

- 付け漏れ検出対照表（`tmp/20260913_bootstrap_conditions_task54/`）: 使い捨ての移行限定資産。`--complete`（検査(f)）有効化により用途を終えている（対照表diff運用は移行中のみ。設計書§5.5）。残置のみとし削除しない
- `tests/fake_backend/fake.rs`: test支援資産（fake backend実装）でtest fnなし。tag対象外・scanner走査には含まれるがtag検査の対象にならない
- `tests/fixtures/`（`tests/fixtures/zellij/*.json`等）: fixtures。scanner走査対象外
- L4統合test harness（podman・実zellij）: repo管理外。要件・実施記録は`docs/testing/test-plan.md`のL4節へ集約（設計書§6.1）。該当要求は各条件書の`[[excluded]]`から参照行で指す
- lib内test: `src/app/list.rs`の3件はlist-display.tomlが、`src/zellij/process.rs`の9件はbackend-process.tomlが管轄する（いずれもsrc/のままtag付けしfile移動しない。設計書§4.3）
- companion plugin実装（`plugin/src/`）: 現在test fnなし。test追加時はremap系の関連条件書へ条件化し、scanner走査対象（`plugin/src/**/*.rs`）に含まれる
