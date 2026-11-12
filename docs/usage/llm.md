# zelper usage（LLM向け参照）

この文書はLLM/agentがzelperを文法エラー・誤用なく使うための機械的参照。人間向け説明は `zelper docs readme`。前提: `zellij` >= 0.44.3 がPATHにあること。

## 対象session解決（全verb共通）

`--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ > error（候補表示）。`--session` はglobal option（verbの前後どちらでも可）。`--session` 明示時は存在確認を先行し、不在名（EXITED名は存在扱いで透過）は `NoTarget` exit 3（candidates付き。zellijを呼ばない）。環境変数経由は検証せずそのまま透過する。実行中sessionが0個の自動解決も `NoTarget` exit 3（作成案内文面）。`list sessions` はsession解決を行わず、session 0個でも正常系（exit 0。humanはheaderのみ+stderr note・JSONは `sessions:[]`・compactは無出力）。

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
zelper add session NAME [--json]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab [TAB...] [--empty] [--yes] [--dry-run] [--json]
zelper move pane PANE (left|right|up|down|rotate) [--backwards] [--json]
zelper move pane PANE --to-tab TABSPEC [--json]        （3形式は相互排他かつちょうど1つ）
zelper move pane PANE --swap-with PANE [--json]
zelper move tab TABSPEC --swap-with TABSPEC [--json]
zelper [--session NAME] attach [--json]
zelper [--session NAME] detach [--json]
zelper watch [PANE...] [filters] [--regex] [--full] [--interval SECS] [--max N] [--json] -- PATTERN
zelper completion bash|zsh|fish
zelper docs readme | llm usage|skill|snippet
```

排他・依存規則（違反はusage error exit 2）:

- `list --compact`（`-c`）: human出力の縮小（panesは `TAB<TAB>短縮cwd` のみ・他resourceは名前のみ）。`--json` 併用時はJSONを優先しcompactは無視
- `send`: `--keys` はtext（`--`以降）と排他。`--enter` はtext指定時にのみ有効
- `remap`: layout名 / `--path` / `--inline` の3sourceは相互排他
- `add tab`: `--layout` / `--path` / `--inline` は相互排他
- `send` / `add` のcommand textは `--` 以降（`last = true`。`--`より前のoptionと混在可）
- `watch` のPATTERNは `--` 以降（v1はちょうど1件・空文字列拒否）。`--json` 等`--`より後ろはPATTERNと解釈されるためoptionは `--` の前に置く。`--interval` は1.0以上の有限値（Durationへ変換不能な巨大値も拒否）・`--max` は1以上・`--regex` のcompile errorはusage error
- `move pane`: action positional（direction/rotate）/ `--to-tab` / `--swap-with` は**三者相互排他かつちょうど1つ必須**。`--backwards` はaction=rotateとのみ併用可。`--swap-with` の相手と同一対象（自分自身）はusage error
- 引数検証（usage error, exit 2）はapp層でbackend初期化（version確認・session解決）の後に行われるため、zellij不在時は引数不正より `ZellijUnavailable`（exit 4）が優先する（全verb共通）
- `remove tab --empty` はTAB省略可。省略時は全空tab（selectable paneが0個のtab）が対象

既定値: `add pane/tab --count 1`、`resize pane STEPS 1`、`watch --interval 1.0 --max 30`。

`move` の意味論（単一対象の移動・交換・DD-13）:

- `move pane P (left|right|up|down|rotate)` はzellij `move-pane`のwrap。slot幾何はそのままにpaneの付け替えswapとして動き、pane ID・titleは不変。移動方向の端にいる場合はrc=0でも何も変わらないため、実行後に幾何を再取得して比較しno-opならwarning行 + exit 0（`data.noop=true`）
- `move pane P --to-tab T` はcompanion plugin（remapと共用）によるプロセス保存のtab間移動。pane ID不変（実行後検証・違反は `VerificationFailed` exit 7）。宛先==現tabならpipeなしのno-op（warning + exit 0）
- `move pane A --swap-with B` は2回のtab間移動によるpane交換（**cross-tabのみ**。同一tab内の位置交換はzellijに機構がなく `Preflight` exit 7 + remap誘導）。両paneとも当該tabの最後のselectable paneである構成（両singleton tab）は順序を変えてもtab自動closeを回避できないため `Preflight` exit 7で拒否し、`move tab --swap-with`（tab名・tab IDの帰属は入れ替わらない点がpane交換と異なる）か`remap`を案内。途中失敗は移動済み/未移動+手動完了hint（`zelper move pane B --to-tab T`）を報告し、**atomicityは主張しない**
- `move tab A --swap-with B` は `move-tab -t` の繰返し合成（距離dに対しright×d + left×(d-1)の2d-1回）で両tabのpositionを入れ替える。tab ID・tab名は不変（実行後検証）。focusは不変
- move対象はtiled terminal paneのみ: floating paneは `Preflight` exit 7（`remap --embed-floating` でtiled化してから）。plugin pane・exited/held paneも事前遮断

