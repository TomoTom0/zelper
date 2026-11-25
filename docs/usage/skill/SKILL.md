---
name: zelper
description: Zellij構造化操作CLI zelperの使い方。zellij sessionのpane/tab構造の読み取り、複数paneへの一括入力送信、layout再配置（remap）、rename、resize、pane/tab/sessionの追加削除を行うときに使用。zellijやtmux的な多pane作業環境の操作、coding agent稼働paneの管理、画面内容の取得、多pane一括コマンド送信に遭遇したら自律的にロードする。
---

# zelper - Zellij構造化操作CLI

zelperはzellij sessionをverb-first文法で操作するCLI。`zellij`バイナリ（>= 0.44.3）をbackendとして呼ぶ。

## 実行原則

- 対象session: `--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ > error（候補表示）
- 破壊的操作（`remove`）はまず `--dry-run --json` で計画を確認する。`remap` は既存paneのkill/restartを行わないが（照合不成立でzelperがspawnした複製paneのcloseのみ・`closed_duplicates`で報告）、多pane再配置では `--dry-run --json` で計画（M/S/k・割当・生成KDL）を確認する
- 構造の把握は `zelper list panes --json` から始める（pane ID / title / command / 位置を一括取得）

## 対象指定（全verb共通）

- positional = pane ID（`terminal_3` / `plugin_1` / bare `3`。`3`は`terminal_3`と同義）。複数指定可
- filter option: `--tab`（tab ID or 一意な名前）/ `--name`（pane title完全一致）/ `--command`（pane command部分一致）/ `--cwd` / `--all`
- positionalとfilterの和集合が対象。単一対象を要求する操作で複数ヒットは `ambiguous target` error（候補列出）
- 並び順は常に決定的（tab position → y → x のvisual order）

## verb一覧

```text
zelper list sessions|tabs|panes|layouts [--tab T] [--json] [-c|--compact]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME
zelper rename tab TAB NAME
zelper rename session NAME NEW_NAME
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...]
zelper add session NAME [--layout NAME|PATH] [--cwd DIR] [--json]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab TAB... [--empty] [--yes] [--dry-run] [--json]
zelper remove session NAME... [--yes] [--dry-run] [--json]
zelper remove session (--all | --dead | --me) [--yes] [--dry-run] [--json]
zelper move pane PANE (left|right|up|down|rotate) [--backwards] [--json]
zelper move pane PANE --to-tab TAB | --swap-with PANE   （3形式は相互排他）
zelper move tab TAB --swap-with TAB [--json]
zelper [--session NAME] attach [--json]      （対話attach。TTY必須）
zelper [--session NAME] detach [--json]
zelper watch [PANE...] [filters] [--regex] [--full] [--interval SECS] [--max N] [--json] -- PATTERN
zelper completion bash|zsh|fish
zelper docs readme | llm usage|skill|snippet
```

誤用しやすい点:

- `list --compact`（`-c`）はhuman出力の縮小（panesはtab+短縮cwdのみ・他は名前のみ）。`--json` 併用時はJSON優先
- `send` のtext指定は `--` 区切り必須: `zelper send --command codex -- y`（`-- y`を忘れるとusage error exit 2）
- `rename` はnoun形式: `zelper rename pane 3 build` / `zelper rename tab agents work` / `zelper rename session old new`
- `resize` は方向まで指定: `zelper resize pane 3 grow right 2`
- `remap` のlayout指定は名前 / `--path` / `--inline` の3source相互排他。併用はusage error
- `move` は単一pane/tabの移動・交換（全体再配置はremap）: `zelper move pane 3 left`（方向移動・rotate。slot間swap・端ではno-op warning+exit 0）/ `zelper move pane 3 --to-tab 1`（tab間移動・pane ID不変）/ `zelper move pane 3 --swap-with 5`（pane交換・cross-tabのみ）/ `zelper move tab 1 --swap-with 3`（tab交換・position入替）。direction・--to-tab・--swap-withは三者排他かつちょうど1つ。同一tab内のpane交換・両tab最後のpane同士の交換はexit 7（remapかtab交換を案内）。floating paneはexit 7（`remap --embed-floating`でtiled化してから）
- `watch` のPATTERNは `--` 区切り必須で1件（`zelper watch --command codex -- READY`）。optionは `--` の前に置く。 pollingは既定1s間隔・最大30回で、開始前の既出テキストもhitする
- session lifecycle: `add session NAME` はdetached sessionの非対話作成（session 0件環境でも動作・同名runningはexit 7）。`attach` は実TTY前提（stdin・stdout両方がTTYでないとexit 7。agent実行からは使えない）。`detach` はclient不在でもno-op成功。複数clientがattach中のdetachは最新clientのみ切断される（先発clientは残る）
- `remove session` はsessionの完全削除（行もresurrection dataも消す。selectorは `NAME...` / `--all` / `--dead` / `--me` の4者排他かつちょうど1つ）。**単一対象でも常に `--yes` か `--dry-run` が必要**（gateがpane/tabと異なり常時）。EXITED行も受理。自sessionの削除は `--me` のみ（NAMEに自session〔ZELLIJ_SESSION_NAME〕を含めるとexit 7・`--all` は自sessionを除外してwarning）。`--me` は報告・exit statusが失われうるbest-effort契約（実行前にstderr warning）。`--all` で自分を巻き込まないよう注意
- `--tab agents` のような名前指定は一意な名前のみ。同名tabが複数あればtab IDを使う

## 出力とexit status

`--json` は `schema_version:1` / `ok` / `data`（または `error.class`）を返す。multi-target操作は `results[]` にper-target結果を格納し、部分失敗を隠さない。`watch` のhit時は例外として `results[]` でなくhit pane単一のobject（`target` / `matched_lines` 等。障害paneは `pane_errors`）。回数切れはexit 5・一部pane消失はexit 6。

```text
0 成功 / 2 usage / 3 対象解決失敗(no target|ambiguous) / 4 zellij不在|未サポートversion
5 操作失敗 / 6 部分失敗 / 7 preflight・layout不正・検証失敗
```

## remapの意味論（誤用注意）

既存の動いているpaneのprocessを**すべて**保持したまま、指定layoutに再配置する。対象はsession全体のselectable・tiled terminal pane（`--tab`指定時はそのtabのpaneに絞り込み、そのtabがanchor）。

- 反復単位はlayout全体（T tab鋳型・S = 全tabのslot数和）。pane数 M > S でもerrorにならない: k = max(1, ceil(M/S)) block × T tabを生成して全paneを配置。kill/restart経路は存在しない
- 生成tab (0,0)はanchor tab（省略時active tab。tab名は保持）。それ以外の生成tab名は幹（名前ありtab鋳型のname / base〔layout名・file stem・`remap`〕）+ `-<b+1>` block接尾。tab名・tab focus・pane focus・`default_tab_template`由来barを`zellij --layout`起動時と同一に再現。移動で空になったtabは自動close。remap対象外の過剰tabはcloseされず報告（`leftover_tabs`）
- slot配置はrun（command+args+cwd）一致照合（occupied slotの生成KDLにはcwdを注入）。照合不成立でspawnした複製paneは適用後に特定してcloseし`closed_duplicates`で報告する
- 移動が必要な場合のみcompanion plugin（binary埋込wasm + permissions.kdl seed）を使用。probe不成立なら状態変更前に中断
- floating paneが存在すると実行はerror。`--embed-floating` でtiled化（process保持）してから組入れ（`--dry-run` は仮想embed計画+警告でexit 0）
- atomicityは主張しない。途中失敗時は実行済み/失敗/未実行を報告

## 既知の制限

- remapのslot割当はgroup（block×tab所属）とrun（command+args+cwd）が一意なpaneのslot対応に限り決定的。同一runの複数pane間・複数shell pane間のslot順は保証しない
- `terminal_command`（`invoked_with`）に `"` `'` `\` 改行・制御文字を含むpane・shell `-c` 起因と疑われるpaneは引数境界を復元できずslot照合を外しうる（warning付きで実行され、適用後検証で検出）
- `resize` は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない
- `list sessions` のJSONはzellijが提供しないためテキストparse（name / created / current / panes_per_tab を返す。EXITED な dead session は表示・session解決から除外される）。空白を含むsession名（zellij直接操作で既存の場合）は先頭wordのみ識別され、状態判定・PANES/`panes_per_tab` 取得・`remove session --all`/`--dead` の対象化に影響する（zellerからの作成・rename新名は空白不含等のみでusage error事前拒否）
- tab IDはclose後に再利用されるため、取得したtab IDは即時使用のみに用いる
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。zellij本体の解決（config.kdlの`layout_dir`）と一致させること

## 典型ユースケース

```text
zelper list panes --json                      # 構造把握（最初の一歩）
zelper read --tab agents                      # agents tabの全pane画面を読む
zelper send --command codex -- y              # codex稼働pane全てにyを送る
zelper watch --command codex -- READY         # いずれかのcodex paneにREADY出現まで待つ
zelper move pane 3 --to-tab agents            # pane 3をagents tabへ移動（process保持）
zelper move pane 3 --swap-with terminal_9     # pane交換（cross-tab）
zelper add session work                       # detached session作成（非対話・0件環境可）
zelper --session work add pane --name build   # 作成したsessionへのpane追加
zelper remap three --dry-run --json           # 3分割layout再配置の計画確認
zelper add pane --name build -- cargo build   # build用paneを追加してコマンド実行
zelper remove pane 12                         # pane削除（2対象以上で--yes確認要求）
zelper remove session --dead --yes            # EXITED行（dead session）のみ掃除（常時--yes必須）
```
