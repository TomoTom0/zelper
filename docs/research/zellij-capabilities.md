# Zellij Capabilities Research (Phase 1)

作成日: 2026-08-21
更新日: 2026-09-02（E6: Plugin APIによるcross-tab移動の実証を反映）・2026-09-05（remap v2のL4実機検証・起動後permissions seed反映の実証を反映）
対象: zelper設計のためのzellij機能調査
検証環境: zellij **0.44.3**（実機検証）+ 公式docs（0.45.x基準）

## 1. 調査方法

- 実機検証: podman sandbox（debian:12-slim、ホストzellijバイナリをread-only mount、`--network=none`、`script -qec`によるPTY駆動）で 0.44.3 を実行。プロセス保存の証明にはheartbeat手法（pane内プロセスがpid・連番付きで0.5秒ごとにログ追記、override後もpidと連番が継続すればプロセス保存と判定）を使用
- docs調査: zellij.dev公式ドキュメント（cli-actions / layouts / creating-a-layout / swap-layouts / cli-recipes / programmatic-control / session-resurrection / compatibility）+ GitHub CHANGELOG/releases
- 一次記録（調査メモ・実験script・生log）はrepo管理外の作業dirに残置。本書に検証結果・結論を集約
- 本書の記載で「0.44.3実機」は実験で確認済み、「0.45.x docs」はdocs由来で0.44.3未検証を意味する
- E6（2026-09-02）のみ手法が異なる: companion plugin（zellij-tile 0.44.3でbuildしたwasm）を`action pipe`で駆動。一次記録はrepo内`tmp/e6_plugin/`（plugin source・script）と`tmp/260901_exp_e6_*`（観測log）。他実験の一次記録はrepo管理外
- remap v2のL4実機検証（2026-09-04/05、TASK-39）もE6と同構成（podman + zellij 0.44.3実binary + companion plugin wasm埋込zelper static musl build）。一次記録はrepo内`tmp/260904_l4_harness_39/`（script・layout・証拠JSON/log）・`tmp/260904_l4_podman_run*_39.log`・`tmp/260904_l4_src_39/`（zellij-server 0.44.3 source）

## 2. Capability matrix

