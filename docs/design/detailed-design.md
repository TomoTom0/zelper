# zelper Detailed Design (Phase 3)

作成日: 2026-08-21
前提: requirements.md / basic-design.md / docs/research/zellij-capabilities.md（0.44.3実機検証）/ docs/design/requirements-traceability.md
本章番号DD-1〜DD-12はrequirements-traceability.md §3で規定したもの

## DD-1. CLI grammar

### 1.1 凍結されたtop-level verb set（v1）

```text
list  read  send  rename  resize  remap  add  remove
```

例外的にtop-levelに許容する非verbコマンド: `completion`（shell補完生成。`zelper completion bash|zsh|fish`）。helpの発見性目的で、文書化された例外として扱う。`docs`（agent向け配布物の出力。`zelper docs readme | llm usage|skill|snippet`）も同様の文書化された例外（配布物の取り出し経路。zellij sessionに触れない純粋出力）。

### 1.2 構文木（全体）

```text
zelper [--session NAME] [--version] <verb> ...

# 対象session解決（session解決を必要とするverb共通: list tabs/panes・read/send等の操作系）
--session NAME > 環境変数 ZELLIJ_SESSION_NAME > 実行中sessionが1つのときそれ > error（候補表示）
# 「実行中」はlist-sessions -nのEXITED行（resurrection待ちdead session）を除外して数える。
# list sessions / list layouts は一覧表示自体が目的のためsession解決を行わない
# （EXITED混在・複数実行中でも表示できる。sessionが全くない場合のlist sessionsは
#  zellij list-sessions自体が失敗するためzelperも失敗する。layoutsはzellijを呼ばず無条件）

list sessions | tabs | panes | layouts [--tab TABSPEC] [--json] [-c|--compact]
  # --tabはpanesの絞り込みに使用（他resourceでは無視されない: 実装は全resourceで受理するが
  # 意味を持つのはpanesのみ。helpにその旨明記）
read [PANESPEC...] [--tab TABSPEC] [--name NAME] [--command CMD] [--cwd DIR] [--all]
     [--full] [--tail N] [--json]
send (PANESPEC... | --tab TABSPEC | --name NAME | --command CMD | --cwd DIR | --all)
     ( -- TEXT | --keys KEY... ) [--enter] [--json]
rename pane PANESPEC NAME
rename tab  TABSPEC  NAME
resize pane PANESPEC (grow|shrink) (left|right|up|down) [STEPS]
resize equalize [--tab TABSPEC | PANESPEC...] [--json]
remap LAYOUTNAME [--tab TABSPEC] [--embed-floating] [--dry-run] [--json]
remap (--path PATH | --inline KDL) [同上のoption]
     # v2（DD-10）: 対象はsession全pane（--tab TABSPECで単tabのsourceに絞り込み）。
     # 廃止option: --session-scope / --overflow nest|tabs は意味を失ったため廃止
     # （v0.1.xで削除済み。指定はusage error, exit 2）
add pane [--tab TABSPEC] [--count N] [--name NAME] [--cwd DIR] [-- CMD...]
add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path PATH | --inline KDL] [-- CMD...]
remove pane PANESPEC... [--yes] [--dry-run] [--json]
remove tab  TABSPEC...  [--yes] [--dry-run] [--json]
remove tab TABSPEC... [--empty] [--yes] [--dry-run] [--json]   # --empty時はTABSPEC省略可（指定時はその範囲の空tabのみ）
completion bash|zsh|fish
```

### 1.3 位置指定operandの規則

- `PANESPEC` = pane IDのみ。`terminal_3` / `plugin_1` / bare `3`（=terminal_3）。**名前・path・その他の暗黙解釈はしない**（名前は`--name`、tab名は`--tab`へoption退避）
- `TABSPEC` = tab ID（整数）または一意なtab名（一意ならIDへ解決、曖昧なら候補列出のerror）。全verbのTABSPEC（`--tab` optionと `rename tab` / `remove tab` のpositional）で共通の解決を用いる
- `remap`のpositional = layout名のみ。path/inlineはoption専用（要件2.9の強制規則）
- `send`のTEXTは`--`以降。targets（positional）とTEXTの区切りを構文で保証する

### 1.4 共通target option（全verbで同一意味）

```text
--session NAME   session指定
--tab SPEC       tab指定（ID、または一意な名前）
--name NAME      pane名（title）完全一致
--command CMD    pane_command部分一致
--cwd DIR        pane_cwd一致
--all            対象sessionの全selectable terminal pane
```

選択規則: positional PANESPEC群とfilter option（--name/--command/--cwd/--all/--tab）は併用可（和集合）。単一対象を要求するverb（rename）で複数ヒット→ `ambiguous target` error（候補一覧を出力）。対象0件 → `no target matched` error。

### 1.5 排他規則（violationはusage error, exit 2）

- `remap`: positional layout名 / `--path` / `--inline` の3者は相互排他
- `add tab`: `--layout` / `--path` / `--inline` は相互排他
- `send`: `-- TEXT` と `--keys` は相互排他
- `--tab`（絞り込み）と`--all`は併合（--tab内のall）
- `resize`: `equalize`と`grow|shrink`構文は相互排他
- `--dry-run`と`--yes`の併用は可（--yesは無視される旨を出力）
- `remap`廃止option（`--session-scope` / `--overflow nest|tabs`）: v2では意味を持たないためv0.1.xで削除済み。指定はusage error（exit 2。DD-10.2 #11）

### 1.6 例と非例

```text
zelper read 12                      # terminal_12のviewport読み取り
zelper read --tab agents            # agents tabの全pane
zelper read 1 2 3 --full            # 3 paneのscrollback込み
zelper send 1 2 3 -- y              # 3 paneへyをbroadcast（Enterなし）
zelper send --command codex --keys Enter   # codex起動pane全てへEnter
zelper rename pane 12 worker-1
zelper rename tab 3 agents
zelper resize pane 5 grow right 3
zelper resize equalize --tab 2
zelper remap agents                 # 全paneをagents layoutへ再配置（ceil(M/N)反復・全pane保存）
zelper remap --path ./three.kdl --dry-run
zelper add pane --count 2 --tab 4
zelper remove tab --empty --dry-run
```

非例（すべてerror）:

```text
zelper read pane:12            # selector接頭辞DSLはv1に存在しない
zelper remap ./three.kdl       # pathのpositional渡しは不可（--pathを使う）
zelper send 1 2 y              # -- がないためusage error
zelper read --json --json     # 重複option
```

### 1.7 shell completion

`clap_complete`による動的生成。`zelper completion bash`等がstdoutへスクリプトを出力。verb・noun・一部の値（layout名は実行時解決のため静的リストに含めない）を補完。

## DD-2. Domain / state model

```rust
pub struct SessionRef { pub name: String }            // session名。ID概念はzellijに不存在
pub struct TabId(pub u32);                            // zellij tab ID。再利用されるため不安定キー（参照は「取得直後に消費」）
pub enum PaneKindId { Terminal(u32), Plugin(u32) }    // 表示は "terminal_N"/"plugin_N"
pub struct Geometry { pub x: u32, pub y: u32, pub rows: u32, pub cols: u32 }

pub struct PaneState {
    pub id: PaneKindId,
    pub title: String,
    pub is_selectable: bool,
    pub is_floating: bool,
    pub is_focused: bool,
    pub exited: bool, pub is_held: bool,
    pub geometry: Geometry,
    pub command: Option<String>,                      // pane_command（PTY前景process。表示・--command filter用）
    pub terminal_command: Option<String>,             // terminal_command（=invoked_with序列化。remap照合key。TASK-74）
    pub cwd: Option<String>,                          // pane_cwd
    pub tab_id: TabId, pub tab_position: u32, pub tab_name: String,
    pub plugin_url: Option<String>,
}
pub struct TabState {
    pub id: TabId, pub position: u32, pub name: String, pub active: bool,
    pub selectable_tiled_panes_count: u32, pub selectable_floating_panes_count: u32,
    pub are_floating_panes_visible: bool,
}
pub enum LayoutRef { Name(String), Path(PathBuf), Inline(String) }
pub struct TargetSet { pub panes: Vec<PaneState> }    // 解決済み・決定順序付き
pub struct OperationPlan { /* verbごとに型付き計画。DD-5〜11で定義 */ }
pub struct TargetedResult<T> { pub target: PaneKindId, pub result: Result<T, OpError> }
```

identity規則: pane IDを主キーとする（0.44.0以降リサイクルなし・実機確認済み）。tab IDは API呼び出し間での同一性保証に使わない（再利用観測済み）。tab名は補助識別（曖昧时可error）。

## DD-3. Zellij backend interface

### 3.1 呼び出し規則

- 実行形式: 常に `zellij --session <NAME> action <ACTION> [args]`（`action --session`は構文errorのため使わない。実機確認済み）
- argv配列でのsubprocess実行（shellを経由しない。`--layout-string`のKDLにquote問題を持ち込まない）
- 1呼び出しごとにtimeout（既定10秒、subscribeのみストリーム扱いでv1では未使用。`pipe_plugin`はblockし得るためtimeout必須）
- `zellij --version`を起動時にparse。**最小サポート0.44.3**（DD-10 v2改訂に伴い引き上げ。全実機検証（S1〜S10・E6）が0.44.3、companion pluginのzellij-tile pinと同version、0.44.1/0.44.2の単体検証実績なし。詳細はDD-10.2 #12・compatibility policy（3.5））。未満、または実証済みmajor系列（0.44.x）を超える未来versionは unsupported version error

### 3.2 typed interface（trait。fake実装と差し替え可能）

