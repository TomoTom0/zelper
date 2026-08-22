# Zellij Capabilities Research (Phase 1)

作成日: 2026-08-21
対象: zelper設計のためのzellij機能調査
検証環境: zellij **0.44.3**（実機検証）+ 公式docs（0.45.x基準）

## 1. 調査方法

- 実機検証: podman sandbox（debian:12-slim、ホストzellijバイナリをread-only mount、`--network=none`、`script -qec`によるPTY駆動）で 0.44.3 を実行。プロセス保存の証明にはheartbeat手法（pane内プロセスがpid・連番付きで0.5秒ごとにログ追記、override後もpidと連番が継続すればプロセス保存と判定）を使用
- docs調査: zellij.dev公式ドキュメント（cli-actions / layouts / creating-a-layout / swap-layouts / cli-recipes / programmatic-control / session-resurrection / compatibility）+ GitHub CHANGELOG/releases
- 一次記録: `tmp/phase1/web-research.md`（docs調査）と `tmp/phase1/local-experiments.md`（実験記録。スクリプト `tmp/phase1/exp-*.sh`、生ログ `tmp/phase1/out/*.log` 全対応表付き）
- 本書の記載で「0.44.3実機」は実験で確認済み、「0.45.x docs」はdocs由来で0.44.3未検証を意味する

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
| pane移動（cross-tab） | **存在しない**（terminal pane） | - | - | 0.44.3実機（不可を確認） | `move-pane`はtab境界でno-op。代替: `new-pane --tab-id`（新規プロセス。移動ではない）。plugin paneのみ`launch-or-focus-plugin --move-to-focused-tab`あり |
| layout discovery | `zellij setup --dump-layout NAME` / layout_dir配置 + bare name | - | KDL text | 0.44.3実機 | bare nameは**拡張子なしのみ**解決（`name.kdl`は相対パス扱いで失敗） |
| layout dump | `action dump-layout` | session全体 | KDL text | 0.44.3実機 | 現状sessionの完全KDL。remap前後の検証基準に使える |
| runtime layout差し替え | `action override-layout <path\|name> [--layout-string S] [--layout-dir D] [--apply-only-to-active-tab] [--retain-existing-terminal-panes] [--retain-existing-plugin-panes]` | active tab基準 | なし | 0.44.3実機 | §4参照。**zelper remapの中核プリミティブ** |
| 既存pane保持 | retain系2 flag | - | - | 0.44.3実機 | slot超過pane: flagなし=**kill** / あり=最終slot内に入れ子収容。floating paneはretain対象外でkill |
| inline layout文字列 | `--layout-string`（new-tab / override-layout / switch-session / session起動） | - | - | 0.44.3実機 | **改行区切りKDL必須**（dump-layout形式）。単行`;`区切り等は全variant parse error。shell quoting回避のためsubprocess argvで渡す。**値はquote必須**: `command=sleep`のようなbare文字列値はzellij parserに拒否される（統合テストS9で確認。zelperは生成時に強制quote） |
| swap layouts | layout内`swap_tiled_layout`/`swap_floating_layout` + `next/previous-swap-layout -t` | あり（`-t`） | `list-tabs --json`の`active_swap_layout_name`/`is_swap_layout_dirty` | 0.44.3実機（field確認） | pane数制約（max/min/exact_panes）駆動の自動再配置。**明示的remapには不向き**（0.45.x docs: breadch first配置） |
| floating panes | `new-pane -f` / `toggle-pane-embed-or-floating -p` / `change-floating-pane-coordinates -p` / `show/hide/toggle-floating-panes -t` | あり（`-p`/`-t`） | `is_floating`等 | 0.44.3実機 | tiled↔floating切替も**プロセス保存**。hidden中もプロセス停止なし。`are-floating-panes-visible`はstdout true/false（exit 0/2を観測、help記載の1と相違） |
| stacked panes | `new-pane --stacked` / `stack-panes -- <ids>` | あり（ID列挙） | - | 0.44.3実機（存在確認のみ。動作詳細未検証） | 0.45.0で描画がリスト形式に変更（0.45.x docs） |
| plugin panes | `launch-plugin <url>` / `launch-or-focus-plugin` / `pipe` | pane ID返却 | `plugin_url`/`is_selectable` | 0.44.3実機 | non-selectable plugin（tab-bar等）は`list-panes`デフォルト除外。slot数勘定から除外が安全 |
| exit code / error | 各action | - | - | 0.44.3実機 | 引数不正はexit 2。存在しないtab IDへのclose等はexit 0で無操作（**サイレント成功**に注意）。`action --session`は構文エラー（`zellij --session NAME action ...`か`ZELLIJ_SESSION_NAME`を使用） |
| session resurrection | 1秒〜間隔でsessionをKDL serialize / `zellij attach` / `action save-session` | - | KDL file | 0.45.x docs（一部0.44.3実機でaction存在確認） | 実体はcommandのre-runでありプロセス継続ではない。zelperでは「復旧保険」として位置づけ |

## 3. 実験環境の再現