`watch` の意味論: 対象pane（複数可・any-hit）の出力を`--interval`秒間隔でpolling（`dump-screen`反復）し、`-- PATTERN`を含む行が出現したら即hit（exit 0）。attempt 1はsleepなし即時実行のため開始前の既出テキストもhit（`--full`でscrollback込み）。照合はdump-screen生テキストの行単位（既定は固定substring・`--regex`で正規表現。ANSI正規化なし）。hit paneはvisual order最初の1件。`--max`回到達でno hitなら失敗。

`attach` / `detach` / `add session` の意味論（session lifecycle・DD-14）:

- `attach`: 対話client起動。**stdin・stdoutの両方がTTYであること必須**（非TTYはzellij起動前に `Preflight` exit 7で拒否。agent/script実行からは使えない。`--session` 不在名・session 0個の `NoTarget` exit 3はこのTTY検査より先に立つ）。対話終了後のみ報告（human `attach session 'NAME' ended` / JSON `data.session`）。zellij非zero終了は `OperationFailed` exit 5
- `detach`: 対象sessionのattach clientをdetach。client不在でも成功（no-op）扱い（human `detached clients from session 'NAME'` / JSON `data.session`）。複数clientがattach中の場合、detachはzellijの仕様として最新のattach clientのみ切断し、先発clientは画面凍結のまま残留する（M3実測: 反復detachでも先発clientは残留するため、全client切断の手段としては機能しない）
- `add session NAME`: detached session作成（`attach -b NAME` 相当の非対話経路）。session 0件環境でも動作（対象session解決を経ない例外dispatch）。同名running session既存は `Preflight` exit 7（fail-fast）・作成呼出失敗は `OperationFailed` exit 5・作成後のlist-sessions出現確認は `VerificationFailed` exit 7。成功は human `created session 'NAME' (detached)` / JSON `data.session`・`data.created=true`。作成直後は空session（tab 1・pane 0件）でfirst attach時にdefault layoutが具現化する。layout適用は作成後の `remap` 合成（`zelper add session work && zelper --session work remap three`）で行う

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
- `remap` は単一のenvelope。成功時の `data` は `m` / `n`（= S: layout全体slot数）/ `k` / `t`（tab鋳型数）/ `s` / `n_slots`（tab毎slot数列）・`mapping[]`（pane / block〔instance keyは維持〕 / tab_index / slot / tab / preserved / alive）・`tabs[]`（生成tabのblock/tab_index/id/name）・`leftover_tabs[]`（remap対象外の残存tab。closeされない）・`closed_duplicates[]`（run一致不成立でzelperがspawnしcloseした複製pane。pane/slot/reason。成功・失敗双方で報告）・`source_excluded_panes[]`（対象tab内に残留するsource外のexited/held pane。pane/tab/title/exited/held。closeされない）・`warnings`・`snapshot_len`。検証失敗時は `error.data` にm/n/k/t/n_slots・mapping・missing・`leftover_tabs`・`closed_duplicates`・`source_excluded_panes` が載る。`--dry-run` の `data` は `dry_run` / `source[]`（visual order・`invoked_with`含む）・M/S/k/T/`n_slots`・`tabs[]`（鋳型name/focus）・割当表（`instances[]`〔block×tab毎のassignments〕）・生成tab毎のKDL preview（`instances[].kdl`・occupied slotにはcommand+args+cwdを注入）・操作列（`operations[]`・new-tab〔空group〕/focus-pane-id/最終go-to含む）・`warnings` で、状態は一切変更しない
- `list sessions` のみJSONをzellijが提供しないためテキストparse。name / created（zellijの相対表記）/ current / panes_per_tab（tab毎tiled pane数。取得失敗時null）を返す。EXITED（dead session）は表示・session解決から除外される
- `list layouts` の `layouts` は `{name, size, modified_epoch}` の配列（stat失敗時はsize/modified_epochがnull）
- `watch` のhit時 `data` は `{ target, pattern, regex, attempts_used, elapsed_secs, matched_lines, pane_errors }`（DD-15）。**multi-targetでもresults[]でなくhit pane単一のobject**（DD-4.2一般規定の例外）。回数切れの `error.data` は `{ attempts_used, elapsed_secs, pattern }`（一部pane消失を伴うPartialFailure時は失敗paneのみの `results[]`）
- `move pane` の方向移動・rotateの成功 `data` は `{ target, action }`（rotateは `backwards` 追加・no-op時は `noop: true` 追加）。`--to-tab` は `{ target, action: "to-tab", tab, tab_name }`。pane交換は `{ action: "swap-pane", panes: [{id, tab}, ...] }`。tab交換は `{ action: "swap-tab", tabs: [{id, position}, ...] }`（DD-13）
- `attach` / `detach` の成功 `data` は `{ "session": "NAME" }`。`add session` の成功 `data` は `{ "session": "NAME", "created": true }`（DD-14）