```rust
pub trait ZellijBackend {
    fn version(&self) -> Result<Version>;
    fn list_sessions(&self) -> Result<Vec<SessionInfo>>;         // list-sessions -n をparse
    fn list_tabs(&self, f: TabsFilter) -> Result<Vec<TabState>>;        // list-tabs -a --json
    fn list_panes(&self, f: PanesFilter) -> Result<Vec<PaneState>>;     // list-panes -a --json
    fn dump_screen(&self, pane: &PaneKindId, full: bool) -> Result<String>;
    fn write_chars(&self, pane: &PaneKindId, text: &str) -> Result<()>;
    fn write_bytes(&self, pane: &PaneKindId, bytes: &[u8]) -> Result<()>;
    fn send_keys(&self, pane: &PaneKindId, keys: &[String]) -> Result<()>;
    fn rename_pane(&self, pane: &PaneKindId, name: &str) -> Result<()>;
    fn rename_tab(&self, tab: TabId, name: &str) -> Result<()>;
    fn new_pane(&self, spec: NewPaneSpec) -> Result<PaneKindId>;         // stdoutのIDをparse
    fn new_tab(&self, spec: NewTabSpec) -> Result<TabId>;                // 同上
    fn close_pane(&self, pane: &PaneKindId) -> Result<()>;
    fn close_tab(&self, tab: TabId) -> Result<()>;
    fn resize(&self, pane: Option<&PaneKindId>, op: ResizeOp) -> Result<()>; // increase|decrease × direction
    fn override_layout(&self, spec: OverrideSpec) -> Result<()>;         // §3.3
    fn go_to_tab(&self, tab: TabId) -> Result<()>;                       // remapの--tab用
    fn dump_layout(&self) -> Result<String>;
    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<()>;    // remapの--embed-floating用
    fn pipe_plugin(&self, path: &std::path::Path, name: &str, payload: &str) -> Result<()>;
    // action pipe --plugin file:<path> --name <name> -- <payload>（DD-10.3）。
    // 権限dialog pending等でblockするためtimeout必須。効果の成否は戻り値に現れない
    // （成功時に即座exit 0）ため、callerがpolling + postcondition検証で判定する
}
// 注（MR-8）: layout_dir列挙はbackend外（app/list.rsが直接読む）。絞り込みはcaller側で行うため
// list_tabs/list_panesにfilter引数は無い。current_tabはcurrent-tab-info --json単体parse
pub struct OverrideSpec {
    pub source: LayoutRef,                    // Nameはbare name（拡張子なし）で解決
    pub apply_only_to_active_tab: bool,       // zelperは常にtrue（他tab保護。実機確認済み）
    pub retain_terminal: bool,                // zelperは原則true
    pub retain_plugin: bool,
    pub cwd: Option<PathBuf>,                 // 必要に応じ--cwd
}
```

### 3.3 KDL取り扱い規則（実機制約の反映）

- zelperはlayout KDLを**読み取り**（slot数カウント・instance分割・再構成commandの注入）と**生成**（改行区切り形式）の両方を行う。parseには`kdl` crateを使用（最小限の走査: 末端pane nodeの列挙とattribute読み取りのみ。zellijのlayout engineを再実装しない）

- `--layout-string`へ渡すKDLは**改行区切り形式**（dump-layout形式）で生成する。単行`;`区切りは0.44.3で全variant parse error（実機確認済み）
- bar維持が必要な場合、layout KDL側に `pane size=1 borderless=true { plugin location="zellij:tab-bar" }`（下部はstatus-bar）を明示する。zelperの生成KDLはlayout宣言をそのまま維持する（plugin leafは保存・slotを消費しない。DD-10.8）が、**barの自動注入は行わない**（layout宣言なきbarの付与はUIの予期しない変化である一方、宣言を維持するだけでは十分と判断。v2で明確化）。`default_tab_template`由来のbar宣言はlayout解決時に正規形TabTemplate列へ反映され、**全生成tabの生成KDLに維持される**（DD-10.6/10.8。v2.2。これも宣言の維持であり自動注入ではない）
- 名前解決はbare nameのみ（`name.kdl`拡張子付きはfile扱いで失敗。実機確認済み）

### 3.4 構造化出力のparse

- `list-panes -a --json` / `list-tabs -a --json` / `current-tab-info --json` をserdeでPaneInfo/TabInfoとして直接decode（field一覧はcapabilities §2）
- `list-sessions`はJSON不在のため `list-sessions -n` のテキストをparse（`NAME [Created X ago]`形式）。EXITED行（`NAME [...] (EXITED - attach to resurrect)`）もcreated/current/exited flag付きで返し、実行中のみへの絞り込みは呼び出し側（resolve_session・list sessions表示）が行う
- `new-pane`/`new-tab`のstdout（`terminal_N` / 整数）をparseしてID取得
- layout一覧: zellijに列挙APIがないため、layout_dir（`ZELLIJ_LAYOUT_DIR`環境変数 > `~/.config/zellij/layouts`）をzelperが直接読む。dir不存在時は空リスト。config.kdlの`layout_dir`読み取りはv1対象外（利用者は`ZELLIJ_LAYOUT_DIR`で一致させる。MR-4で確定）

### 3.5 capability detection

起動時に`zellij --version`を実行し: (a) 実行可能か（zellij unavailable）、(b) 0.44.3以上か（unsupported version）、を判定。feature単位の細分検出はversion判定で代表する。zellijを呼ぶ出力系（`list sessions` を含む）にも同じgateを適用する。`list layouts` はzellijを呼ばないためgate対象外（zellij不在・未対応versionでも動く）。

**compatibility policy**（設計レビューR5）: 実行時要件はzellij >=0.44.3。**実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3（companion plugin build。DD-10.3のpin）のみ**。0.44.4以降・0.44.x系上位patchなど未実証組合せではcompanion plugin protocolの互換性は保証しないが、remapのprobe pipe（DD-10.7 step 3）とpostcondition検証（DD-10.9）が安全網となり、黙示の破壊的失敗は設計上存在しない（失敗は状態変更前に中断するか検証で検知される）。zellij新seriesのサポート時はtile pin更新と再実証をセットで実施する。

## DD-4. Output contracts

### 4.1 human出力

- `list`系: 表形式（列 = 識別に必要な最小限: ID, 名前, 付随metadata）
  - `list sessions`: `NAME / CREATED / CURRENT / PANES`。CREATEDは `list-sessions -n` の相対表記（`1m ago`等）、CURRENTは `(current)` suffixを `*` 表示、PANESはtab毎pane数の `+` 連結（5tab目以降は `...` 省略。floating paneは `2+1f` 形式で併記。他sessionのlist-tabs失敗時は `-`）。session毎に `--session NAME action list-tabs` を発行する
  - `list tabs`: `TAB_ID / POS / ACTIVE / NAME / PANES`（tiled count、floatingがあれば `2+1f`）
  - `list panes`: `PANE_ID / TAB / TITLE / COMMAND / CWD`
  - `list layouts`: `NAME / SIZE / MODIFIED`（bytes・`3d 5h ago` 相対表記。file stat由来）
  - `--compact`（`-c`）: human出力の縮小。panesは `TAB<TAB>短縮cwd`（HOME prefixを `~` 置換・cwd不明時 `-`）のみ、sessions/tabs/layoutsは名前のみ（header無し）。`--json` 併用時はJSONを優先しcompactは無視
- `read`複数pane: pane毎にヘッダ行 `=== terminal_3 (HB1, tab:2 agents) ===` + 内容
- 変更系: 実行した操作の1行サマリ + 失敗があれば `FAILED` 行
- dry-run: 実行予定操作のリスト（`[plan]`接頭辞）+ 対象数

### 4.2 JSON契約（schema_version = 1）

- 全コマンド共通: 成功時はコマンド固有のobject、失敗時は:

```json
{ "schema_version": 1, "ok": false, "error": { "class": "AmbiguousTarget", "message": "...", "candidates": ["terminal_3", "terminal_5"] } }
```

- `error.class` enum: `Usage` / `ZellijUnavailable` / `UnsupportedVersion` / `NoTarget` / `AmbiguousTarget` / `LayoutNotFound` / `LayoutInvalid` / `Preflight` / `OperationFailed` / `PartialFailure` / `VerificationFailed`
- multi-target操作は `results: [{ "target": "terminal_3", "ok": true, ... }, ...]` の配列。**部分失敗を隠さない**
- 成功時envelope: `{ "schema_version": 1, "ok": true, "data": <コマンド固有object> }`。multi-target操作は `data.results[]` に `TargetedResult`（`target`, `ok`, `detail`|`error`）を格納する
- stable fields（v1保証）: `schema_version`, `ok`, `data`, `error.class`, `results[].target`, `results[].ok`。その他（message詳細等）はstable外と明示

### 4.3 exit status

```text
0  成功（部分失敗なし）
2  usage error（clap規約）
3  対象解決失敗（no target / ambiguous）
4  zellij unavailable / unsupported version
5  操作失敗（単対象失敗、または複数対象の全失敗）
6  部分失敗（複数対象の一部失敗）
7  postcondition検証失敗 / preflight失敗
```

error.classとexit statusの対応: `Usage`=2 / `NoTarget`・`AmbiguousTarget`=3 / `ZellijUnavailable`・`UnsupportedVersion`=4 / `OperationFailed`=5 / `PartialFailure`=6 / `Preflight`・`LayoutNotFound`・`LayoutInvalid`・`VerificationFailed`=7

## DD-5. `list` 操作設計