| Capability | Public primitive | Explicit target support | Structured output | Verified version | Notes/limitations |
|---|---|---|---|---|---|
| session一覧 | `zellij list-sessions` | なし（session名のみ） | なし（`-n`/`-s` テキストのみ） | 0.44.3実機 | JSON不在。zelperはテキストparseが必要 |
| tab一覧 | `action list-tabs` `-a -j` | 出力のみ（操作は`-t/--tab-id`） | あり（TabInfo全field） | 0.44.3実機 | **tab IDはclose後再利用**（安定キー不可） |
| pane一覧 | `action list-panes` `-a -j` | 出力のみ（操作は`-p/--pane-id`） | あり（PaneInfo全field。`pane_command`/`pane_cwd`/geometry含む） | 0.44.3実機 | デフォルト出力はnon-selectable plugin除外、`-a`で全件 |
| pane/tab ID体系 | `terminal_N` / `plugin_N`（bare `3`は`terminal_3`と等価） | ほぼ全actionが`-p`/`-t` | JSON `id`は数値のみ | 0.44.3実機 | terminal/plugin独立採番・単調増加。**pane IDは再利用なし（安定）**。pane内から`ZELLIJ_PANE_ID`/`ZELLIJ_SESSION_NAME`で自己識別可 |
| screen dump | `action dump-screen -p ID [-f] [-a] [--path P]` | あり（`-p`。省略時focused） | テキスト | 0.44.3実機 | 1回取得。デフォルトviewport、`-f`でscrollback、`-a`でANSI保持 |
| pane更新ストリーム | `zellij subscribe --pane-id ID... [--scrollback N] -f json` | あり（複数pane可） | NDJSON | 0.44.3実機 | 終了しないストリーム。1 event = viewport全行スナップショット |
| text入力 | `action write-chars -p ID <s>` / `action write -p ID <bytes>` / `action paste -p ID <s>` | あり（`-p`） | なし | 0.44.3実機（pasteは0.45.x docsのみ） | `write 13`=Enter。pasteはbracketed paste（docs: write-charsより高速・堅牢） |
| key入力 | `action send-keys -p ID <KEY>...` | あり（`-p`） | なし | 0.44.3実機 | keyごとに独立引数（`send-keys e c h o Enter`）。スペース区切り1引数はexit 2 |
| pane rename | `action rename-pane -p ID <name>` / `undo-rename-pane -p` | あり | なし | 0.44.3実機 | list-panesのtitleに反映 |
| tab rename | `action rename-tab -t ID <name>` / `rename-tab-by-id <id> <name>` / undo系 | あり | なし | 0.44.3実機 | `query-tab-names`で名前一覧（JSONなし） |
| session rename | `action rename-session <name>` | なし | なし | 0.44.3実機 | **かつてのsession名への戻しはexit 0で無効果**（2回観測） |
| pane作成 | `action new-pane [opts] [-- <cmd>]` | 作成IDをstdout返却。`--tab-id`で他tab作成可 | ID返却（`terminal_N`） | 0.44.3実機 | `--cwd` `-n/--name` `-d/--direction` `-f/--floating` `--stacked` `--near-current-pane`等。方向未指定は最大空き領域 |
| tab作成 | `action new-tab [opts]` / `zellij --layout NAME --session 既存` | tab IDをstdout返却 | ID返却（数値） | 0.44.3実機 | `-l/--layout`（layout_dirのname）+ `--layout-string`が使用可。`--layout`+`--session`は**既存sessionへのtab追加専用**（新規sessionは`--new-session-with-layout`） |
| pane削除 | `action close-pane -p ID` | あり | なし | 0.44.3実機 | プロセスはkillされる。command paneはheld状態になりうる（`exited`/`is_held`/`exit_status`で判別） |
| tab削除 | `action close-tab -t ID` / `close-tab-by-id <id>` | あり | なし | 0.44.3実機 | **存在しないIDはexit 0で無操作**（エラー検出に使えない） |
| resize | `action resize <increase\|decrease> <left\|right\|up\|down>` + `-p` | あり（`-p`） | なし | 0.44.3実機 | 境界単位の増減。**geometry次第でno-opあり**→適用後`list-panes -g`検証が前提。1回の刻み幅はdocsに記載なし |
| pane移動（tab内） | `action move-pane <dir>` + `-p` / `move-pane-backwards -p` | あり（`-p`） | なし | 0.44.3実機 | 同一tab内のみ。プロセス保存を確認 |
| pane移動（cross-tab） | **CLI actionには存在しない**。Plugin API（zellij-tile、0.38〜）に `break_panes_to_new_tab` / `break_panes_to_tab_with_index` / `break_panes_to_tab_with_id(pane_ids, tab_id, should_change_focus)` あり | あり（pane_ids + target tab） | なし | 0.44.3実機（CLI不可を確認、Plugin API経由の可をE6で確認） | `move-pane`はtab境界でno-op（CLI surfaceは不変）。companion plugin + `action pipe`駆動で**プロセス保存移動を実証**（§4.1 E6）。移動元tabが空になるとtabごと自動close。`new-pane --tab-id`は新規プロセス（移動でない）。plugin paneのみ`launch-or-focus-plugin --move-to-focused-tab`あり |
| companion plugin pipe | `action pipe --plugin <url> --name <n> -- <payload>` / `launch-plugin <url>`（positional。`--url`はexit 2） | あり（`--plugin`） | なし（`cli_pipe_output`のCLI stdout応答は**不達**: 1s timeout ERROR、致命なし。postcondition検証で代替） | 0.44.3実機（E6） | pluginは**bin crate**必須（cdylibはloaderが`_start` exportを要求しload失敗）。plugin未稼働時のpipeは自動launch（defaultはfloating pane）。`ZellijPlugin::pipe`で受信。権限は`<XDG_CACHE_HOME>/zellij/permissions.kdl`へのseedで無dialog自動化（**session起動後のseedも有効**。§4.1 E6/L4） |
| layout discovery | `zellij setup --dump-layout NAME` / layout_dir配置 + bare name | - | KDL text | 0.44.3実機 | bare nameは**拡張子なしのみ**解決（`name.kdl`は相対パス扱いで失敗） |
| layout dump | `action dump-layout` | session全体 | KDL text | 0.44.3実機 | 現状sessionの完全KDL。remap前後の検証基準に使える |
| runtime layout差し替え | `action override-layout <path\|name> [--layout-string S] [--layout-dir D] [--apply-only-to-active-tab] [--retain-existing-terminal-panes] [--retain-existing-plugin-panes]` | active tab基準 | なし | 0.44.3実機 | §4参照。**zelper remapの中核プリミティブ**。各pane nodeに既存paneと同一の`command`+`args`を書くとrun一致照合で決定論的slot配置（E6。§4） |
| 既存pane保持 | retain系2 flag | - | - | 0.44.3実機 | slot超過pane: flagなし=**kill** / あり=最終slot内に入れ子収容。floating paneはretain対象外でkill |
| inline layout文字列 | `--layout-string`（new-tab / override-layout / switch-session / session起動） | - | - | 0.44.3実機 | **改行区切りKDL必須**（dump-layout形式）。単行`;`区切り等は全variant parse error。shell quoting回避のためsubprocess argvで渡す。**値はquote必須**: `command=sleep`のようなbare文字列値はzellij parserに拒否される（統合テストS9で確認。zelperは生成時に強制quote） |
| swap layouts | layout内`swap_tiled_layout`/`swap_floating_layout` + `next/previous-swap-layout -t` | あり（`-t`） | `list-tabs --json`の`active_swap_layout_name`/`is_swap_layout_dirty` | 0.44.3実機（field確認） | pane数制約（max/min/exact_panes）駆動の自動再配置。**明示的remapには不向き**（0.45.x docs: breadch first配置） |
| floating panes | `new-pane -f` / `toggle-pane-embed-or-floating -p` / `change-floating-pane-coordinates -p` / `show/hide/toggle-floating-panes -t` | あり（`-p`/`-t`） | `is_floating`等 | 0.44.3実機 | tiled↔floating切替も**プロセス保存**。hidden中もプロセス停止なし。`are-floating-panes-visible`はstdout true/false（exit 0/2を観測、help記載の1と相違） |
| stacked panes | `new-pane --stacked` / `stack-panes -- <ids>` | あり（ID列挙） | - | 0.44.3実機（存在確認のみ。動作詳細未検証） | 0.45.0で描画がリスト形式に変更（0.45.x docs） |
| plugin panes | `launch-plugin <url>` / `launch-or-focus-plugin` / `pipe` | pane ID返却 | `plugin_url`/`is_selectable` | 0.44.3実機 | non-selectable plugin（tab-bar等）は`list-panes`デフォルト除外。slot数勘定から除外が安全 |
| exit code / error | 各action | - | - | 0.44.3実機 | 引数不正はexit 2。存在しないtab IDへのclose等はexit 0で無操作（**サイレント成功**に注意）。`action --session`は構文エラー（`zellij --session NAME action ...`か`ZELLIJ_SESSION_NAME`を使用） |
| session resurrection | 1秒〜間隔でsessionをKDL serialize / `zellij attach` / `action save-session` | - | KDL file | 0.45.x docs（一部0.44.3実機でaction存在確認） | 実体はcommandのre-runでありプロセス継続ではない。zelperでは「復旧保険」として位置づけ |

