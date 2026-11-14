# zelper tests 索引

テスト構成の管理対象はこのREADME。**検証条件の正本は`tests/design/`のテスト設計条件書**（`<stem>.toml`・test-structure skill準拠）。条件id・given/expect・出典・excluded記録は各条件書と`tests/design/README.md`を参照。テスト階層・L4統合test・fail-first履歴は docs/testing/test-plan.md。

## 構成（file → 条件書の索引）

- `design/` — テスト設計条件書20file・319条件（正本）+ README.md（条件書一覧・R番号→条件id対応表・範囲明示）
- `unit/` — L1 純粋ロジック
  - `selector.rs` — selector.toml（対象解決。10条件）
  - `parser.rs` — backend-parser.toml（zellij出力parse。8条件）
  - `error_map.rs` — json-contract.tomlの一部（error class↔exit status対応表・JSON envelope。4条件分）
  - `layout_planner.rs` — layout-planner.toml（remap v2 planner grouping。26条件。TASK-81のpane_cwd欠落skip+warning・shell -c高疑いheuristicsを含む）
  - `layout_generator.rs` — layout-generator.toml（remap v2 生成KDL。14条件。TASK-81のcwd注入・Run::Cwd slot表現・hold属性非併記を含む）
- `cli/` — L3 CLI契約（assert_cmd + fake zellij shim。shimはtest実行時に生成: argv記録・fixture応答・FAKE_* envで応答差替え。shim公開はstaging+atomic rename・binary内直列化でETXTBSY対策。fail-first期のfail testによるmutex毒化にもlock取得側で回復）
  - `list_read_send.rs` — cli-grammar（26。うちmove系5件: TASK-88のparse排他・native argv/no-op JSON・fail exit5・tab swap argv列〔shimのFAKE_FAIL_MOVE_PANE/FAKE_FAIL_MOVE_TAB・FAKE_TABS_JSON〔move-tabでposition移動を反映するtabs状態管理〕〕・help出現。TASK-65: 残excluded 8件の条件化14件——zellij不在実path exit4・plugin PANESPEC正例・remap positional path・--json重複・resize equalize×grow排他・session解決残り段〔--session/env優先・複数/0件error〕・rename/resize/add/remove parse→argv正例・--tab×--all併合・--dry-run×--yes注意行・completion zsh/fish種別。shimのFAKE_PANES_JSON〔rename-pane/close-pane/new-paneをlist-panes応答へ反映するpane状態管理〕・FAKE_NEW_PANE_TEMPLATE〔new-pane応答id採番〕）+ read-send（11。--tail/--full加工2件・失敗注入/複数行系列4件。shimのFAKE_DUMP_SCREEN応答差し替え・FAKE_FAIL_DUMP_SCREEN/FAKE_FAIL_WRITE_CHARS失敗注入）+ list-display（11）+ json-contract（2）+ watch（10。文法・出力契約・exit分類。TASK-86: shimのFAKE_DUMP_SCREEN_FRAMES〔@@ATTEMPT@@区切り・pane毎呼出回数で内容が変わるdump-screen応答〕・FAKE_FAIL_DUMP_SCREEN_AFTER〔pane:成功回数で途中から失敗〕で多attempt・pane消滅系列を再現）+ attach-detach（11。L3層。TASK-89: shimのFAKE_DETACH_FAIL・FAKE_SESSIONS_CREATE〔attach -b応答制御・ok時は作成NAMEをlist-sessions応答へ反映する状態管理〕・FAKE_SESSIONS_EMPTY〔0-session環境。TASK-93: 0件文面はstderr出力——backend decode条件〔rc=1 + stderr完全一致 + stdout空〕の適合前提〕・不在session名へのaction rc=1 Session not found応答。TASK-93: --session不在名の事前存在確認・非TTY attachの解決先行・list sessions 0個出力。TASK-96: add session NAME検査Usage文面〔空文字・'/'含み・空白含み・先頭`-`（`--`経由）〕）+ remove-session（16。L3層。TASK-84テスト先行: selector 4者排他〔NAME.../--all/--dead/--me〕・不在NAME candidates〔EXITED込み〕・dry-run plan〔running tabs/panes・deadはnull+list_tabs_for不呼出〕・delete-session <name> -f argv・部分失敗exit 6 results[]・self保護〔NAME経路拒否・--all除外warning・selfのみNoTarget〕・--me系〔env必須・解決名実行・実行前warning・dry-run current注記〕・--dead〔EXITED行のみ〕・0-session NoTarget・probe dispatch・成功報告〔removed session行・(was dead)〕。shimのFAKE_FAIL_DELETE_SESSION / FAKE_MUTE_DELETE_SESSION・${FAKE_LOG}.deleted〔delete-session作用のlist-sessions応答反映〕・FAKE_SESSIONSのEXITED行）+ rename session（7。L3層。TASK-95テスト先行: 成功argv/報告〔action形式・renamed_session/name envelope〕・不在NAME candidates〔EXITED込み〕・NEW_NAME衝突Preflight〔running・EXITED両方〕・同一名Usage・不正新名Usage〔空文字・'/'含み・空白含み・先頭`-`（`--`経由。TASK-97テスト先行で4案目追加）〕・EXITED名Preflight拒否・probe dispatch。shimの${FAKE_LOG}.renamed〔rename-session作用のlist-sessions応答反映〕。FAKE_FAIL_RENAME_SESSION / FAKE_MUTE_RENAME_SESSIONは機構のみ維持——行使はL2のfail_op/mute注入条件が担う〔設計§2.10行使分担〕）の94条件
  - `remap.rs` — remap-cli.toml（15条件。dry-run multi-tab拡張・JSON n=S互換・preflight warning伝達経路〔preflight-warning-paths-stderr-and-json〕・TASK-81のdry-run floating緩和・KDL preview cwd・error.data拡張・成功data.closed_duplicatesを含む。shimはFAKE_DUP_AFTER_OVERRIDEでdup spawnのemulationも持つ）
  - `docs.rs` — docs-verb.toml（9条件。TASK-88のmove文法記載・verb set v2=12配布物記載2件を含む。`[[test]]`宣言必須: tests/直下以外の自動target化に注意）