```bash
podman run --rm --network=none \
  -v /home/tomo/.local/share/mise/installs/zellij/latest/zellij:/usr/local/bin/zellij:ro \
  -v $PWD/tmp/phase1:/work -w /work \
  docker.io/library/debian:12-slim bash /work/exp-NN-*.sh
```

- zellijバイナリはstatically linked（glibc非依存）のためdebian:12-slimで動作
- client本体: `script -qec "stty rows 60 cols 200; zellij --session zelper-p1-XXX" /dev/null &`
- 外部操作: `zellij --session NAME action ...`（PTY不要）
- **config.kdlを配置しないとFirst Run Setup Wizard（plugin pane）が現れ、`-d`付きnew-pane連続発行時にpaneが作成直後に消える**。テスト環境では必ずconfig.kdlを置く
- プロセス保存検証: pane内で `bash /work/hb.sh <tag>`（0.5秒ごとに`timestamp tag i pid`を追記）を起動し、操作後にpidと連番iの継続を確認
- 実験済みの全スクリプト・生ログ: `tmp/phase1/local-experiments.md` §1の対応表参照

## 4. override-layout 詳細（remap中核）

0.44.3実機で確認した動作（出典: `tmp/phase1/out/04*.log`）:

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

### 4.1 複数tab layout適用とoverflow（`tmp/phase1/overflow-experiments.md`、out/p3-*.log）

| 実験 | 状態 | 結果 | プロセス |
|---|---|---|---|
| E1 | 6 pane/1 tab → 2-tab×3-slot、flagなし | tab構成はlayout通り2 tab化。**既存paneは第1tabの3 slotのみに投入**（t1/t2/t3保存）、第2tabは全slot新規shell、t0/t4/t5はkill | 部分保存・部分kill |
| E2 | 同状態 + retain | killなし・6 pane全保存。ただし**全paneが第1tab内に押し込まれlayout形状は崩壊**、第2tabは新規shell | 全保存 |
| E3 | 4 pane → 2-tab×3-slot | 分配はtab毎均衡でなく**第1tabへの前詰め**。t0 kill、第2tabは新規shell | 部分保存 |
| E4 | `zellij --layout <file> --session 既存` | **既存tab・pane・geometryに完全無影響**。2 tab追加（全pane新規shell） | 既存全保存 |
| E5 | 7 pane → 2-tab×3-slot | なし=余り4 pane kill。retain=全保存だが第1tab内入れ子（focus位置依存の割り込み分割） | 上記同様 |

**結論: override-layoutは既存paneを第1tabにしか割り当てない。追加tabは常に新規pane。したがって「M>N paneを追加tabでlayout反復・プロセス保存」は実現不可**。cross-tab移動actionも不存在（§2）のため事前分散も不可。代替は (a) 1 tab内slot一致適用（全保存）、(b) E4経路でのtab追加+プロセス再作成、(c) retain付き単tab適用の入れ子（形状はzellij自動配置に委ねる）の3つ。

副次発見: 200桁の表示領域では7 pane目の`new-pane -d right`が静かに失敗する（pane即消失）。テストハーネンスは十分な表示サイズを確保する。
- `--layout-string`は改行区切りKDL必須（§2参照）。positional引数はfile path。bare name（拡張子なし）はlayout_dirから解決

## 5. zelper設計への帰結

1. **remap**: `list-panes -a --json`で現状把握 → 生成KDLを`override-layout --layout-string --apply-only-to-active-tab --retain-existing-terminal-panes`でtab単位適用、の構成で成立。プロセス保存は実証済み。ただし(a) 超過paneの入れ子収容、(b) 割当順の非自明性、(c) barの喪失、(d) floating paneのkill、の4点はzelper側で明示的に扱う
2. **cross-tab移動はCLIに存在しない**。remapのoverflow（複数tabインスタンス）は「既存paneを別tabへ移す」ではなく、tab再編成の組合せで設計する必要がある（要件2.6のoverflow設計に直結する制約）
3. **session列挙にJSONがない**ため、zelperのsession listは`list-sessions`テキストparse（`-n`で色消し）を実装する
4. **tab IDは再利用される**ため安定キーにできず、操作の直後に取得したIDを即消費する設計にする。pane IDは安定
5. **サイレント成功**（存在しないIDへのclose等がexit 0）があるため、zelperは操作後にlist-panes/list-tabsでpostcondition検証を行う（basic-design 2.5のverify段階に対応）
6. resizeはno-opがあり得るため、反復は上限付き + `list-panes -g`での収束判定が必要
7. テストハーネンス要件: 隔離session + config.kdl配置（Setup Wizard抑止）+ PTY駆動。本実験のpodman構成をそのまま統合テスト基盤にできる

## 6. 未検証事項（Phase 3で必要に応じて追加実験）

~~複数tab layout適用の挙動~~ → 解決（§4.1に追加）。残る未検証:

- `paste` actionの0.44.3での実挙動（docsは0.45.x基準）
- swap layoutsの詳細動作（breadth first配置、`--layout-string`内へのswap node記述可否）
- stacked panesの実挙動・`resize`のstack対応
- `save-session`のserialize間隔と`dump-layout`の差分
- resize 1回あたりの刻み幅（%または行/列数）
- 複数クライアント接続時の`is_focused`・`other_focused_clients`の挙動