- request: 対象resource種別 + session + filter
- 解決: tabs→`list-tabs -a --json`、panes→`list-panes -a --json`（--tab絞り込みはzelper側でtab解決後にfilter）、layouts→layout_dir列挙、sessions→`list-sessions -n` parse
- precondition: version判定通過
- backend actions: 読み取り1回。ただしsessionsはPANES概要のため `list-sessions -n` 1回 + 実行中session毎に `--session NAME action list-tabs` を発行（DD-4.1。他sessionのlist-tabs失敗は該当行を `-` に落とし一覧は維持）
- postcondition: なし（読み取り専用）
- 失敗: session不存在はzellij側errorをclass=`OperationFailed`で包む
- dry-run: 対象外

## DD-6. `read` 操作設計

- request: PANESPEC群またはfilter option群、`--full`（scrollback）、`--tail N`（行末N行、zelper側加工。`--tail`は取得済み内容——viewport、`--full`指定時はscrollback込み——の末尾N行に適用）
- 解決: filterは`list-panes`→TargetSet。positional PANESPECはlist-panes結果と照合（存在確認 + 読み取り対象確定）
- plan: 対象pane毎に`dump-screen -p`（full指定時`-f`）。順序はTargetSet順（tab position → y → x）
- 並行: 読み取り専用のため順次実行（結果順序の決定性を優先。並列化はP1）
- 失敗: per-pane失敗を`results[]`で報告、exit 6（部分）または5（全失敗）
- JSON: `results[]: { target, title, tab, content | error }`。`--tail`はcontentに適用

## DD-7. `send` 操作設計

- request: targets、text（`-- TEXT`）またはkeys（`--keys`）、`--enter`（text送信後にCR付加）
- 実行: text → `write-chars -p`（複数行textはv1ではwrite-chars単呼び。`paste`は未検証のためv1未使用）。`--enter` → `write -p 13`。keys → `send-keys -p`（key毎に独立argv要素）
- 順序: TargetSet順に順次。部分失敗でも残対象へ継続（broadcast性質）
- postcondition: なし（書き込み結果の検証手段が存在しない。仕様として明記）
- 失敗: per-pane報告

## DD-8. `rename` 操作設計

- request: 対象（pane/tab単一）、新name
- plan: `rename-pane -p` / `rename-tab-by-id <id> <name>`
- postcondition: `list-panes`/`list-tabs`でtitle/name反映を確認（サイレント成功対策）
- 失敗: 検証失敗はclass=`VerificationFailed`（exit 7）

## DD-9. `resize` アルゴリズム

### grow/shrink

- request: PANESPEC、方向、STEPS（既定1）
- 実行: `resize <increase|decrease> <dir> -p`をSTEPS回
- postcondition: 各step後に`list-panes -g`でgeometry取得。変化なしが2回連続 → それ以上のstepを実行せず「これ以上変更不可」と報告（exit 0、warning）。**no-opがあり得る**ため無検証の成功を返さない

### equalize

1. 対象（--tab または PANESPEC群）のtiled pane群のgeometryを`list-panes`で取得（floating paneは対象外）
2. 目標サイズ計算: 対象が同一直線配置（横1列→cols均等、縦1列→rows均等）の場合は算術均等。それ以外（格子状）は行毎・列毎の均等を順に適用するヒューリスティック
3. 収束ループ: 最大20 iteration。各iterationで最大偏差のpaneに`resize`を1 step適用し`list-panes -g`で再取得。偏差が1行/列以下または進捗なし（2連続no-op）で終了
4. 結果報告: 達成geometryをJSON/humanで返す。**完全均等を保証しない**（helpとJSONの両方に明記）
- 反復上限とno-op検出により無限ループは構造的に不存在

## DD-10. `remap` アルゴリズム（v2）

初版: 2026-08-21 / 全面改訂: 2026-09-03（TASK-34/37。E6実験によるcross-tab移動の実証を受け、basic-design 4.6の原本仕様へ復帰）/ 改訂: 2026-09-15（TASK-74。run一致照合のkeyを`terminal_command`（invoked_with序列化）へ変更・`default_tab_template`の生成KDL反映・poll中の空応答retry。改訂の根拠と証拠は`remap-v2-matching-fix-task74.md`）/ 改訂: 2026-09-15 v2.2（TASK-75。multi-tab layout全体再現: 反復単位をlayout全体へ一般化・tab名/tab focus/pane focus再現・`focus-pane-id`追加・正規形TabTemplate列へのlayout解決一元化。根拠と証拠は`remap-v3-multi-tab-task75.md`）。本節が旧DD-10（適用単位=tab、M>N既定error、fill/nest/tabs 3 mode、`--overflow tabs`のkill+command再起動、`--session-scope`のper-tab独立適用）全体を置き換える。設計レビューR1〜R7・R9（design-review.md §4.6）を反映済み。

**実装状態の注記（2026-09-16 E2E改修・完了時追際）**: v2系のうちv2.1（TASK-74）に加え**v2.2（TASK-75・multi-tab全体再現）も実装済み**（L1〜L3自動test 171条件green。E2E acceptance 1巡目で要因A〔pane focus対象の位置ベース化〕・要因B〔空groupのnew-tab --layout-string経路〕を検出し本節条文を改訂・改修済み。TASK-75設計書§7.1。E2E acceptance 80条件全PASS〔S-v3-1〜5・tmp/task75/acceptance/〕・コードレビューCR75-1〜3対応まで完了）。E2E再検証・requirements-traceability.mdのimplemented表記・README・usage文書（repo側）の追従も完了（2026-09-16完了時追際）。

改訂の要旨:

- v1は「cross-tabのpane移動は不可能」（capabilities E1〜E5）を前提に、適用単位をtabとし、M>Nを既定error（明示時のみnest=形状不保証 / tabs=kill+再起動）としていた
- E6（TASK-35、zellij 0.44.3 sandbox実験）でcompanion plugin（zellij-tileの`break_panes_to_*`）+ `action pipe`駆動による**プロセス保存のcross-tab移動**が実証され、上記制約の根拠が消滅した
- v2仕様（要件2.6・ユーザー指定）: **対象は全pane（tab単位ではない）**。M pane + N slot layout → **ceil(M/N) instance反復で全paneを配置**。**全paneプロセス保存・kill/restartなし・M>N error廃止**。空slotは既定shell、layout形状は各tabで保証
- v2.2（TASK-75）: 反復単位を「layout最初のtab」から**layout全体（正規形TabTemplate列・T tab）**へ一般化。M pane + S slot（S=sum(N_t)）→ ceil(M/S) block × T tabを生成し、tab名（anchor例外）・tab focus・pane focus・default_tab_template由来barを`zellij --layout`起動時と同一に再現。T=1はv2.1と後方互換（割当・k・命名・tab focus復帰の式レベル一致。10.6）

### 10.1 前提（実験確定事実・要件・未検証リスク）

E6確定事実（一次記録: capabilities §4.1、`tmp/e6_plugin/`・`tmp/260901_exp_e6_*`）:

1. `break_panes_to_tab_with_id(pane_ids, tab_id, should_change_focus)` / `break_panes_to_new_tab(pane_ids, name, should_change_focus)`（zellij-tile 0.44.3）でプロセス保存移動可（pane id・pid不変をheartbeat連番で実証）。移動元tabが空になると自動close。tab idはclose後に再利用され得る（DD-2規則どおり不安定キー）
2. companion pluginは**bin crate**としてbuildする（cdylibは`_start` export欠落でload失敗）。targetは`wasm32-wasip1`
3. CLI駆動: `zellij --session S action pipe --plugin file:<path>.wasm --name <name> -- <payload>`（payloadは単一文字列）。`ZellijPlugin::pipe`で受信。未稼働なら自動launch（floating pane）。plugin→CLI応答（`cli_pipe_output`）はserver内1s timeoutで不達のため**設計に使わない**。権限が通っていればpipeは即座にexit 0。権限dialogがpendingする状況ではpipe CLI自体がblockする（E6 run2/run4で観測）→ backend呼び出しtimeoutで検知可能
4. 権限: `<XDG_CACHE_HOME>/zellij/permissions.kdl`（Linux ProjectDirs cache）に、plugin location（`file:` schemeを除いた絶対path）をquoted KDL node名として権限（ReadApplicationState / ChangeApplicationState / ReadCliPipes）とともに事前記述すると対話dialogなしでgrant（**session起動前のseedで実証**。0.44.3にこの自動化経路は他に存在しない。grant時zellij自身もこのfileを書き換える）
5. override-layoutは、layout KDLのpane nodeに既存paneと同一の`command`+`args`を書くとrun一致照合（`find_already_running_panes`）で**決定論的にslot配置**される（spawn 0・kill 0を実証）。bare `pane`はshell pane（invoked_with=None）のみ一致。`--layout-string`は改行区切り形式・値quote必須
   - TASK-74精密化（spike S実証・2026-09-15）: 照合keyのdata sourceは`list-panes`の**`terminal_command`**（= paneの`invoked_with()`〔起動時Run〕の序列化）である。**`pane_command`はPTYの現在の前景process**（/proc由來）であってinvoked_withとは無関係（shell paneでは常にshell自身）。bare `pane` / `new-pane` / layout bare slot 起動のpane（既定shell）はinvoked_with=Noneでありbare slotと完全一致する
   - `Run::Command`の等価比較はcwdを含む全field一致である（起動時cwd明示〔layout `cwd=`・`zellij run --cwd`〕のpaneは無cwdslotと照合しない。`zellij run`の--cwdなし起動はinvoked_withのcwd=Noneのため無cwdslotと照合する）。plugin leaf slotは既存plugin pane（Run::Plugin）と照合し宣言位置へ配置される