- `fake_backend/` — L2 決定的fake backend
  - `fake.rs` — FakeBackend本体（状態持ち・呼び出し記録・失敗注入・move phase後のtab id入れ替え/消失注入・watch用のdump_frames/dump_fail_after/dump_full_frames注入・move用のmove_pane/move_tab emulation〔M1/M2/m8実測基準値: 幾何隣接則による交換相手選択・rotateは対象と前後1交換〔wrap〕・focus遷移則・edge no-op・tab移動wrap〕とmute_break_id_nth/rename_moved_pane_after_break_nth/mute_move_tab/hide_pane_in_lenient_after_break/vanish_tab_after_move_tab注入〔未設定時は従来挙動不変〕。test支援資産でtag対象外）
  - `rename_add_remove.rs` — rename.tomlのうちL2 10条件（pane 3 + session L2 7）+ add-remove.toml（7）の17条件（session L2はTASK-95テスト先行+TASK-97テスト先行: 状態書き換え〔exited不変〕・mute黙失敗のpostcondition検出〔VerificationFailed〕・呼出失敗〔OperationFailed〕・事後list失敗class保持〔but failed to confirm〕・空白含み新名拒否〔世界modelで復元不能のためUsage事前拒否〕・先頭`-`新名拒否〔TASK-97: 検査順固定（先頭`-`は'/'・whitespace後・同一名differ前）と旧NAME="-x"一覧存在時のbackend到達契約保護〕・postcondition両判定の独立保護〔コードレビュー指摘2〕。検査優先順位固定〔不在/EXITEDがNEW_NAME衝突に優先〕の複合違反testも本file。rename.tomlのL3 7条件は cli/list_read_send.rs。fake.rsのrename_session〔record rename-session@{from} {to}+sessions行書き換え〕・mute_rename_session / fail_list_sessions_after_rename / rename_post_inject注入も参照）
  - `resize.rs` — resize.toml（7条件）
  - `companion_seed.rs` — companion-seed.toml（14条件。tempdir実file。lock fileは残留期待〔concurrent-seeds・stale-lock-file-tolerated-and-retained〕・r26/c4で恒久lock file許容）
  - `watch.rs` — watch.toml（16条件のうちloop系6件: t0即時poll・any-hit tie-break・--regex opt-in・--full転送・interval sleep/試行回数・pane障害除外継続。文法・出力契約・exit分類の10件はcli/list_read_send.rs。TASK-86）
  - `attach.rs` — attach-detach.toml（18条件のうちL2層5件: attach成功報告契約〔tty_ok注入〕・detach no-op成功・add session一覧取得失敗2系〔TASK-93: fail_op事前確認失敗・postcheck arm事後確認失敗〕・add session NAME検査fail-fast〔TASK-96: backend呼出ゼロ・Unicode空白・複合違反の優先順位〕。run_interactive系2件はlib src/zellij/process.rs・L3層11件はcli/list_read_send.rs。TASK-89。fake.rsのsession状態管理〔create_session_detachedでNAMEをlist_sessionsへ追加〕・mute_create_session・fail_list_sessions_after_create注入も参照）
  - `remove_session.rs` — remove-session.toml（21条件のうちL2層5件。TASK-84テスト先行: 常時gate〔単一NAMEでも--yes/--dry-run必須〕・postcondition 2系〔still listed is failure・事後list失敗のclass保持伝播〕・NAME重複除去〔指定順保持・報告1回〕・EXITED行受理〔dead経路で実行〕。L3層16件はcli/list_read_send.rs。fake.rsのsession状態管理拡張〔delete_sessionでsessionsから行削除・mute_delete_session / fail_list_sessions_after_delete注入・set_sessions fixture差し替え〕も参照）
  - `remap.rs` — remap-sequence.toml（46条件。multi-tab移動系列・空group new-tab --layout-string・focus-pane-id・最終go-to・per-tab検証・leftover・M=0・tab id再解決〔tab-id-resolution-lenient-before-apply〕・embed-floating-exited-held-excluded-from-floating-setを含む。r41はerror.data拡張assert・r47/template-invalidはfocus-pane-id/new-tab不発生assert。TASK-81のreconciliation系13条件〔安定観測・反転・dup close・closed_duplicates報告・(e')(e'')・dry-run緩和・E2E改訂F1/F2の押し出し割り込み・held化dup回収・再E2E R1の同一cwd曖昧tie-break・R2の対象tab内source外held pane報告〕を含む。fake.rsのFakeRun side-table・invoked_with等値matcher〔find_already_running_panes模倣〕・derive互換・bare slot spawn dupのpane_cwd注入も参照）
  - `move.rs` — move.toml（24条件。TASK-88+REVIEW-3: native系argv形式（M1実配置の全pane照合）〔move-pane <dir> -p / move-pane -p / move-pane-backwards -p〕・edge no-op検出・companion系実行sequence〔permissions seed→probe→break-id〕・no-op/不在/曖昧・floating/plugin pane preflight・pane交換の順序規則〔singleton判定〕・同一tab/両singleton拒否・部分失敗報告・tab交換の隣接transposition合成〔2d-1回〕・postcondition検証〔pane id不変・position交換〕・backend失敗・REVIEW-3追加（probe効果抑制と送信失敗の分離・timeout時の再取得による移動済み/未移動/確認不能の正確報告・tab交換途中消滅の部分進捗報告）。polling timeout系3条件は10s deadlineを実時間消費〔fake_remap r36と同一方式〕）
- `fixtures/zellij/` — 実zellij 0.44.3出力fixture 11file: panes.json / tabs.json（汎用）・panes-three.json（3 pane基本remap）・panes-remap.json / tabs-remap.json（remap dry-run）・panes-hetero.json / tabs-hetero.json（M=15・T=3 dry-run検証用の合成fixture）・panes-floating.json（TASK-81: 3 tiled + selectable floating terminal）・panes-cwd.json（TASK-81: terminal_command+pane_cwd付きpane）・panes-two.json（TASK-81: M=2<N=3検証失敗系列）・panes-three-dup.json（TASK-81: dup pane入り・FAKE_DUP_AFTER_OVERRIDEと併用）

lib内test 14件（`src/app/list.rs` 3件・src/zellij/process.rs 11件〔TASK-89のrun_interactive系2件・TASK-90のrc0+stderr系1件・TASK-93のlist-sessions 0件decode 1件を含む〕）はlist-display.toml・backend-process.toml・attach-detach.tomlが管轄（src/のままtag付け。条件数の内訳14条件・9条件・16条件に含む）。

## L4統合テスト（実zellij）

正本は docs/testing/test-plan.md §3「L4統合test」（harness構成・シナリオ要件・実施記録S1〜S11・S-v2-1〜6・既知の検証限界）。runner/harness・実行記録はrepo管理外の検証作業dir（エフェメラル）。

## 実行

```bash
cargo test          # L1〜L3（321テスト）
mise run verify-conditions   # 条件書の機械検証（id対応・網羅・schema・付け漏れ--complete込み。319条件でexit 0）
cargo clippy --all-targets && cargo fmt --check   # lint
# L4はrepo管理外のharnessで実行（構成はdocs/testing/test-plan.md §1・要件は§3）
# 注: zellij session内で実行してもテストは隔離済み（ZELLIJ_SESSION_NAMEを除去）
```

## 規則

- **テスト追加時は条件書の更新が先行**（update-tests skillの二段階委任: 条件書→test code+tag付け）。tag付け忘れは`mise run verify-conditions`（--complete検査）がfailで検出する
- 実装前fail-firstの履歴はtest-plan §5
- fixtureは実機出力からの抽出物とし、改変時は出典をコメントで明記
- テスト追加時はこのREADMEの該当行を更新する