## 3. 実験環境の再現

`tmp/phase1`は当時の実験script格納dir（repo管理外・非公開）。script一式無しの完全再現は不可、harness構成の再構築は§1記述による

```bash
podman run --rm --network=none \
  -v "$(command -v zellij)":/usr/local/bin/zellij:ro \
  -v $PWD/tmp/phase1:/work -w /work \
  docker.io/library/debian:12-slim bash /work/exp-NN-*.sh
```

- zellijバイナリはstatically linked（glibc非依存）のためdebian:12-slimで動作
- client本体: `script -qec "stty rows 60 cols 200; zellij --session zelper-p1-XXX" /dev/null &`
- 外部操作: `zellij --session NAME action ...`（PTY不要）
- **config.kdlを配置しないとFirst Run Setup Wizard（plugin pane）が現れ、`-d`付きnew-pane連続発行時にpaneが作成直後に消える**。テスト環境では必ずconfig.kdlを置く
- プロセス保存検証: pane内で `bash /work/hb.sh <tag>`（0.5秒ごとに`timestamp tag i pid`を追記）を起動し、操作後にpidと連番iの継続を確認
- 全script・生logの対応表は非公開の一次記録にあり、結果は本書§2以降に集約

## 4. override-layout 詳細（remap中核）

0.44.3実機で確認した動作（§1手法の実機検証。生logは非公開の一次記録）:

| 状態 | 結果 | プロセス |
|---|---|---|
| 3 pane → 3 slot | 全pane同一ID・geometryのみ変化 | **保存**（pid継続を証明） |
| 3 pane → 2 slot、retainなし | 超過1 pane close | **kill** |
| 3 pane → 2 slot、`--retain-existing-terminal-panes` | 超過paneは最終slot内に入れ子収容 | 保存 |
| 2 pane → 3 slot | 空slotに既定shell起動 | 既存2 pane保存 |
| 2 pane → 3 slot（空slotに`command=`/`args`指定） | 指定コマンド起動 | 既存2 pane保存 |
| 複数tab状態でflagなし適用 | **他tabがすべてclose** | 他tabのpaneはkill |
| 複数tab状態で`--apply-only-to-active-tab` | active tabのみ再構成 | 他tabは無傷 |
| floating pane共存でactive-tab限定適用 | floating paneはclose | kill（retain flagの対象外） |

追加事項:

- 適用後のtabは**layout KDLに明示しない限りtab-bar/status-barを持たない**（pane領域が全面化）。bar維持なら生成KDLに `pane size=1 borderless=true { plugin location="zellij:tab-bar" }` を含める
- slot↔paneの割当順はpane ID昇順ではなかった（観測値）。zelperは適用後に`list-panes`で実対応を検証する設計が必要
- layout KDLの各pane nodeに既存paneと同一の`command` + `args`を書くと `find_already_running_panes`（screen.rs）のrun一致照合で**決定論的にslot配置**される（E6。bare `pane`（run=None）はcommand起動paneとは一致せず、shell pane（invoked_with=None）のみと一致）

### 4.1 複数tab layout適用とoverflow（一次記録: E1〜E5は非公開、E6は`tmp/e6_plugin/`・`tmp/260901_exp_e6_*`）

| 実験 | 状態 | 結果 | プロセス |
|---|---|---|---|
| E1 | 6 pane/1 tab → 2-tab×3-slot、flagなし | tab構成はlayout通り2 tab化。**既存paneは第1tabの3 slotのみに投入**（t1/t2/t3保存）、第2tabは全slot新規shell、t0/t4/t5はkill | 部分保存・部分kill |
| E2 | 同状態 + retain | killなし・6 pane全保存。ただし**全paneが第1tab内に押し込まれlayout形状は崩壊**、第2tabは新規shell | 全保存 |
| E3 | 4 pane → 2-tab×3-slot | 分配はtab毎均衡でなく**第1tabへの前詰め**。t0 kill、第2tabは新規shell | 部分保存 |
| E4 | `zellij --layout <file> --session 既存` | **既存tab・pane・geometryに完全無影響**。2 tab追加（全pane新規shell） | 既存全保存 |
| E5 | 7 pane → 2-tab×3-slot | なし=余り4 pane kill。retain=全保存だが第1tab内入れ子（focus位置依存の割り込み分割） | 上記同様 |
| E6 | 3 pane/2 tab（t1: A,B / t2: C）→ companion pluginの `break_panes_to_tab_with_id` でA,Bをt1→t2へ移動（CLI `action pipe`で駆動） | pane id・pid不変、heartbeat連番Gapなし。t1は空になり自動close | **保存** |
| E6 | 移動済み3 paneへ `command`+`args` 一致KDLで `override-layout --apply-only-to-active-tab --retain-existing-terminal-panes`（slot構成を変えて2回適用） | run一致照合で決定論的にslot配置。新規spawn 0・kill 0・配置入替成功 | **保存** |
| E6 | `break_panes_to_new_tab` でCを新規tabへ移動 | pid・連番継続 | **保存** |

