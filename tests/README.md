# zelper tests 索引

テスト構成の管理対象はこのREADME。**検証条件の正本は`tests/design/`のテスト設計条件書**（`<stem>.toml`・test-structure skill準拠）。条件id・given/expect・出典・excluded記録は各条件書と`tests/design/README.md`を参照。テスト階層・L4統合test・fail-first履歴は docs/testing/test-plan.md。

## 構成（file → 条件書の索引）

- `design/` — テスト設計条件書18file・229条件（正本）+ README.md（条件書一覧・R番号→条件id対応表・範囲明示）
- `unit/` — L1 純粋ロジック
  - `selector.rs` — selector.toml（対象解決。10条件）
  - `parser.rs` — backend-parser.toml（zellij出力parse。8条件）
  - `error_map.rs` — json-contract.tomlの一部（error class↔exit status対応表・JSON envelope。4条件分）
  - `layout_planner.rs` — layout-planner.toml（remap v2 planner grouping。26条件。TASK-81のpane_cwd欠落skip+warning・shell -c高疑いheuristicsを含む）
  - `layout_generator.rs` — layout-generator.toml（remap v2 生成KDL。14条件。TASK-81のcwd注入・Run::Cwd slot表現・hold属性非併記を含む）
- `cli/` — L3 CLI契約（assert_cmd + fake zellij shim。shimはtest実行時に生成: argv記録・fixture応答・FAKE_* envで応答差替え。shim公開はstaging+atomic rename・binary内直列化でETXTBSY対策。fail-first期のfail testによるmutex毒化にもlock取得側で回復）
  - `list_read_send.rs` — cli-grammar（6）+ read-send（11。--tail/--full加工2件・失敗注入/複数行系列4件。shimのFAKE_DUMP_SCREEN応答差し替え・FAKE_FAIL_DUMP_SCREEN/FAKE_FAIL_WRITE_CHARS失敗注入）+ list-display（10）+ json-contract（2）+ watch（10。文法・出力契約・exit分類。TASK-86: shimのFAKE_DUMP_SCREEN_FRAMES〔@@ATTEMPT@@区切り・pane毎呼出回数で内容が変わるdump-screen応答〕・FAKE_FAIL_DUMP_SCREEN_AFTER〔pane:成功回数で途中から失敗〕で多attempt・pane消滅系列を再現）+ attach-detach（8。L3層。TASK-89: shimのFAKE_DETACH_FAIL・FAKE_SESSIONS_CREATE〔attach -b応答制御・ok時は作成NAMEをlist-sessions応答へ反映する状態管理〕・FAKE_SESSIONS_EMPTY〔0-session環境〕・不在session名へのaction rc=1 Session not found応答）の47条件
  - `remap.rs` — remap-cli.toml（15条件。dry-run multi-tab拡張・JSON n=S互換・preflight warning伝達経路〔preflight-warning-paths-stderr-and-json〕・TASK-81のdry-run floating緩和・KDL preview cwd・error.data拡張・成功data.closed_duplicatesを含む。shimはFAKE_DUP_AFTER_OVERRIDEでdup spawnのemulationも持つ）
  - `docs.rs` — docs-verb.toml（7条件。`[[test]]`宣言必須: tests/直下以外の自動target化に注意）
- `fake_backend/` — L2 決定的fake backend
  - `fake.rs` — FakeBackend本体（状態持ち・呼び出し記録・失敗注入・move phase後のtab id入れ替え/消失注入・watch用のdump_frames/dump_fail_after/dump_full_frames注入〔未設定時は従来挙動不変〕。test支援資産でtag対象外）
  - `rename_add_remove.rs` — rename.toml（3）+ add-remove.toml（7）の10条件
  - `resize.rs` — resize.toml（7条件）
  - `companion_seed.rs` — companion-seed.toml（14条件。tempdir実file。lock fileは残留期待〔concurrent-seeds・stale-lock-file-tolerated-and-retained〕・r26/c4で恒久lock file許容）
  - `watch.rs` — watch.toml（16条件のうちloop系6件: t0即時poll・any-hit tie-break・--regex opt-in・--full転送・interval sleep/試行回数・pane障害除外継続。文法・出力契約・exit分類の10件はcli/list_read_send.rs。TASK-86）
  - `attach.rs` — attach-detach.toml（12条件のうちL2層2件: attach成功報告契約〔tty_ok注入〕・detach no-op成功。run_interactive系2件はlib src/zellij/process.rs・L3層8件はcli/list_read_send.rs。TASK-89。fake.rsのsession状態管理〔create_session_detachedでNAMEをlist_sessionsへ追加〕・mute_create_session注入も参照）
  - `remap.rs` — remap-sequence.toml（46条件。multi-tab移動系列・空group new-tab --layout-string・focus-pane-id・最終go-to・per-tab検証・leftover・M=0・tab id再解決〔tab-id-resolution-lenient-before-apply〕・embed-floating-exited-held-excluded-from-floating-setを含む。r41はerror.data拡張assert・r47/template-invalidはfocus-pane-id/new-tab不発生assert。TASK-81のreconciliation系13条件〔安定観測・反転・dup close・closed_duplicates報告・(e')(e'')・dry-run緩和・E2E改訂F1/F2の押し出し割り込み・held化dup回収・再E2E R1の同一cwd曖昧tie-break・R2の対象tab内source外held pane報告〕を含む。fake.rsのFakeRun side-table・invoked_with等値matcher〔find_already_running_panes模倣〕・derive互換・bare slot spawn dupのpane_cwd注入も参照）
- `fixtures/zellij/` — 実zellij 0.44.3出力fixture 11file: panes.json / tabs.json（汎用）・panes-three.json（3 pane基本remap）・panes-remap.json / tabs-remap.json（remap dry-run）・panes-hetero.json / tabs-hetero.json（M=15・T=3 dry-run検証用の合成fixture）・panes-floating.json（TASK-81: 3 tiled + selectable floating terminal）・panes-cwd.json（TASK-81: terminal_command+pane_cwd付きpane）・panes-two.json（TASK-81: M=2<N=3検証失敗系列）・panes-three-dup.json（TASK-81: dup pane入り・FAKE_DUP_AFTER_OVERRIDEと併用）

lib内test 10件（`src/app/list.rs` 3件・`src/zellij/process.rs` 7件）はlist-display.toml・backend-process.tomlが管轄（src/のままtag付け。条件数の内訳13条件・7条件に含む）。

## L4統合テスト（実zellij）

正本は docs/testing/test-plan.md §3「L4統合test」（harness構成・シナリオ要件・実施記録S1〜S11・S-v2-1〜6・既知の検証限界）。runner/harness・実行記録はrepo管理外の検証作業dir（エフェメラル）。

## 実行

```bash
cargo test          # L1〜L3（229テスト）
mise run verify-conditions   # 条件書の機械検証（id対応・網羅・schema・付け漏れ--complete込み。229条件・229tag・65excludedでexit 0）
cargo clippy --all-targets && cargo fmt --check   # lint
# L4はrepo管理外のharnessで実行（構成はdocs/testing/test-plan.md §1・要件は§3）
# 注: zellij session内で実行してもテストは隔離済み（ZELLIJ_SESSION_NAMEを除去）
```

## 規則

- **テスト追加時は条件書の更新が先行**（update-tests skillの二段階委任: 条件書→test code+tag付け）。tag付け忘れは`mise run verify-conditions`（--complete検査）がfailで検出する
- 実装前fail-firstの履歴はtest-plan §5
- fixtureは実機出力からの抽出物とし、改変時は出典をコメントで明記
- テスト追加時はこのREADMEの該当行を更新する