6. TASK-75実験事実（2026-09-15・podman sandbox・一次記録: `tmp/task75/`）:
   - multi-tab layout起動時、名前ありtabは名前を保持し、名無しtabはzellij自動名「Tab #N」で具体化される。`default_tab_template`は**全tab**に展開され各tab subtree末尾にbar leafとして現れる。`new_tab_template`はdump-layout末尾に残存する
   - tab focus: layoutのfocus=true指定tabがactiveになる。**focus無指定でも最初のtabがactive**。dumpは常にactive tabにfocus=trueを付けるため、dumpからfocus指定有無は区別不能
   - pane focus: layoutのpane focus=trueは起動後に反映される（非先頭pane・非active tabも有効）。`list-panes`の`is_focused`は**全tabで**layout指定どおり各1 paneがfocused（focus無指定時は各tab最初のterminal pane）。dumpのpane focus=trueはactive tabのみで、非active tabのfocus位置はdumpから復元不能（検証はlist-panes経由）
   - pane idはterminal/plugin系列で独立採番（list-panes上でid重複・is_plugin区別必須）。pane idはlayout宣言順に振られない（幾何との対応は実測とおり。id順=layout宣言順の仮定は禁止）
   - `zellij --session S action focus-pane-id <PANE_ID>`が存在する（PANE_ID形式: `terminal_N` / `plugin_N` / bare N）

要件（requirements.md §2.6）: 既存terminal paneの保存（L93: 明示選択のないkill/recreate禁止）、M>N時のlayout instance反復による決定論的配置、決定論的かつ文書化されたpane順序、plugin paneを誤って破壊・混入しないこと。

未検証リスク（**設計が依存してはならないもの**）:

- `break_panes_to_tab_with_index`（position基準）の実挙動 → 不使用（10.2 #8）
- floating paneに対する`break_panes_to_*`の挙動 → floating paneは移動対象外（10.5）
- tab idとpositionが乖離したsessionでの照合 → with_idはlist-tabs直前取得・効果はpane id基準のpollingで検証（10.7）
- 実行中serverが**起動後の**permissions seedを再読込するか → 依存せず、失敗は検証で検知してhint案内（10.4）
- `break_panes_to_new_tab`のname引数 → 不使用。tab名は`rename-tab-by-id`で付与（10.3）

### 10.2 主要設計判断（選択肢比較）

| # | 軸 | 採用 | 却下案と理由 |
|---|---|---|---|
| 1 | wasmの配布 | zelper binary埋込（include_bytes!）+ 実行時にcache dirへextract（10.3） | release assetへの同梱（installが2 file化し配置path管理が利用者負担。単一binary配布の崩壊）/ 実行時build（cargo+wasm targetを利用者に課す）/ wasmのrepo commit（生成物のcommit） |
| 2 | plugin起動方式 | 最初のpipeによる自動launch（floating）に任せる | 明示`launch-plugin`（non-floating配置はE6未検証。呼び出し1手順増で利益なし） |
| 3 | 権限seedのタイミング | remap実行時・pluginが必要なとき毎回（冪等check+追記） | `setup` subcommand（CLI表面増。単一binary配布にsetup工程は存在しない）/ 初回のみ（失敗が潜在化する。毎回checkはfile読み1回で安価） |
| 4 | scope既定 | **session全体の全pane**（selectable・tiled・terminal） | 従来のactive tab既定（ユーザー指定仕様「remapの適用はtab単位ではない。すべてのpane」に反する） |
| 5 | `--session-scope` | 廃止。既定がその意味になるため（v0.1.xで削除済み） | 意味を持たせたまま残す（既定と同義で無意味） |
| 6 | instance 0のtab | 現active tab（anchor）を再利用。tab名・位置を保持 | 常に新規tab（active tabの名前と位置を失い、全groupの移動とrenameが必須になる） |
| 6注記（v2.2・TASK-75） | anchor（生成tab (0,0)）は**名前を保持しrenameしない**。multi-tab再現においてanchorはlayout最初のtab名との一致検証から唯一除外される（同一性検証の例外。ユーザー承認C2・2026-09-15。単tab/inline運用の現行挙動不変のため） | anchorをlayout最初のtab名へrenameする（現行sessionのtab名を黙って失う。D1不採用） |
| 7 | instance j>=1のtab | `break_panes_to_new_tab`で「新規tab作成+移動」を1呼び出しで行う | `new-tab --layout-string`で空tabを作ってからwith_idで移す（2呼び出し。tab idの受け渡しとid再利用raceが増える） |
| 8 | 移動API | with_id（instance 0への集中）+ new_tab（instance j>=1）のみ | with_index（position基準・実挙動未検証・tab id再利用で誤送りになり得るため不使用） |
| 8注記（v2.2・TASK-75・E2E要因B改訂） | 割当paneを持つgroupはbreak-new（移動+tab作成を1呼び出し）のまま。**割当0件のgroup（空group）のみnew-tab --layout-string <生成KDL>（tab作成時点で生成KDL適用）+ id parse**で具体化（空listのbreak-newは未検証のため不使用。10.7 5-b。旧「layout引数なし→override-layoutで正規化」経路は既定paneがbare slotに照合されず残存するため廃止〔U75-10解決〕） | new-tab（layout引数なし）後にclose-paneで既定paneを除去（最終pane closeでtab自体が自動closeされるため不成立。TASK-75設計書§7.1実験4） |
| 9 | 空slot | 常にbare pane（既定shell）に正規化 | layout宣言commandの起動（「空slotは既定shellで埋まる」仕様に反し、配置の操作に起動の意味を混ぜる。BD 4.6.4「apply layout ≠ start commands」） |
| 10 | M<=N単一tab case | 生成KDL適用に統一（as-is適用廃止） | layout as-is適用の維持（code pathが2つになり、空slot意味とcommand pane配置の決定論が不統一） |
| 11 | `--overflow` | 廃止。即時削除し指定はusage error（公開直後のv0.1.xのため。旧`--overflow tabs`は破壊的再構成を期待する指定がno-opで別意味に解釈される危険を避ける。v0.1.xで削除済み） | 意味を変えて残す（旧指定の意図を無言で再解釈する）/ 当面hidden no-opで受理（旧scriptが黙って異なる挙動に載せ替わる移行期間が残る） |
| 12 | zellij version要件 | 最小0.44.3に引き上げ（DD-3.1・compatibility policyはDD-3.5） | 0.44.1維持（実機検証はすべて0.44.3のみ。0.44.1/0.44.2の単体検証実績がなく、pluginはzellij-tile =0.44.3 buildでserverとの組は同versionのみ実証）/ remapのみ0.44.3を要求するfeature gate（二重gateの複雑化に対し実利がpatch版差程度） |
| 13 | floating pane | 移動対象外。従来どおりpreflight error / `--embed-floating`でtiled化して組入れ | break系での移動（floating paneの移動挙動はE6未検証） |

### 10.3 companion plugin（配布・build・起動・protocol）

- **crate**: cargo workspace memberとして `plugin/`（crate名 `zelper-companion-plugin`）。**bin crate**（E6事実2）。`zellij-tile = "=0.44.3"` にexact pin。compatibility policy（DD-3.5）: 実行時要件はzellij >=0.44.3、実証済み組合せはzellij 0.44.3 + tile 0.44.3のみ。0.44.4以降など未実証組合せではplugin protocol互換性は保証しないが、probe（10.7 step 3）+ postcondition検証（10.9）により黙示の破壊なしに失敗する設計とする。zellij新seriesのサポート時はpin更新と再実証をセットで行う
- **build・組込**: profileは`opt-level="z"` + lto。zelper本体のbuild.rsがpluginをbuild・埋込、zelper crateは`include_bytes!`で参照する。仕様（設計レビューR4）:
  - cargo実行ファイルは`CARGO` env変数で解決（未設定なら`cargo`）
  - build command: `<cargo> build -p zelper-companion-plugin --release --locked --target wasm32-wasip1 --target-dir <workspace>/target/plugin-build`。`-p`明示によりworkspace全体の再帰buildを防止し、target-dir分離により親cargoのtarget dir lockと衝突しない。`--locked`でCargo.lockを尊重
  - artifact: `target/plugin-build/wasm32-wasip1/release/zelper-companion-plugin.wasm`
  - build.rsはartifact（後述のescape hatch指定時は指定wasm）を`OUT_DIR/zelper-companion.wasm`へcopyし、zelper crateは`include_bytes!(concat!(env!("OUT_DIR"), "/zelper-companion.wasm"))`で読む（通常build・escape hatchの両経路で同一の参照pathに統一）
  - escape hatch: `ZELPER_PLUGIN_WASM` envにprebuilt wasm pathを指定するとbuild.rsのplugin buildを省略し、そのfileを`OUT_DIR`へcopyする（CI高速化等）
  - rerun指示: `cargo:rerun-if-changed=plugin/`（crate一式）+ `cargo:rerun-if-env-changed=ZELPER_PLUGIN_WASM`
  - `wasm32-wasip1` targetが未installならbuild.rsが明確なerrorを出す
- **runtime配置**: `$XDG_CACHE_HOME/zelper/companion/<zelper-version>/zelper-companion.wasm` へ、存在しなければextract（書込は同dirのtemp file + atomic rename）。version入pathのため別versionとの混在・skewが構造的に発生しない。この絶対pathがpipeの`file:<path>`とpermissions.kdlのnode名に使われる（実行時解決。XDG_CACHE_HOME変更に追従）。配置先override（env等）はv2では提供しない（permissions.kdlのnode名との一致管理が必須なため設定表面を増やさない）
- **起動**: pipe自動launch（floating・E6実証）に任せる。plugin paneはhost tabに属し、host tabのcloseとともに消えるが、以降のpipeが自動launchで復帰させるため設計上の制約にならない
- **plugin側実装規則**:
  - `load()`: `set_selectable(false)`（remapのsource・検証に現れない。E6実証）+ `request_permission([ReadApplicationState, ChangeApplicationState, ReadCliPipes])`（seed漏れ時に対話grantで救済されうる経路を残す）
  - `pipe()`: pipe nameでsubcommandをdispatch。CLIへの応答手段はないため処理のみ行う。未知のnameは何もしない（旧binary混在時の安全側挙動）
  - `render()`: no-op
  - protocol（payloadは空白区切りの位置文字列。pane idは`terminal_N`またはbare int）:

```text
probe     <nonce>                       -> rename_pane_with_id(own pane, "zelper-probe-<nonce>")
break-id  <tab_id> <pane_id_csv>        -> break_panes_to_tab_with_id(panes, tab_id, true)
break-new <pane_id_csv>                 -> break_panes_to_new_tab(panes, None, true)
```

  - `probe`は**状態変更前の応答性・権限確認**（設計レビューR3）: 権限（ChangeApplicationState）を要するがuserのpane/tab構成を変えない操作（plugin自身のnon-selectable paneのtitle書換え。E6で権限付与時に動作確認済みの操作）を使う。zelperはnonce込みのtitle出現をlist-panesでpollingし、pluginが権限付きで応答したことを確認してから状態変更へ進む（10.7 step 3）。nonceは実行毎に一意とし、前回実行のtitle残留による偽陽性を防ぐ
  - `should_change_focus=true`はE6実証値（falseは未検証のため不使用）。tab名はbreak-newのname引数（未検証）ではなく`rename-tab-by-id`で付与する

### 10.4 permissions.kdl seed（自動grant）

- 対象file: `$XDG_CACHE_HOME/zellij/permissions.kdl`（XDG_CACHE_HOME未設定時は`~/.cache/zellij/`）
- タイミング: remap実行でcompanion pluginが必要なとき（10.7の移動が必要なとき）**毎回**。dry-runでは書かない
- 手順（他pluginの権限を壊さない追記）:
  1. fileが存在しない → 当該dirを作成し、companionのnodeのみの新規file
  2. fileが存在する → `kdl` crateでparseし、対象node（node名 == wasm絶対path文字列）ごとに判定:
     - 存在し必要3権限すべてあり → 何もしない
     - 存在し権限不足 → そのnodeに不足権限のみ追記して書戻し（他node・他権限は保持）
     - nodeが存在しない → **file末尾へのtext append**（kdl crateのround-trip整形で既存行が変化するのを避けるため純append）。追記前に既存内容が改行終端でなければ先頭に改行を補い、前行末尾への連結によるKDL破壊を構造的に防ぐ（設計レビューR1）
  3. 既存fileのparse失敗時は書き換えずPreflight error（手動修復を促す。ユーザーの他plugin権限を壊すリスクの排除を優先）
- 書込みの原子性と並行書込防御（設計レビューR2）:
  - 書込みは同dirのtemp file作成 + atomic renameで行う（readerが途中状態を観測しない）
  - 書戻し直前にfileを再読込し、最初の読み取り時から内容が変化していたら（zellij他processの書換）、変化後の内容に対して手順2をやり直す（bounded retry 3回）
  - retry打ち切り後も競合が続く場合は書込まずPreflight errorで中断する（last-writer-winsで他pluginの権限を黙って失わせない）
- 失敗の扱い: 読み書き不可・parse不可・競合打ち切りはすべてPreflight error（pathと対策を表示）
- 既知の非決定性への防御: 実行中serverが起動後のseedを権限判定に反映するかは未検証（E6はsession起動前seedで実証）。設計はこれに依存せず、反映されない場合は移動前に実施するprobe pipe（10.7 step 3）が不成立となり状態変更前に中断される。probe失敗時のerrorには「session開始前にseedされていない場合、権限dialogがpendingしている可能性がある。session再起動後の再実行または手動grant」をhintとして含める
- zellij自身もgrant時にこのfileを書き換える（E6観測）。並行書込は「変更検知 + bounded retry + atomic rename」で防御し（R2）、防御しきれない競合はPreflight errorとして中断する（黙示の権限喪失を許容しない）
- 既知の残リスク（TASK-39コードレビューC2で明記）: 「再読込一致確認 -> atomic rename」の間にzellij自身がfileを書き換える競合は外部から排除できない（zellijの書換を外部からlockする手段が存在しないため）。この場合はzellij側の更新がzelperの書込で上書きされうる（lost-update）が、(a) windowは再読込->renameの最小区間に縮小済み、(b) zelper自身の並行実行はpermissions.kdlと同dirのadvisory lock file（flock）で直列化する、(c) zellijは`request_permission`呼出毎にこのfileを再読込するため、他pluginの権限喪失時は当該pluginが次の権限要求で回復できる——黙示の恒久喪失ではない

### 10.5 scope解決とsource順序

- source pane: selectable かつ tiled な terminal pane（`is_remap_source`。従来通り）。
  ただしexit hold中（`exited` / `is_held`）のpaneは実行runを持たずrun一致照合の対象にならないためsource外とし、元tabに残す（paneはkillされず生存。TASK-39コードレビュー観点2補足で確定）
- scope: 既定 = **session全体（全tab）**。`--tab TABSPEC` 指定時 = そのtabのpaneのみをsourceとし、anchorもそのtabとする。**`--tab`時のlayout適用はlayout最初のtab鋳型による単tab適用**（正規形TabTemplate列の鋳型0のみ・T=1経路。multi-tab layoutを指定しても2番目以降のtabは適用しない。v2.2）
- 順序: `list-panes`の (tab_position, pane_y, pane_x) 昇順（visual order）。文書化されtestで固定（従来通り）
- floating pane: scope内にselectable floating terminal paneがあればpreflight error（既定）。`--embed-floating`で`toggle-pane-embed-or-floating -p`（プロセス保存・実証済み）によりtiled化してsourceに組入れる。tiled化はlayout解決・plan確定**後**の状態変更第1歩とし、dry-runでは実行しない（計画はtoggle前snapshotから仮想的にfloating paneを含めるためdry-runと実行の計画が一致する。PR#1対応の維持）。tiled化の対象はpreflightで特定したselectable floating terminal paneのみ（probe自動launchで現れるcompanion plugin pane（floating・non-selectable）はtoggleしない。10.3）。companion plugin paneはsourceに現れない（10.3）

### 10.6 配置アルゴリズム（plan）（v2.2全面改訂: TASK-75）

- **正規形TabTemplate列（layout解決の出力）**: layoutの各`tab` nodeを文書順に列挙し、各tabの鋳型（subtree・name・focus・n_slots・pane_focus_slot）を構築する:
  - subtree: layoutに`default_tab_template`がある場合、template subtreeの文書順最初の`children` nodeをtab subtreeのnodesで置換したもの（TASK-74のchildren置換規則を**layout解決時点へ一元化**。`children` node不在・置換後terminal leaf数とtab subtree slot数の不一致は`LayoutInvalid`）。template無しlayoutではtab subtreeをそのまま。tab属性（name・focus）はsubtree本体から除去し鋳型fieldへ保持する
  - n_slots: subtreeの末端terminal pane slot数（plugin leaf除外・SKIP_NODES規則・従来通り）
  - pane_focus_slot: subtree内で`focus=true`を持つ文書順最初のterminal pane leafのslot index（無し=None。複数個は文書順最初を採用しwarning——zellijの複数focus=true挙動は未検証のためzelper側規則として確定）
  - tab nodeが1つもないlayout（`--inline 'layout { ... }'`等）は「1つの名無しtab鋳型」（T=1。従来の`base_subtree`相当）
  - `new_tab_template`・`pane_template`/`tab_template`（SKIP_NODES）は対象外のまま（v2.1と同一）
- 反復単位 = **layout全体**。T = 鋳型数・N_t = 鋳型tのslot数・**S = sum(N_t)**。**N_t=0の鋳型（terminal slotを持たないtab）は`LayoutInvalid`**（remapの適用経路は全生成tabにterminal pane経由の具体化を要するため。事前中断）
- M = source pane数。block数 k = **max(1, ceil(M/S))**
- anchor: 既定 = active tab（current-tab-info）。`--tab`時 = 解決済み対象tab
- 割当: source visual orderのpane index i（0開始）→ **block b = floor(i/S)**・block内offset o = i%S → 累積slot数 C_t = N_0+...+N_{t-1}（C_0=0）として o in [C_t, C_t+N_t) となるtab t・**slot s = o - C_t**（visual order逐次充填）
- 生成tab = **全block全tab（k×T）**。(b,t)=(0,0) はanchor（名前保持・renameしない。10.2 #6注記）。それ以外は新規tab。tab名: **幹 = T>=2 かつ鋳型tがname属性を持つなら鋳型名、そうでなければbase**（base = layout名 / --pathのfile stem / `remap`（--inline））、b>=1なら幹に`-<b+1>`接尾。名無し鋳型はzellij自動名（Tab #N）に依存せずzelperが命名する（自動名採番はCLIから制御不能のため再現対象外。10.13）
- 空slot: 最終blockの各tabでのみ発生（割当pane数がN_tに満たないtab・割当0件のgroupを含む）。空slotは常にbare pane（既定shell。10.2 #9維持）。全割当paneは **preserved一色**（kill/restartは存在しない）
- M=0（空session）: k=1、全tab全slot空（既定shellで埋まる）
- plannerは純粋関数（fake backendなしで検証可能。従来通り）
- **T=1後方互換（割当・k・命名・tab focus復帰の式レベル一致。v2.2の後方互換保証）**: T=1では S=N_0=N であるため、割当式は b=floor(i/N)・s=i%N（現行instance j・slot式と一致）、k = max(1, ceil(M/N))、命名は幹=baseにより (0,0)=anchor・b>=1=`<base>-<b+1>`（現行`<base>-<j+1>`と一致）、focus復帰はfocus=true鋳型がblock 0 tab 0=anchorを指すため無指定時と同一のanchor復帰になる——**保証範囲はこれら計画レベル（割当式・k式・命名・tab focus復帰先）の一致のみ**であり、実行操作列全体の一致ではない（pane focus再現〔focus-pane-id〕はT=1でも新たに実行され、fake backend testの期待値にfocus-pane-id呼出が加わる。TASK-75設計書§2.7・TR75-3）。上記の一致を回帰testで固定する
- **決定論性の保証範囲**（要件2.6「pane ordering for slot assignment must be deterministic and documented」の解釈確定。設計レビューR7。v2.2でgroup定義を一般化）:
  - (i) 各paneのgroup（=所属するblock×tab）はvisual orderから決定論的に定まる
  - (ii) slot対応は**run（command+args）に一意なpane**について決定論的に定まる（run一致照合）
  - (iii) **同一runの複数pane間・複数shell pane（run=None）間のslot順は保証しない**——zellijのrun一致照合がrun等価なpaneを互いに区別しないため（実装上の限界であり、要件の決定論性からの明示的な例外。要件側にもrequirements.md §2.6に同一例外を明記済み〔TASK-74設計レビューTR74-3。2026-09-15〕。10.13参照）。これらのpane間でプロセス保存・pane数・所属tabが保証されることに変わりはない