## exit codeとerror class対応

| exit | class（JSON `error.class`の値） | 意味 |
|---|---|---|
| 0 | - | 成功 |
| 2 | `Usage` | 引数・文法・排他規則違反 |
| 3 | `NoTarget` / `AmbiguousTarget` | 対象解決失敗（0件 / 複数ヒット。candidates参照） |
| 4 | `ZellijUnavailable` / `UnsupportedVersion` | zellij不在 / version < 0.44.3 または未実証の上位series（0.45.0等） |
| 5 | `OperationFailed` | zellij操作の失敗（remapのprobe不成立・移動timeout・watchの試行回数切れを含む。いずれもpaneは無傷） |
| 6 | `PartialFailure` | multi-targetの一部失敗（results[]参照。watchの回数切れ+一部pane消失を含む） |
| 7 | `LayoutNotFound` / `LayoutInvalid` / `Preflight` / `VerificationFailed` | layout不在 / KDL不正 / 事前条件違反 / 適用後検証不一致 |

class名はPascalCaseで固定。stdoutはJSON（または人間可読テキスト）、診断はstderr。

## 安全機構

- `remove`: 対象2件以上（または `--empty`）は破壊的とみなし、`--yes` か `--dry-run` がない限りerror（`Preflight` exit 7のgate。単一対象は即実行）
- `remove --dry-run`: 削除計画を表示して実行しない
- `remap` は実行開始前に存在したpaneのkill/restartを行わない（run一致不成立でzelper自身がspawnした複製pane・開始前snapshotに存在しないidで対応規則によりdupと特定されたもののみcloseし、`closed_duplicates` で報告）。移動が必要な場合はprobe（companion plugin応答性確認）成功後に状態変更を開始し、不成立なら一切の状態変更前に中断
- `--dry-run` は一切のsession状態を変更しない（tab切替・companion pluginのwasm展開・permissions.kdl書換も発生しない）

## remap意味論要点

- 既存paneのprocessを**すべて**保持したままlayoutへ再配置。対象はsession全体のselectable・tiled terminal pane（`--tab` はsource絞り込み兼anchor指定。この場合layout最初のtab鋳型による単tab適用）
- 反復単位はlayout全体（T tab鋳型・S = 全tabのslot数和）。pane数 M > S でもerrorにならない: k = max(1, ceil(M/S)) block × T tabを生成し全paneを配置。kill/restart経路は存在しない
- 生成tab (b,t)=(0,0) = anchor tab（`--tab`指定時そのtab・省略時active tab・tab名は保持）。それ以外の生成tab名は幹 + `-<b+1>`（b >= 1 のblock接尾。幹は名前ありtab鋳型のname / base〔layout名・file stem・`remap`（`--inline`）〕。名無し鋳型はzelper命名）。tab名・tab focus（focus=true鋳型のblock 0 tabへ最終go-to）・pane focus（`focus-pane-id`）・`default_tab_template`由来barを`zellij --layout`起動時と同一に再現（anchor名は保持例外）
- 割当順はvisual order（tab position → y → x）で決定的。割当0件のgroup（空group）は `new-tab --layout-string` で生成。空slotは既定shellで埋まる。移動で空になったtabは自動close
- occupied slotはrun（command+args+cwd）一致照合で配置: 生成KDLに `terminal_command` 復元のcommand+argsとcwd（pane_cwd）を注入する。照合不成立でspawnした複製paneは、適用後の安定観測と多段規則（command dup=argv一致逆引き・bare dup=pane_cwd属性照合+数突合+一意性判定）で特定してcloseし、該当shell slotはcwd付き表現へ反転して再適用する（`closed_duplicates` で報告）
- 割当ありgroupのcross-tab移動が必要な場合のみcompanion plugin（binary埋込wasm + permissions.kdl seed）を使用し、probe成功後に移動（空groupのnew-tabのみならplugin不使用）。probe不成立は状態変更前に中断（`OperationFailed` exit 5）
- floating pane存在時の実行は `Preflight` error（exit 7）。`--embed-floating` でtiled化（process保持）して組入れ。`--dry-run` のみfloatingがあっても仮想embedの計画と警告でexit 0（実行は変わらずpreflight error）
- 適用後、pane ID生存・割当tab所属・各生成tabのpane数（== N_t）・pane focus（is_focused）・run一意割当の位置照合・割当slot位置への新規spawn pane残存（exited/held込み）を検証。不一致は `VerificationFailed`（exit 7）。remap対象外の過剰tabはcloseされず `leftover_tabs` として報告
- atomicityは主張しない。途中失敗は実行済み/失敗/未実行を報告

