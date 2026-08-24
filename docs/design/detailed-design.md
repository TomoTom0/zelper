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

# 対象session解決（全verb共通）
--session NAME > 環境変数 ZELLIJ_SESSION_NAME > 実行中sessionが1つのときそれ > error（候補表示）

list sessions | tabs | panes | layouts [--tab TABSPEC] [--json]
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
remap LAYOUTNAME [--tab TABSPEC | --session-scope] [--overflow nest|tabs] [--embed-floating]
     [--dry-run] [--json]
remap (--path PATH | --inline KDL) [同上のoption]
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
- `remap`: `--tab`と`--session-scope`は相互排他

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
zelper remap agents                 # active tabをagents layoutへ
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
    pub command: Option<String>,                      // pane_command
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
- 1呼び出しごとにtimeout（既定10秒、subscribeのみストリーム扱いでv1では未使用）
- `zellij --version`を起動時にparse。**最小サポート0.44.1**（`--layout-string`導入版）。未満または未来のmajor不一致は unsupported version error

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
- bar維持が必要な場合、生成KDLに `pane size=1 borderless=true { plugin location="zellij:tab-bar" }`（下部はstatus-bar）を明示する。zelperのremapは既定でtab-bar/status-barを含むKDLを生成する（全面化は非破壊でもUIの予期しない変化であるため）
- 名前解決はbare nameのみ（`name.kdl`拡張子付きはfile扱いで失敗。実機確認済み）

### 3.4 構造化出力のparse

- `list-panes -a --json` / `list-tabs -a --json` / `current-tab-info --json` をserdeでPaneInfo/TabInfoとして直接decode（field一覧はcapabilities §2）
- `list-sessions`はJSON不在のため `list-sessions -n` のテキストをparse（`NAME [Created X ago]`形式）
- `new-pane`/`new-tab`のstdout（`terminal_N` / 整数）をparseしてID取得
- layout一覧: zellijに列挙APIがないため、layout_dir（`ZELLIJ_LAYOUT_DIR`環境変数 > `~/.config/zellij/layouts`）をzelperが直接読む。dir不存在時は空リスト。config.kdlの`layout_dir`読み取りはv1対象外（利用者は`ZELLIJ_LAYOUT_DIR`で一致させる。MR-4で確定）

### 3.5 capability detection

起動時に`zellij --version`を実行し: (a) 実行可能か（zellij unavailable）、(b) 0.44.1以上か（unsupported version）、を判定。feature単位の細分検出はv1ではバージョン判定で代表する（0.44.1〜0.44.3間のaction差は実機検証済みの範囲で存在しない）。

## DD-4. Output contracts

### 4.1 human出力

- `list`系: 表形式（列 = 識別に必要な最小限: ID, 名前, 付随metadata）
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
- backend actions: 読み取り1回
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

## DD-10. `remap` アルゴリズム

### 10.1 前提（実験確定事実）

- 適用単位はtab。`override-layout --apply-only-to-active-tab --retain-existing-terminal-panes --retain-existing-plugin-panes`を常時使用（他tab保護・超過pane保護・plugin保護。実機確認済み）
- **M>Nのoverflowはプロセス保存付きでは実現不可**（override-layoutは既存paneを第1tabにしか割り当てない。capabilities §4.1実験E1〜E5）
- **floating paneはremap適用でkillされる**（retain flagの対象外）。preflightで検出し、既定はerror。`--embed-floating`指定時は`toggle-pane-embed-or-floating -p`（プロセス保存を実証済み）でtiled化する。tiled化はlayout解決・plan確定**後**に行い、layout miss/invalidやM>N+overflow未指定のerrorが状態変更後に起こらないようにする（PR#1レビュー対応）
- 適用後のslot↔pane割当順はpane ID昇順ではない。適用後に`list-panes`で実対応を検証する

### 10.2 source-set解決と順序

- scope解決: 既定=active tab。`--tab`指定時は対象tabを`go-to-tab-by-id`でactive化し、実行前に元のactive tabを記録して完了時に復帰する。`--session-scope`は各tabに本アルゴリズムを独立適用する（**cross-tab pane統合は提供しない**——実現不可のため）
- slot数 N の算出: LayoutRefをKDLとしてparse（name→layout_dirのfile、path→file、inline→文字列）し、末端のterminal pane node（bare pane / command pane / edit pane）を数える。plugin leafはslotから除外する（数える・command注入のslot indexing両方で同じ規則。PR#1レビュー対応）
- source paneの順序: `list-panes`の (tab_position, pane_y, pane_x) 昇順（visual order）。文書化されテストで固定される
- 対象pane: selectable かつ tiled なterminal pane。floating paneの扱いは10.1に従う（preflight error / `--embed-floating`でtiled化して組入れ。tiled化はlayout解決・plan確定後、dry-runでは実行しない。計画はtoggle前のsnapshotから仮想的にfloating paneを含めるためdry-runと実行が同じ計画になる）

### 10.3 配置アルゴリズム

**M ≤ N（fill mode・既定）**: layoutをそのまま適用。既存M paneがzellijのmappingでslotに配置され、空のN-M slotは既定shellで埋まる（実機確認済み）。全pane保存。

**M > N**: 既定は**error**（黙示のkill/再作成を禁止する要件2.6に従う。error文に下記2戦略の案内を出す）。明示指定による2戦略:

- `--overflow nest`: 1 instance分のlayoutをretain付きで適用。全pane保存されるが、overflow分はzellijの自動配置で第1tab内に入れ子になる。**layout形状は保証しない**。検証はpane ID生存のみ
- `--overflow tabs`: 要件2.6のoverflow語彙（layout反復）を担う破壊的mode:
  1. instances = ceil(M/N)。pane i（visual order）→ instance floor(i/N) の slot i%N
  2. 事前snapshotで各paneの`pane_command`/`pane_cwd`を記録
  3. overflow pane（i ≥ N）を`close-pane -p`でclose（**kill**。明示flagによる同意済み）
  4. 対象tab（残りN pane）へlayout適用（fill modeと同じ。保存）
  5. instance j ≥ 2 を`new-tab --layout-string <生成KDL>`で作成。生成KDLはlayout 1 instance分のslot構成で、再構成paneのslotに`command`+`args`+`cwd`を、残slotはbare pane（既定shell）を記述。commandの分割はpane_command文字列の空白分割（heuristic。引用は失われる——既知制限として文書化。`/bin/sh`等のshellのみのpaneはbare pane）
  6. 作成tabを`rename-tab-by-id`で `<layout名>-<j+1>` にrename
  7. 検証: list-panesで全instanceのpane数・command一致を確認。旧pane IDから新pane IDの対応を結果で報告

### 10.4 検証とdry-run

- 検証（全mode）: (a) source pane IDの生存（nest/fill。tabs modeでは対象tab内で検証）または再構成paneのcommand一致 + instance tabのtiled pane数（tabs。command不一致・pane数不一致はともに検証失敗。PR#1レビュー対応）、(b) 失敗時はclass=`VerificationFailed` + 適用済み/未適用の一覧。`--json`時は失敗でも成功envelopeを出力せず、mapping/missingを`error.data`に載せてmainから**単一の**error envelopeを出す（PR#1レビュー対応）。geometryの個別報告はv1では省略（`list panes --json`で確認可能。MR-14で確定）
- dry-run: (a) source pane一覧（visual order）、(b) NとM、(c) paneからtab/slotへの割当表（tabs modeはpreserved/recreatedの別を明示）、(d) 実行予定のbackend操作列。状態変更は一切なし（tab切替も行わない）

### 10.5 途中失敗の扱い

- 適用済みinstanceは保持し、rollbackしない（実行済み/失敗/未実行を報告）。atomicityは主張しない（DD-12）
- 復旧: dry-runの再実行、または事前snapshot（`dump-layout`）からの手動再構成を案内。session resurrectionが保険として機能

### 10.6 worked examples（development-plan.mdが要求する7種）

| # | 状態 | 挙動 |
|---|---|---|
| 1 | 1 pane、3-slot layout | fill mode。既存1 pane保存、空2 slotは既定shell。計3 pane |
| 2 | 3 pane、3-slot | 全pane保存・ID不変、geometryのみ変化（実験実証済みのパターン） |
| 3 | 4 pane、3-slot | 既定=error（案内表示）。`--overflow nest`: 4 pane保存・形状不保証。`--overflow tabs`: pane0-2保存、pane3をcloseしtab 2 slot 1にcommand再起動、tab 2のslot 2-3は既定shell |
| 4 | 6 pane、3-slot | `--overflow tabs`: 2 instances。pane0-2保存、pane3-5をtab 2で再構成 |
| 5 | 7 pane、3-slot | `--overflow tabs`: 3 instances。pane0-2保存、pane3-5をtab 2、pane6をtab 3 slot 1に再構成（slot 2-3はshell） |
| 6 | 複数source tab | `--session-scope`: 各tabに独立適用。tab間のpane移動は発生しない（不可能なため提供しない） |
| 7 | tabs modeでinstance 2作成失敗 | instance 1適用済み保持。error報告: 実行済み（instance 1）/ 失敗（instance 2）/ 未実行なし。dry-run再実行とsnapshot復旧を案内。rollbackなし |

### 10.7 生成KDLの規則

- 改行区切り形式・bar plugin明示（DD-3.3）
- `new-tab --layout-string`へはargvで直接渡す（shell経由しない）
- `cwd`は該当slotのpane属性として絶対pathで指定する（実装・S9実機検証に一致。tab属性は使わず、pane毎の相対合成も使わない。shellのみpaneの再作成でもcwdは注入する — PR#1レビュー対応）

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

- preflight: 全mutating verbで (1) version判定 (2) 対象解決 (3) plan生成 の順。失敗は状態変更前に停止
- snapshot: remap / bulk remove は実行前に `list-panes -a --json` と `dump-layout` を取得し、JSON出力時に`_snapshot`として添付可能（障害診断用。dry-run表示にも利用）
- dry-run: v1は `remap` / `remove`（複数対象・`--empty`）が対応。planのみ出力しbackendのmutating呼び出しを一切行わない。`resize equalize` / `add` のdry-runはP1（要件3.7はSHOULD。MR-7で確定）
- atomicity: 主張しない。部分失敗時は (a) 実行済み操作の一覧 (b) 未実行の一覧 (c) 推奨復旧操作（例: 再度remap、またはdump-layoutからの再構成）を報告
- remap実行中失敗の扱い: tab単位の適用のどこで失敗しても、既に適用済みtabは保持（戻さない）。失敗tab以降を未実行として報告。resurrection（session serialize）が保険として機能する旨を文書化

## 実装モジュール構成（basic-design §9を踏襲）

```text
src/
  main.rs  cli/  app/{list,read,send,rename,resize,remap,add,remove}.rs
  domain/  zellij/{process,parser,capabilities}.rs
  layout/{resolver,generator}.rs   # generator: KDL生成（改行区切り・bar明示）
  output/{human,json}.rs  error.rs
```

Rust library選定: `clap`（derive）+ `clap_complete` / `serde`+`serde_json` / `thiserror`（library境界）+ `anyhow`（binary境界） / `kdl` crate（layout KDLの最小parse: slot数カウント・末端pane列挙。生成は文字列組み立て） / CLI testは`assert_cmd`+`predicates`。subprocessは`std::process::Command`で十分（async不要）。
