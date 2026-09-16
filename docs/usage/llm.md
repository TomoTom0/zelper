# zelper usage（LLM向け参照）

この文書はLLM/agentがzelperを文法エラー・誤用なく使うための機械的参照。人間向け説明は `zelper docs readme`。前提: `zellij` >= 0.44.3 がPATHにあること。

## 対象session解決（全verb共通）

`--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ > error（候補表示）。`--session` はglobal option（verbの前後どちらでも可）。

## 対象指定（read / send / resize / remove等）

- PANESPEC = pane ID。`terminal_3` / `plugin_1` / bare `3`（`3`は`terminal_3`と同義）。複数指定可
- filter option（read/send共通）: `--tab` / `--name`（pane title完全一致）/ `--command`（pane command部分一致）/ `--cwd` / `--all`（selectable terminal pane全て。plugin paneは含まない。selectableなfloating terminal paneはtiledと同じく含む）
- 対象 = positional PANESPEC群とfilterの和集合。空なら `NoTarget` error（exit 3）
- TABSPEC = tab ID（整数）または一意なtab名。一意でなければ `AmbiguousTarget` error（candidatesにtab ID列）
- 単一対象を要求する操作（rename等）で複数ヒットは `AmbiguousTarget`（exit 3、candidatesにpane ID列）
- 並び順は常に決定的（tab position → y → x のvisual order）

## 全verb文法

```text
zelper list sessions|tabs|panes|layouts [--tab T] [--json] [-c|--compact]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME [--json]
zelper rename tab TAB NAME [--json]
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS] [--json]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...] [--json]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...] [--json]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab [TAB...] [--empty] [--yes] [--dry-run] [--json]
zelper completion bash|zsh|fish
zelper docs readme | llm usage|skill|snippet
```

排他・依存規則（違反はusage error exit 2）:

- `list --compact`（`-c`）: human出力の縮小（panesは `TAB<TAB>短縮cwd` のみ・他resourceは名前のみ）。`--json` 併用時はJSONを優先しcompactは無視
- `send`: `--keys` はtext（`--`以降）と排他。`--enter` はtext指定時にのみ有効
- `remap`: layout名 / `--path` / `--inline` の3sourceは相互排他
- `add tab`: `--layout` / `--path` / `--inline` は相互排他
- `send` / `add` のcommand textは `--` 以降（`last = true`。`--`より前のoptionと混在可）
- `remove tab --empty` はTAB省略可。省略時は全空tab（selectable paneが0個のtab）が対象

既定値: `add pane/tab --count 1`、`resize pane STEPS 1`。

## JSON出力契約（`--json`）

成功:

```json
{"schema_version": 1, "ok": true, "data": ...}
```

失敗:

```json
{"schema_version": 1, "ok": false, "error": {"class": "...", "message": "...", "candidates": [...], "data": ...}}
```

- `candidates` は候補があるerrorに付く（pane/tab IDのほか、session曖昧時はsession名、remapのfloating pane検出時もpane ID列）。空なら省略
- `data` は部分失敗時のper-target結果等が付く場合のみ
- multi-target操作（read / send / remove）の `data` は `results[]`。各要素は:

```json
{"target": "terminal_3", "ok": true, "detail": ..., "error": "..."}
```

- `detail` / `error` は該当時のみ。部分失敗は隠されない（一部失敗でexit 6、全対象失敗はexit 5）
- `remap` は単一のenvelope。成功時の `data` は `m` / `n`（= S: layout全体slot数）/ `k` / `t`（tab鋳型数）/ `s` / `n_slots`（tab毎slot数列）・`mapping[]`（pane / block〔instance keyは維持〕 / tab_index / slot / tab / preserved / alive）・`tabs[]`（生成tabのblock/tab_index/id/name）・`leftover_tabs[]`（remap対象外の残存tab。closeされない）・`warnings`・`snapshot_len`。検証失敗時は `error.data` にm/n/k/t/n_slots・mapping・missingが載る。`--dry-run` の `data` は `dry_run` / `source[]`（visual order・`invoked_with`含む）・M/S/k/T/`n_slots`・`tabs[]`（鋳型name/focus）・割当表（`instances[]`〔block×tab毎のassignments〕）・生成tab毎のKDL preview（`instances[].kdl`）・操作列（`operations[]`・new-tab〔空group〕/focus-pane-id/最終go-to含む）・`warnings` で、状態は一切変更しない
- `list sessions` のみJSONをzellijが提供しないためテキストparse。name / created（zellijの相対表記）/ current / panes_per_tab（tab毎tiled pane数。取得失敗時null）を返す。EXITED（dead session）は表示・session解決から除外される
- `list layouts` の `layouts` は `{name, size, modified_epoch}` の配列（stat失敗時はsize/modified_epochがnull）

## exit codeとerror class対応

| exit | class（JSON `error.class`の値） | 意味 |
|---|---|---|
| 0 | - | 成功 |
| 2 | `Usage` | 引数・文法・排他規則違反 |
| 3 | `NoTarget` / `AmbiguousTarget` | 対象解決失敗（0件 / 複数ヒット。candidates参照） |
| 4 | `ZellijUnavailable` / `UnsupportedVersion` | zellij不在 / version < 0.44.3 または未実証の上位series（0.45.0等） |
| 5 | `OperationFailed` | zellij操作の失敗（remapのprobe不成立・移動timeoutを含む。いずれもpaneは無傷） |
| 6 | `PartialFailure` | multi-targetの一部失敗（results[]参照） |
| 7 | `LayoutNotFound` / `LayoutInvalid` / `Preflight` / `VerificationFailed` | layout不在 / KDL不正 / 事前条件違反 / 適用後検証不一致 |

class名はPascalCaseで固定。stdoutはJSON（または人間可読テキスト）、診断はstderr。

## 安全機構

- `remove`: 対象2件以上（または `--empty`）は破壊的とみなし、`--yes` か `--dry-run` がない限りerror（`Preflight` exit 7のgate。単一対象は即実行）
- `remove --dry-run`: 削除計画を表示して実行しない
- `remap` に破壊的経路は存在しない（kill/restartなし）。移動が必要な場合はprobe（companion plugin応答性確認）成功後に状態変更を開始し、不成立なら一切の状態変更前に中断
- `--dry-run` は一切のsession状態を変更しない（tab切替・companion pluginのwasm展開・permissions.kdl書換も発生しない）

## remap意味論要点

- 既存paneのprocessを**すべて**保持したままlayoutへ再配置。対象はsession全体のselectable・tiled terminal pane（`--tab` はsource絞り込み兼anchor指定。この場合layout最初のtab鋳型による単tab適用）
- 反復単位はlayout全体（T tab鋳型・S = 全tabのslot数和）。pane数 M > S でもerrorにならない: k = max(1, ceil(M/S)) block × T tabを生成し全paneを配置。kill/restart経路は存在しない
- 生成tab (b,t)=(0,0) = anchor tab（`--tab`指定時そのtab・省略時active tab・tab名は保持）。それ以外の生成tab名は幹 + `-<b+1>`（b >= 1 のblock接尾。幹は名前ありtab鋳型のname / base〔layout名・file stem・`remap`（`--inline`）〕。名無し鋳型はzelper命名）。tab名・tab focus（focus=true鋳型のblock 0 tabへ最終go-to）・pane focus（`focus-pane-id`）・`default_tab_template`由来barを`zellij --layout`起動時と同一に再現（anchor名は保持例外）
- 割当順はvisual order（tab position → y → x）で決定的。割当0件のgroup（空group）は `new-tab --layout-string` で生成。空slotは既定shellで埋まる。移動で空になったtabは自動close
- 割当ありgroupのcross-tab移動が必要な場合のみcompanion plugin（binary埋込wasm + permissions.kdl seed）を使用し、probe成功後に移動（空groupのnew-tabのみならplugin不使用）。probe不成立は状態変更前に中断（`OperationFailed` exit 5）
- floating pane存在時は `Preflight` error（exit 7）。`--embed-floating` でtiled化（process保持）して組入れ
- 適用後、pane ID生存・割当tab所属・各生成tabのpane数（== N_t）・pane focus（is_focused）を検証。不一致は `VerificationFailed`（exit 7）。remap対象外の過剰tabはcloseされず `leftover_tabs` として報告
- atomicityは主張しない。途中失敗は実行済み/失敗/未実行を報告

## 誤用と正解

| 誤り | 正しい |
|---|---|
| `zelper send 3 y` | `zelper send 3 -- y`（textは `--` 以降） |
| `zelper rename 3 name` | `zelper rename pane 3 name`（noun必須） |
| `zelper resize pane 3 grow` | `zelper resize pane 3 grow right`（方向まで必須） |
| `zelper remap three --inline '...'` | layout名と `--inline` は排他。どちらか一方 |
| `zelper send --keys Enter -- y` | `--keys` とtextは排他 |
| 対象名が曖昧なまま再試行 | errorの `candidates` のIDを使う |

## 既知の制限（agentが前提にしてはならないこと）

- remapのslot割当はgroup（block×tab所属）とcommand+argsが一意なpaneのslot対応に限り決定的。同一commandの複数pane間・複数shell pane間のslot順は保証外
- `terminal_command`（`invoked_with`）に `"` `'` `\` 改行・制御文字を含むpaneは、空白分割でquotingを復元できずslot照合を外しうる（warning付きで実行され、適用後検証で検出）
- `resize` は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない
- tab IDはclose後に再利用される。取得したtab IDは即時使用のみ
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。config.kdlの `layout_dir` は読まない