## 誤用と正解

| 誤り | 正しい |
|---|---|
| `zelper send 3 y` | `zelper send 3 -- y`（textは `--` 以降） |
| `zelper rename 3 name` | `zelper rename pane 3 name`（noun必須） |
| `zelper resize pane 3 grow` | `zelper resize pane 3 grow right`（方向まで必須） |
| `zelper remap three --inline '...'` | layout名と `--inline` は排他。どちらか一方 |
| `zelper send --keys Enter -- y` | `--keys` とtextは排他 |
| `zelper watch 3 READY` / `zelper watch 3 -- READY --json` | PATTERNは `--` 以降のためoptionは `--` の前へ: `zelper watch 3 --json -- READY` |
| `zelper move pane 3 left --to-tab 1` | directionと `--to-tab` は三者排他。どちらか一方 |
| `zelper move pane 3 --swap-with 3`（自分自身）/ 同一tab内の2 pane交換 | 自身との交換はusage error。同一tab内は機構がなく `Preflight` exit 7（`remap` を使う） |
| 非TTY環境（script/agent実行）での `zelper attach` | `Preflight` exit 7で拒否される。session作成は `zelper add session NAME`（非対話）を使い、attachは実TTYで行う |
| 対象名が曖昧なまま再試行 | errorの `candidates` のIDを使う |

## 既知の制限（agentが前提にしてはならないこと）

- remapのslot割当はgroup（block×tab所属）とrun（command+args+cwd）が一意なpaneのslot対応に限り決定的。同一runの複数pane間・複数shell pane間のslot順は保証外
- `terminal_command`（`invoked_with`）に `"` `'` `\` 改行・制御文字を含むpane・shell `-c` 起因と疑われるpaneは、空白分割で引数境界を復元できずslot照合を外しうる（warning付きで実行され、適用後検証で検出）。cwd注入後も、起動後のchdir・path正規化差によりpane_cwd≠起動時cwdとなったpaneは照合を外しうる
- `resize` は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない
- `move` は単一対象操作（全体再配置は `remap`）。pane交換（`--swap-with`）のatomicityは主張しない（2回の移動の間で失敗しうる。部分状態の報告と手動完了hintで補完）。対象はtiled terminal paneのみ（floatingは `remap --embed-floating` でtiled化してから）
- `watch` はpolling実装（`dump-screen`反復・既定1s間隔）。最小intervalは1sのため高速pollは不可能。対象解決は開始時1回のみ（監視中に追加されたpaneは対象外）。照合はdump-screen生テキストの行単位でANSI escapeの正規化は行わない
- `attach` は対話clientのため実TTY前提（stdin・stdout両方）。複数clientがattach中のsessionへの `detach` はzellijの仕様として最新のattach clientのみ切断し、先発clientは画面凍結のまま残る（M3実測: 反復detachでも先発clientは残留）。`add session` で作成直後のsessionは空（tab 1・pane 0件）で、first attach時にdefault layoutが具現化する。作成直後に `remap` する場合は空session（M=0）への適用になる点に注意
- tab IDはclose後に再利用される。取得したtab IDは即時使用のみ
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。config.kdlの `layout_dir` は読まない