### 10.7 実行sequence

前提: 全preflight（layout解決・plan・floating検出・companion setup）を状態変更前に完了させる順序。dry-runは10.10のみでここに入らない。polling定数: 250ms間隔・10s deadline（list-panes再取得で条件判定）。

**空応答のlenient取得（TASK-74改訂）**: `list-panes`/`list-tabs`は、screen繁忙時（layout適用中等）にscreen応答の1s timeoutで **exit 0・stdout空**の応答を返す（実験cで実証）。この空応答は一時的な未成立であり、**fatalとはしない**。適用対象と扱い:

- 対象: (1) polling closure内の再取得（step 3 probe・step 5-a/5-bの完了確認）(2) step 1 snapshot取得（`panes_now`/`tabs_now`）(3) 10.9検証冒頭の再取得。backend interfaceは`list_panes_lenient`/`list_tabs_lenient`（`Result<Option<..>>`。空・exit 0 → `Ok(None)`）を提供し、callerは`None`を未成立として扱う
- 空応答の取り扱い: poll対象では条件未成立としてpollingを継続（250ms間隔・deadline 10s**不変**。C3規則「deadline超過後の条件成立は成功扱いにしない」も不変）。snapshot・検証冒頭では取得できるまで同じdeadline内で再試行する
- fatal条件: (a) 空応答がdeadline（10s）を超えて継続 (b) 非空出力でのparse失敗 (c) 非zero exit。(a)は当該phaseのpolling timeout errorへ合流（文面にscreen繁忙の1s timeout機構への言及を含める）

1. snapshot: list-tabs / list-panes / dump-layoutを取得（報告・復旧用。DD-12）
2. anchor idをlist-tabsから解決（直前取得・直後消費。DD-2）
3. companion setup — **break系移動が必要な場合のみ**（k>1、またはgroup (0,0)〔anchor group〕にanchor外tabのpaneが存在）: wasm extract + permissions seed（10.3/10.4）→ **probe pipe** `probe <nonce>` を送り、list-panesで`zelper-probe-<nonce>` titleの出現をpolling（250ms間隔・10s deadline）。**probe不成立（timeout）は一切の状態変更前に中断**（OperationFailed + 10.4の権限hint）。起動済みserverがseedを反映していない・権限dialogがpending・plugin protocol非互換（DD-3.5の未実証組合せ）のいずれもここで検知する（設計レビューR3）。break系を使わない（空groupのnew-tabのみ等）場合はprobeもpluginも不使用（このときremapはcompanion pluginなしで完結する。v2.2: 空groupの新規tab生成はnew-tab経路のため）
4. `--embed-floating`時: floating paneをtiled化（最初の状態変更）
5. 移動phase（group = (block b, tab t)。plugin pipeはfire-and-forget。各pipe後にpollingで完了確認）:
   - a. group (0,0)（anchor group）のanchor外paneを `break-id <anchor_id> <ids>` でanchorへ。完了条件 = group (0,0)の全paneのtab_id == anchor。**group (0,0)のinboundを後続groupのoutboundより先に行う**（anchorが一時的に空になって自動closeする事態を構造的に排除する不変条件の一般化。M>=1なら逐次充填によりgroup (0,0)は必ず1 pane以上を持ち、k>=2ならblock 0はS>=1 paneを持つため5-b開始前にanchor必ず非空）
   - b. block b昇順・tab t昇順で (0,0) 以外の各groupへ:
     - 割当pane 1件以上のgroup: `break-new <ids>` で新規tabへ。完了条件 = groupの全paneが同一tabに所属し、かつそのtabのselectable tiled terminal pane集合がgroupと一致（tab id再利用に耐える特定方法。このtab idをtargetとする）
     - 割当pane 0件のgroup（空group・最終blockのtail等）: **`new-tab --layout-string <生成KDL_{b,t}>`でtab作成時点で生成KDLを適用**（E2E要因B改訂・TASK-75設計書§3.4/実験4: 旧経路〔layout引数なし→go-to→override-layout〕は既定pane〔bare shell〕がoverride-layoutのbare slotに照合されず残存しN_t+1 paneで検証(b)がfail。bare slotはtab作成経路でのみ具体化され、override-layoutは既存paneとrun一致しない限り常に新規spawnする〔U75-10解決〕）しstdoutのtab idをparse（DD-3.2。parse不能・非zero exitは`OperationFailed`で中断）。target = parse済みid
     - 直後に`rename-tab-by-id <target> <生成tab名>`（生成tab名は10.6命名規則。(0,0)はanchorのためrename対象外）。**new-tab由来idはこのrenameで「取得直後に消費」する**（DD-2運用。以降の同実行内参照はtargets配列〔下記data flow〕）
   - polling timeout（pipe効果が現れない）はOperationFailed + 10.4の権限hint + 移動済み/未移動groupの報告。**pipeはCLI失敗後もserver側で遅延実行されうる**ため、中断時の報告には「状態が遅れて変化しうる」旨の注意を含める（R3。probeによる事前検知でこの経路に到達する可能性は抑制されているが排除はできない）
6. layout適用phase（v2.2: focus-pane-id追加・E2E要因A/B改訂）: 全group (b,t)をblock昇順・tab昇順の順に `go-to-tab-by-id <target>` → `override-layout --layout-string <生成KDL_{b,t}> --apply-only-to-active-tab --retain-existing-terminal-panes --retain-existing-plugin-panes`（retain 2 flag常時・他tab保護。従来規則の維持。**空groupはnew-tab --layout-stringで作成時点で生成KDL適用済みのためoverride-layoutをskip**〔E2E要因B〕）→ **`focus-pane-id terminal_<id>`**（tab毎focus paneの再現: 対象slot s = 鋳型pane_focus_slot、無ければslot 0。**対象pane idは適用後のlist-panes〔lenient〕で当該tabのterminal paneをvisual order〔geometry y,x昇順〕に並べたs番目から決定する**——mapping pane id基準はbare pane群のrun一致配置が割当順と一致しないため実幾何とずれる〔E2E要因A・TASK-75設計書§2.4〕。空slot〔spawn pane〕も位置から特定可能なため全生成tabで実行。呼出失敗はwarning・検証(d)で検知。「already focused」のexit 2はfocus状態が実際に成立しているため成功扱い）
7. 検証（10.9）→ **最終go-to**（best-effort。v2.2）: layoutにfocus=true鋳型があれば文書順最初のfocus=true鋳型 t* のblock 0 tab (0, t*)へ、無ければanchorへ復帰（T=1では前者も (0,0)=anchor と一致し現行挙動と同一）。**複数のfocus=true鋳型がある場合は文書順最初を採用しwarningを出す**（E2E要因改修・2026-09-16明文化: zellijはdump上常にactive tabへfocus=trueを付けるため複数指定を機械的に判別できず、zelper側規則として文書順最初に確定し採用を利用者へ通知。10.10のwarning伝達経路）

各phaseで状態を再取得し、長寿命のtab id参照を持たない（DD-2）。**DD-2原則の適用範囲（v2.2明確化・TR75-4）**: 「長寿命」とは実行を超えた保持（構造体field・global・file等への残置）の禁止であり、**単一execute実行scope内のtargets配列（`targets[b*T+t]`・現行から同じ運用）での逐次参照は許容**する。tab idはclose後再利用される不安定キーであるため、実行内でも参照のたびに存在確認を行う: **step 6の各group処理直前にlist-tabs（lenient）でtargets[j]の存在を確認し、不在なら再解決する**——割当ありgroupはpane所属基準（10.7 5-bと同一のmembership特定）・空groupはrename済み生成tab名基準（list-tabsのname一致。go-to-tab-name〔20260915_doc_task75_go_to_tab_name_help.txt〕は同名tab曖昧性のため補助にしか使わない）。再解決不能は`OperationFailed`（中断時報告）。自己作成tabは自実行内にcloseされない限りid再利用の競合sourceが存在しないため、この保持・確認で十分である。