E6詳細（2026-09-02、zellij-tile 0.44.3 sandbox実験）:

- Plugin API: zellij-tile 0.44.3に `break_panes_to_new_tab` / `break_panes_to_tab_with_index` / `break_panes_to_tab_with_id(pane_ids: &[PaneId], tab_id: usize, should_change_focus: bool)` が存在し、companion wasm pluginから呼べる。pluginは**bin crate**でbuild必須（cdylibはloaderが `_start` exportを要求するためload失敗）
- server実装は `extract_pane` → `add_tiled_pane`（PTYごと移設）で、プロセス保存の裏付け
- CLI駆動: `zellij --session S action pipe --plugin file:/path/e6.wasm --name <name> -- <payload>` でplugin（`ZellijPlugin::pipe`）が受信し実行（exit 0）。plugin未稼働時のpipeは自動launch（defaultはfloating pane）
- 権限自動化: `<XDG_CACHE_HOME>/zellij/permissions.kdl`（Linux ProjectDirs cache）にpluginのlocation文字列（file: locationはplain path）をquoted KDL node名として権限とともに記述すると対話dialogなしでGranted。0.44.3にlayout KDLの `permission_*` propertyやlaunch-pluginの権限引数は存在しない（source確認済み）のため、**permissions.kdl seedが唯一の自動化経路**
- **起動後のseedも有効**: `request_permission`は呼出毎にpermissions.kdlをdiskから再読込する（zellij-server 0.44.3 source確認・実機追認。L4 S-v2-1: session起動後にzelperがseedしてprobe即成立・移動実行。事前seedは不要）
- 合成: 移動済みpane群へのoverride-layoutは、§4追加事項のrun一致照合（`command`+`args`一致）により決定論的なslot配置・入替が可能
- `cli_pipe_output`（plugin→CLI応答）はCLI stdoutに不達（server logに "Action CliPipe did not complete within 1s timeout" ERROR）。致命ではなく、効果検証はlist-panes/list-tabsによるpostcondition検証で代替

L4実機検証（remap v2・2026-09-04/05、TASK-39。E6構成の再現で下記algorithmをzelper本体として実証）:

- S-v2-1: 2 tab / 9 hb pane → 4-slot layoutでk=3 instance生成（anchor tab保持 + 新規2 tab）。全9 pane pid不変・heartbeat連番Gapなし。空slotは既定shellで補完。runtime seed（session起動後にzelperがpermissions.kdlへseed）でprobe即成立・移動実行
- S-v2-2: dry-run --json は操作前後のlist-panes完全一致（状態変更なし。wasm extract・permissions.kdl書換も不発生）
- S-v2-3: k=1（移動不要）の経路ではcompanion pluginを一切使用しない（wasm extract無し）
- S-v2-4: `--overflow nest`（廃止option）はexit 0・stderr 1行warningのみで動作はv2既定と同一
- S-v2-5: floating pane存在でpreflight error（pane状態完全不変）。`--embed-floating` でpid保存のままtiled化して組入れ
- S-v2-6: permissions未seed + dialog応答不可で移動要remap時: probe pipeがtimeout（10s）し、**状態変更前に中断**（exit 5・権限hint付き・terminal pane構造完全不変）

**結論（2026-09-05更新）: override-layout単体では既存paneを第1tabにしか割り当てない（E1〜E5で観測、不変）。ただしcompanion pluginの `break_panes_to_*` によりプロセス保存のcross-tab移動は可能（E6実証）。「M>N paneを追加tabでlayout反復・プロセス保存」のalgorithm: pane群をN件ずつgroup化 → `break_panes_to_tab_with_id` で各target tabへ移動 → 各tabにcommand照合付きKDLでoverride-layout適用。zelper v2はこの構成で実装され、L4 S-v2-1〜6で実証済み**。

