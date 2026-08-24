# zelper usage（LLM向け参照）

この文書はLLM/agentがzelperを文法エラー・誤用なく使うための機械的参照。人間向け説明は `zelper docs readme`。前提: `zellij` >= 0.44.1 がPATHにあること。

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
zelper list sessions|tabs|panes|layouts [--tab T] [--json]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME [--json]
zelper rename tab TAB NAME [--json]
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS] [--json]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T | --session-scope] [--overflow nest|tabs] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...] [--json]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...] [--json]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab [TAB...] [--empty] [--yes] [--dry-run] [--json]
zelper completion bash|zsh|fish
zelper docs readme | llm usage|skill|snippet
```

排他・依存規則（違反はusage error exit 2）:

- `send`: `--keys` はtext（`--`以降）と排他。`--enter` はtext指定時にのみ有効
- `remap`: layout名 / `--path` / `--inline` の3sourceは相互排他。`--tab` と `--session-scope` は排他
- `add tab`: `--layout` / `--path` / `--inline` は相互排他
- `send` / `add` のcommand textは `--` 以降（`last = true`。`--`より前のoptionと混在可）
- `remove tab --empty` はTAB省略可。省略時は全空tab（selectable paneが0個のtab）が対象

既定値: `add pane/tab --count 1`、`resize pane STEPS 1`、`remap --overflow` 無指定時のoverflowはerror。

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
- `remap --session-scope` はresults[]を生成しない。tab毎に独立したenvelopeを順に出力し、失敗時は最初のerrorで中断（それまでに適用したtabは残る）
- `list sessions` のみJSONをzellijが提供しないためテキストparse。session名は正確、付帯情報（作成時刻等）は簡略。過信しないこと

## exit codeとerror class対応

| exit | class（JSON `error.class`の値） | 意味 |
|---|---|---|
| 0 | - | 成功 |
| 2 | `Usage` | 引数・文法・排他規則違反 |
| 3 | `NoTarget` / `AmbiguousTarget` | 対象解決失敗（0件 / 複数ヒット。candidates参照） |
| 4 | `ZellijUnavailable` / `UnsupportedVersion` | zellij不在 / version < 0.44.1 |
| 5 | `OperationFailed` | zellij操作の失敗 |
| 6 | `PartialFailure` | multi-targetの一部失敗（results[]参照） |
| 7 | `LayoutNotFound` / `LayoutInvalid` / `Preflight` / `VerificationFailed` | layout不在 / KDL不正 / 事前条件違反 / 適用後検証不一致 |

class名はPascalCaseで固定。stdoutはJSON（または人間可読テキスト）、診断はstderr。

## 安全機構

- `remove`: 対象2件以上（または `--empty`）は破壊的とみなし、`--yes` か `--dry-run` がない限りerror（`Preflight` exit 7のgate。単一対象は即実行）
- `remove --dry-run`: 削除計画を表示して実行しない
- `remap --overflow tabs`: overflow paneをcloseしてcommand再起動（そのpaneのみ破壊的）。`--dry-run` で保存/再作成の別を事前確認
- `--dry-run` は一切のsession状態を変更しない（tab切替もしない）

## remap意味論要点

- 既存paneのprocessを保持したままlayoutへ再配置。適用はtab単位、他tabは保護
- pane数 M <= slot数 N: 全pane保存。空slotは既定shellで埋まる
- M > N: 既定は `Preflight` error。`--overflow nest`（全pane保存、第1tab内入れ子、layout形状不保証）/ `--overflow tabs`（layout反復、overflow paneは再起動）
- floating pane存在時は `Preflight` error。`--embed-floating` でtiled化（process保持）して組入れ
- 適用後、pane ID生存（tabs modeでは再作成paneのcommand一致）を検証。不一致は `VerificationFailed`（exit 7）
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

- 既存paneを別tabへprocess保持で移す手段はZellijに存在しない
- `resize` は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない
- `--overflow tabs` の再構成commandは `pane_command` の空白分割のため、引用を含むcommandは崩れうる
- tab IDはclose後に再利用される。取得したtab IDは即時使用のみ
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。config.kdlの `layout_dir` は読まない