**空groupのみの実行（M=0・またはsource全てanchor内で新規tabが空groupのみ。TR75-5）**: companion plugin・probeは不使用（break系pipe 0件）。preflightは現行と同一depth（layout解決・N_t=0検証・10.6正規形化LayoutInvalid・plan・生成KDL計算）を**状態変更前に完了**し、new-tab実行時点に未知の検証は残さない。new-tab経路の実証（tab id parse・既定paneが生成KDL適用でN_tへ正規化されること）はfake/E2E必須条件とする（TASK-75設計書§5）。

移動phase完了時点で各target tabはちょうどそのgroupのpaneのみを持ち（空groupのnew-tabは既定1 paneのみ）、layout適用のrun一致照合に残りpane（未照合の入れ子・重複spawn）が介入しない状態になっている。

### 10.8 生成KDLの規則（v2.2: 正規形subtree化）

- 全生成tab (b,t)（M<=S単一block含む）で共通の統一規則。生成KDLは**tab nodeなしの`layout { subtree }`形式**（per-tab個別適用。tab属性〔name・focus〕は生成KDLに載せずrename-tab-by-id・go-to-tab・focus-pane-idで再現）:
  - base = 当該tab鋳型の**正規形subtree**（default_tab_templateのchildren置換済み・tab属性除去済み。v2.2で置換を10.6 layout解決へ一元化し、generatorは置換済みsubtreeのみを受取る）。改行区切り形式・値quote必須（DD-3.3）。`override-layout`へはargvで直接渡す
  - occupied slot: 当該paneの**`terminal_command`（invoked_with序列化）**を注入する。`command` = terminal_command空白分割のargv[0]、`args` = 残り。**cwdは注入しない**（TASK-74改訂。旧規則「pane_commandがSomeなら注入する」は、pane_command=PTY前景process〔shell paneでは常にshell自身〕とinvoked_with=Noneの不一致でbare slotとの照合を壊すため廃止）
  - `terminal_command` = None（bare・既定shell起動pane）: bare pane slotのまま（run=None一致。pane_commandの値は参照しない）
  - 空slot: 常にbare pane（10.2 #9）。**cwd注入は全廃**（再作成paneが存在しなくなったため。occupied slotはspawnしない・空slotは既定shell）
  - **default_tab_template反映（TASK-74。v2.2で一元化）**: template由来のbar leaf等は正規形subtreeに含まれたまま生成KDLへ出力される（template内plugin leaf〔compact-bar等〕はslotを消費せず維持され、既存plugin paneとRun::Plugin照合して宣言位置へ配置される。10.1 事実5。新規tabにbar paneが既にあれば照合で消費され、無ければ新規spawnされる——いずれもbar 1件）。`children` node不在・置換後leaf数不一致の`LayoutInvalid`判定は10.6正規形化（layout解決時点）で行う
  - それ以外のnode属性（size / split_direction / name等）はlayout宣言をそのまま維持する。plugin leafはslotを消費せず維持（MR-32規則）
- 既知制限（10.13）: terminal_command空白分割は引数のquoteを復元できない。同一commandのpane群どうしのslot割当順はzellijのrun一致順に依存する

### 10.9 検証（postcondition）

layout適用phase完了後にlist-panes / list-tabsを再取得し:

- (a) 全source pane idが生存し、割当group（block×tab）のtabに所属（**pane id基準**。tab id基準の照合はid再利用回避のため補助にしか使わない）
- (b) **各生成tab (b,t) のselectable tiled terminal pane数 == N_t**（**anchor (0,0)を含む全生成tab**が対象。anchor例外は名前・位置のみであり形状・slot数の例外ではない。command照合missによる重複spawn・未照合paneの入れ子残留を検出。「pane数不一致も検証失敗」のPR#1規則を継承）
- (c) 生成tab名が10.6命名規則どおり（**(0,0)=anchorは名前検証から除外**——anchor tab名保持の唯一の例外。10.2 #6注記）。tab一覧にanchorが残存
- (d) **pane focus（v2.2・E2E要因A改訂: 位置ベース）**: 各生成tab (b,t)で、当該tabのterminal paneをvisual order（geometry y,x昇順）に並べた**s番目（s = 鋳型pane_focus_slot、無ければslot 0）のpane**の`is_focused == true`（list-panes経由。非active tabもis_focusedを持つ。10.1 事実6）。10.7 step 6のfocus対象決定と同一の位置基準であり、**全生成tabが対象**（空slot tab除外は撤廃）
- **leftover_tabs報告（v2.2）**: tabs_afterのうちtargetsに含まれないtab（source外paneのみのtab・companion plugin host tab等）を`leftover_tabs`（id/name/position/selectable pane数）として報告する（humanはwarning行・JSONはdata field）。**closeしない**（閉じる場合はremove verbの明示利用。要件2.6）
- **検証の観測経路とdump-layoutの扱い（v2.2・TR75-8）**: 検証はlist-panes/list-tabs経由で行い、**dump-layoutとの完全一致は保証・検証対象外**である。E2E比較における比較対象は「tab毎slot数・幾何・bar存在・検証可能focus（is_focused・active tab）」に限定し、dumpの正規化表現（size="33%"等）・noise（new_tab_template残存・hide_floating_panes・zellij:link pane・active tabへ常に付くfocus=true等）・保証外項目（tab自動名・pane title・pane id）の除外対応表はTASK-75設計書§4.3に定める
- 失敗時: class=`VerificationFailed`（exit 7）+ 期待/実測差分 + 実行済み/失敗/未実行の区分 +（pipe効果が皆無だった場合）10.4の権限hint。`--json`時は成功envelopeを出さず、mapping/missingを`error.data`に載せた**単一の**error envelopeをmainから出す（PR#1規則の維持）
- 成功時のJSON `data`: `mapping[]`（pane → block/tab/slot/TabId。全件preserved）・作成tabのidとposition・`snapshot_len`（従来fieldの継続）・`t`/`s`/`n_slots`（v2.2追加）
- 最終go-to（focus復帰）失敗は操作失敗に加えない（best-effort。従来通り）

### 10.10 dry-run（v2.2拡張）

- 出力: (a) source pane一覧（visual order・現在のtab付き。TASK-74改訂: 各paneの`terminal_command`〔invoked_with〕を含める。`pane_command`〔前景process〕は表示目的で残す）、(b) M / S / k に加え **T（鋳型数）とper-tab slot数列 N_t**、(c) pane → (block, tab, slot, 生成tab名)割当表（全件preserved・empty slot数）、(d) **生成KDL preview**（生成tab (b,t) 毎。humanは`[plan]` block、`--json`は`instances[].kdl`）、(e) 実行予定backend操作列（companion setup・probe pipe・break pipe〔break-id/break-new〕・new-tab（空group）・rename・go-to・override・**focus-pane-id**・最終go-to先を種別表示）
- JSONの`n` fieldには**S（layout全体slot数）**を格納する（T=1では従来値Nと同一。stable fields〔DD-4.2〕外のdata内部構成拡張のため契約変更にならない）。新規に`t`・`s`・`n_slots`（N_t列）・鋳型のname/focusを追加
- **preflight warningの伝達経路（E2E改修・2026-09-16明文化）**: 複数focus=true鋳型/pane検出・quoting等のpreflight warningは、human実行・--json実行・dry-runの全経路で**stderrの`warning:`行**へ出力する（--json実行時もstdoutのenvelope契約を壊さない）。加えて機械可読経路としてdry-run/成功時のJSON dataへ**`warnings`（文字列列）**を格納する
- 状態変更は一切なし: tab切替・pipe・wasm extract・permissions.kdl書換・focus-pane-idを行わない（DD-12）

### 10.11 途中失敗と復旧

- 適用済み手順は保持しrollbackしない。atomicityは主張しない（DD-12）
- 失敗位置の種別と報告:
  - companion setup失敗: 状態変更前（tiled化を除く）に即中断
  - 移動phaseのpolling timeout: 移動済みgroupと未移動groupを区別して報告（paneは全て生存）
  - layout適用phase失敗: paneは全て生存し正しいtabに所属（形状のみ未適用）である旨を報告
- 再実行の冪等性: remapは再実行時に現source構成からplanを作り直すため、中断からの再実行で全体が完成する（dry-runで現在状態の計画を確認してから再実行する案内を出す）。snapshot（dump-layout）からの手動再構成とsession resurrectionは従来どおり保険

### 10.12 worked examples（development-plan.md要求の7種をv2で更新+1追加、v2.2で#9追加）

| # | 状態 | 挙動（v2） |
|---|---|---|
| 1 | 1 pane、3-slot layout | k=1。anchorへ生成KDL適用。1 pane保存 + 2 slot既定shell |
| 2 | 3 pane同一tab、3-slot | k=1・移動なし・**plugin不使用**。全pane保存・command paneは決定論的slot配置 |
| 3 | 4 pane、3-slot | k=2。group 0（3 pane）→ anchor適用。group 1（1 pane）→ break-newで新規tab `<base>-2`。**全pane保存**（旧仕様のerror/nest/tabsを廃止した置換例） |
| 4 | 6 pane、3-slot | k=2。2 instance各3 pane |
| 5 | 7 pane、3-slot | k=3。第3 instanceは1 pane + 2空slot（shell） |
| 6 | 5 paneが3 tabに分散、3-slot | k=2。group 0 = visual順先頭3 pane（tab跨ぎ）をbreak-idでanchorへ集中。空になったsource tabは自動close。group 1 = 残2 paneをbreak-newで新規tab。**旧`--session-scope`（per-tab独立適用）の置換例** |
| 7 | 移動完了後、instance 2のoverride-layout失敗 | 全pane生存・所属tab正しい（形状のみ未適用）。失敗/未実行を報告、rollbackなし。再実行で完成 |
| 8 | companion pluginが権限不足で不動（dialog pending・seed未反映・protocol非互換等） | probe pipe不成立（timeout）→ **状態変更前に中断**。OperationFailed + 権限hint（10.4）。pane無傷（R3） |
| 9 | 12 pane、異構成multi-tab layout（T=3: 9+1+2 slot・S=12・tab focusあり）〔v2.2追加〕 | k=1・生成tab 3。anchor（(0,0)）名前保持、(0,1)="T-single"〔鋳型名〕、(0,2)=base名〔名無し鋳型〕。各tab幾何は鋳型どおり・全tabにbar。最終go-toはfocus=true鋳型のblock 0 tab。15 paneならk=2・6 tabでblock 1は`-2`接尾・最終blockの割当0件tabはnew-tab経路 |