副次発見: 200桁の表示領域では7 pane目の`new-pane -d right`が静かに失敗する（pane即消失）。テストハーネンスは十分な表示サイズを確保する。
- `--layout-string`は改行区切りKDL必須（§2参照）。positional引数はfile path。bare name（拡張子なし）はlayout_dirから解決

## 5. zelper設計への帰結

1. **remap**: `list-panes -a --json`で現状把握 → 生成KDLを`override-layout --layout-string --apply-only-to-active-tab --retain-existing-terminal-panes`でinstance tab単位に適用、の構成で成立（v2では項2のcross-tab移動と組み合わせ、全paneをk instanceへ配置）。プロセス保存は実証済み。ただし(a) 超過paneの入れ子収容（v2ではgroup化+cross-tab移動で解消）、(b) 割当順の非自明性（`command`+`args`一致の記述で決定論化。E6。同一run・shell pane間は要件例外として保証外）、(c) barの喪失（生成KDLはlayout宣言nodeのみ維持しbarを自動注入しない）、(d) floating paneのkill（preflight errorと`--embed-floating`で明示扱い）、の4点はzelper側で明示的に扱う
2. **cross-tab移動はCLI actionに存在しないが、Plugin API（`break_panes_to_*`）をcompanion plugin + `action pipe`でCLI駆動すればプロセス保存移動が可能（E6実証）**。remapのoverflow（複数tabインスタンス）は「pane群をN件ずつgroup化 → `break_panes_to_tab_with_id` で各target tabへ移動 → command照合付きKDLでoverride-layout適用」で設計できる。**remapのM>Nを既定errorとする根拠は消えた**（要件2.6のoverflow設計参照。companion pluginの配布とpermissions.kdl seedという運用要件が新たに加わる）→ v2としてこの設計どおり実装され、L4 S-v2-1〜6で実証済み（plugin wasmはbinary埋込のため配布形態は単一binaryのまま）
3. **session列挙にJSONがない**ため、zelperのsession listは`list-sessions`テキストparse（`-n`で色消し）を実装する
4. **tab IDは再利用される**ため安定キーにできず、操作の直後に取得したIDを即消費する設計にする。pane IDは安定
5. **サイレント成功**（存在しないIDへのclose等がexit 0）があるため、zelperは操作後にlist-panes/list-tabsでpostcondition検証を行う（basic-design 2.5のverify段階に対応）
6. resizeはno-opがあり得るため、反復は上限付き + `list-panes -g`での収束判定が必要
7. テストハーネンス要件: 隔離session + config.kdl配置（Setup Wizard抑止）+ PTY駆動。本実験のpodman構成をそのまま統合テスト基盤にできる

## 6. 未検証事項（Phase 3で必要に応じて追加実験）

~~複数tab layout適用の挙動~~ → 解決（§4.1に追加）。~~起動後のpermissions.kdl seedが実行中serverに反映されるか~~ → 解決（反映される。§4.1 E6詳細・L4 S-v2-1）。残る未検証:

- `paste` actionの0.44.3での実挙動（docsは0.45.x基準）
- swap layoutsの詳細動作（breadth first配置、`--layout-string`内へのswap node記述可否）
- stacked panesの実挙動・`resize`のstack対応
- `save-session`のserialize間隔と`dump-layout`の差分
- resize 1回あたりの刻み幅（%または行/列数）
- 複数クライアント接続時の`is_focused`・`other_focused_clients`の挙動
- `break_panes_to_tab_with_index` の実挙動（E6は `with_id` / `new_tab` のみ実行）
- floating paneに対する `break_panes_to_*` の挙動（移動可否・tiled化の有無。zelper v2はこの経路を使わない: 対象外とするか`--embed-floating`でtiled化後に移動するため。L4 S-v2-5は後者の経路を実証）
- tab close/reorderでtab idとpositionが乖離したsessionでの `break_panes_to_tab_with_id` の照合挙動
- `cli_pipe_output` がCLI stdoutへ不達になる原因（E6では1s timeout ERRORを観測。致命ではない）