### 10.13 既知制限と防御

- terminal_command空白分割: 引数のquote・埋め込み空白は復元不可 → 該当paneのrun一致が外れ、(b)検証（pane数 == N_t〔tab毎〕。v2.2）が重複spawnとして検出する。preflightでterminal_commandに`"`や`\`等を含むpaneがあればwarning出力（実行は可。恒久対応はzellij側のargv列API待ち）（TASK-74改訂: 対象をpane_commandからterminal_commandへ変更）
- **起動時cwd明示paneのrun一致不可**（TASK-74）: invoked_withのcwd=Some（layout `cwd=`付きcommand slot・`zellij run --cwd`起動）のpaneは無cwdslotと照合せず（10.1 事実5の等価仕様）、複製spawn+grid内挿入となり(b)検証で`VerificationFailed`として検知する。CLI表面にinvoked_withのcwdを区別するdataが存在しないため設計上の受容。error文面に当該原因の可能性を含める。検証状況: 絶対path cwd明示のみspike Sで実証済み。相対cwd・layout継承cwd・`zellij run --cwd`の正規化差異は未検証だが、起動時cwdがRunCommandのcwd fieldへ載る以上同じ照合外れ経路と推定する（TASK-74設計レビューTR74-7）。preflightでcwd=Someであること自体は検出不可（terminal_commandはcommand+argsのみの序列化）
- **hold flag・EditFile pane**（TASK-74）: `hold_on_start`/`hold_on_close`付き起動pane・`zellij edit` pane（Run::EditFileはterminal_commandに現れない）も同様に照合から外れ(b)検証で検知する。`exited`/`is_held`のpaneは従来どおりsource外
- 同一commandの複数pane: それらの間のslot割当順はzellijのrun一致順依存（順序は保証しない——10.6決定論性保証範囲(iii)の要件例外。**requirements.md §2.6にも同一の例外を明記済み**〔TASK-74設計レビューTR74-3〕）。プロセス保存・pane数・所属tabは保証
- shell pane（terminal_command None）: bare slotとのみ一致。複数shell pane間の順序は保証しない（同(iii)の要件例外）
- tab id再利用: 「list-tabs直前取得・直後に消費」（DD-2）+ 新規tab特定はpane所属基準（10.7 5-b）+ index系API不使用、で防御
- 並行client操作: remap実行中の他client操作（pane追加等）は競合して検証失敗になり得る（再実行で解消）。排他機構は提供しない
- swap layout / template: SKIP_NODES規則でslot数から除外（MR-32）。remap後のswap自動切替の挙動は保証しない（従来通り対象外）。`new_tab_template`は対象外のまま（zellijが新規tab生成時に使う予約領域。dumpへの残存は実機挙動で、remapの再現対象外。TASK-75）
- **multi-tab再現の対象外（TASK-75・v2.2）**: (1) pane nodeの`name`属性→pane titleは保証外・観察のみ（実測ではtitleへ反映されるが、remapはtitleを設定せずrun一致照合とも無関係） (2) 名無しtab鋳型のzellij自動名「Tab #N」は再現しない（採番がCLIから制御不能のためzelper側命名に置換。10.6） (3) ~~focus-pane-id対象が空slotのtabはfocus設定・検証から除外~~ **撤廃（E2E要因A改訂）**: focus対象を適用後のlist-panes位置から決定する設計へ変更し、空slot（spawn pane）も位置から特定する（10.7 step 6） (4) anchor tabの位置はlayout最初のtab位置と一致する保証がない（anchor再利用・10.2 #6の帰結。名前と同様に例外扱い。**形状・slot数は例外ではない**——10.9 (b)はanchorを含む全生成tabへ適用） (5) 過剰tab（targets外の残存tab）は放置し報告のみ（10.9）。**要件側にも(1)(2)を含む同一の例外を明記済み**〔TASK-75設計レビューTR75-1。2026-09-16。requirements.md §2.6〕
- permissions.kdlはcompanionのversion別pathのnodeを蓄積し得る（旧versionのnodeは残る。無害・手動削除可）

### 10.14 error経路（v2で残るもの）

M>N既定errorとoverflow指定漏れerrorは廃止。残る経路とclass:

- `Usage`: 3source排他違反等（DD-1.5）
- `LayoutNotFound` / `LayoutInvalid`: layout解決不能・parse失敗・N_t=0（terminal slotを持たないtab鋳型を含む。v2.2）・default_tab_templateのchildren不在/置換後leaf数不一致（10.6正規形化時点）
- `Preflight`: floating pane存在（--embed-floatingなし）/ companion setup失敗（wasm書込不可・permissions.kdl parse/書込不可）/ `--tab`解決不能は`NoTarget`
- `ZellijUnavailable` / `UnsupportedVersion`: DD-3.1（最小0.44.3）
- `OperationFailed`: pipe CLI失敗（blockによるtimeoutを含む）・polling timeout・go-to/override/rename失敗
- `VerificationFailed`: 10.9 (a)(b)(c)の不一致

## DD-11. `add` / `remove` 操作設計

### add

- pane: `new-pane`をcount回（`--cwd`/`--name`/`--tab-id`/`--`commandをmap）。戻りIDをresultsへ
- tab: `new-tab`をcount回。`--layout`は`-l NAME`へ、`--path`は`-l PATH`へ、`--inline`は`--layout-string`へmap
- postcondition: list-panes/list-tabsで存在確認
- 失敗: 作成済み分は残存させ、per-target結果報告（rollbackしない）

### remove

- pane: `close-pane -p`順次。**プロセスはkillされる**ことをhelpに明記
- tab: `close-tab-by-id`。事前に`list-tabs`でID解決・存在確認（存在しないIDがexit 0で無操作のため、postconditionで消失確認し`VerificationFailed`を検出）
- `--empty`: 「空tab」= selectable な pane（tiled+floating両方）が0個のtab。tab-bar等のnon-selectable pluginのみのtabが該当。対象をdry-run形式で列挋してから削除
- 安全: 2対象以上の破壊的削除、または`--empty`は`--yes`または`--dry-run`必須（非対話環境で確定的な挙動）。単一pane/tab削除は確認不要
- 失敗: 順次実行・部分失敗報告（残対象も実行）

## DD-12. Safety model

- preflight: 全mutating verbで (1) version判定 (2) 対象解決 (3) plan生成 の順。remapはこれに (4) companion setup（移動が必要な場合のみ。wasm extract・permissions seed。DD-10.3/10.4）が続き、いずれも失敗は状態変更前に停止
- snapshot: remap / bulk remove は実行前に `list-panes -a --json` と `dump-layout` を取得し、JSON出力時に`_snapshot`として添付可能（障害診断用。dry-run表示にも利用）
- dry-run: v1は `remap` / `remove`（複数対象・`--empty`）が対応。planのみ出力しbackendのmutating呼び出しを一切行わない。remapのdry-runはcompanion setup（wasm extract・permissions.kdl書換）も行わない（DD-10.10）。`resize equalize` / `add` のdry-runはP1（要件3.7はSHOULD。MR-7で確定）
- atomicity: 主張しない。部分失敗時は (a) 実行済み操作の一覧 (b) 未実行の一覧 (c) 推奨復旧操作（例: 再度remap、またはdump-layoutからの再構成）を報告
- remap実行中失敗の扱い: 移動・layout適用のどこで失敗しても、既に適用済みの変更は保持（戻さない）。失敗手順以降を未実行として報告（DD-10.11）。resurrection（session serialize）が保険として機能する旨を文書化
- 廃止optionのtest固定（設計レビューR6。TASK-43で即時削除に方針変更）: `--session-scope` / `--overflow nest|tabs` はv0.1.xで削除済み。指定はusage error（exit 2・clap経由のstderr出力。`--json`併用時もstdoutには何も出ない）であることと、clap helpに出現しないことをtestで固定する

## 実装モジュール構成（basic-design §9を踏襲、DD-10 v2に伴い更新）

```text
plugin/                            # cargo workspace member zelper-companion-plugin（DD-10.3）
                                   # bin crate・zellij-tile =0.44.3・wasm32-wasip1 release buildを
                                   # zelperのbuild.rsがbinaryへ埋込
src/
  main.rs  cli/  app/{list,read,send,rename,resize,remap,add,remove}.rs
  companion.rs                     # wasm extract・permissions.kdl seed（DD-10.3/10.4）
  domain/  zellij/{process,parser,capabilities}.rs
  layout/{resolver,generator}.rs   # generator: KDL生成（改行区切り・slot正規化。DD-10.8）
  output/{human,json}.rs  error.rs
```

Rust library選定: `clap`（derive）+ `clap_complete` / `serde`+`serde_json` / `thiserror`（library境界）+ `anyhow`（binary境界） / `kdl` crate（layout KDLの最小parse: slot数カウント・末端pane列挙。生成は文字列組み立て） / CLI testは`assert_cmd`+`predicates`。subprocessは`std::process::Command`で十分（async不要）。
