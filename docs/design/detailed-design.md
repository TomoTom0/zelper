# zelper Detailed Design (Phase 3)

作成日: 2026-08-21
前提: requirements.md / basic-design.md / docs/research/zellij-capabilities.md（0.44.3実機検証）/ docs/design/requirements-traceability.md
本章番号DD-1〜DD-12はrequirements-traceability.md §3で規定したもの

## DD-1. CLI grammar

### 1.1 凍結されたtop-level verb set（v2・12 verb）

```text
list  read  send  rename  resize  remap  add  remove  move  attach  detach  watch
```

（`watch`はTASK-86・`attach`/`detach`はTASK-89・`move`はTASK-88・いずれも2026-09-25 user指示により追加〔TASK-87設計〕。操作設計はそれぞれDD-15・DD-14・DD-13）

例外的にtop-levelに許容する非verbコマンド: `completion`（shell補完生成。`zelper completion bash|zsh|fish`）。helpの発見性目的で、文書化された例外として扱う。`docs`（agent向け配布物の出力。`zelper docs readme | llm usage|skill|snippet`）も同様の文書化された例外（配布物の取り出し経路。zellij sessionに触れない純粋出力）。

verb setの拡張方針（v2・TASK-87改訂・2026-09-25 user指示。TASK-88適用）: verb setは基準付きの凍結として運用する。拡張の第一経路は既存verbのnoun/resource拡張（例: `add session`・`remove session`）であり、verb数を増やさず語彙を育てる。新規top-level verbの追加は、以下の基準をすべて満たす場合のみ検討する:

1. agent/user primitiveとして独立した価値がある
2. 既存grammar（verb×noun構造・共通target optionとPANESPEC等の共有target grammar）に自然に載らない
3. 対話操作の一対一の複製にとどまらない（session解決・共有契約・非対話経路の提供など、zelperとしての付加価値を伴う）
4. 共有契約（JSON envelope・exit status・error class）を継承する

除外の位置づけ: 従来の「明示的除外」（TASK-72・2026-09-19）は恒久除外ではなく凍結時点の見送り分類である。user指示・要件変更により再評価でき、基準1〜4への適合確認と本節のverb表・§1.2構文木の更新を経て追加する。TASK-87（2026-09-25 user指示）により `move`（単一pane/tabの移動・交換。remap=全体再配置と対をなすlightweight操作）・`attach` / `detach`（session lifecycle・human handoff。作成は`add session`へ分離）・`watch`（pane出力のpattern監視・要件P1）を追加する。scroll/theme・focus系等の残る見送り項目は継続（TASK-72分類を参照）。

remapとの役割分担（DD-1.1とDD-10の双方へ）: 「remapはlayoutを基準とした全pane再配置（companion plugin・KDL生成・検証を伴うheavyweight操作）・moveは単一対象の即時・可逆な相対位置変更（lightweight操作）」。

### 1.2 構文木（全体）

```text
zelper [--session NAME] [--version] <verb> ...

# 対象session解決（session解決を必要とするverb共通: list tabs/panes・read/send等の操作系）
--session NAME > 環境変数 ZELLIJ_SESSION_NAME > 実行中sessionが1つのときそれ > error（候補表示）
# 「実行中」はlist-sessions -nのEXITED行（resurrection待ちdead session）を除外して数える。
# --session NAME明示時は事前存在確認を先行（TASK-93）: list_sessions（EXITED含む一覧）に
# 該当nameがあればpass-through（EXITEDも存在扱い。attach --session EXITED名はresurrection
# の正規経路のため）。不在はNoTarget exit 3 + candidates（running名のみ。runningなしなら
# 作成案内文面。DD-5/14）。環境変数 ZELLIJ_SESSION_NAME 経路は検証せずpass-through
# （list-sessions呼出なし）。自動解決で実行中0件（完全0件・EXITEDのみとも）もNoTarget
# exit 3（作成案内文面）
# list sessions / list layouts は一覧表示自体が目的のためsession解決を行わない
# （EXITED混在・複数実行中でも表示できる。sessionが全くない場合のlist sessionsはbackendの
#  0件decode（DD-3.1）により空一覧の正常系——headerのみ+stderr note・JSONはsessions:[]・
#  compactは無出力、いずれもexit 0。layoutsはzellijを呼ばず無条件）

list sessions | tabs | panes | layouts [--tab TABSPEC] [--json] [-c|--compact]
  # --tabはpanesの絞り込みに使用（他resourceでは無視されない: 実装は全resourceで受理するが
  # 意味を持つのはpanesのみ。helpにその旨明記）
read [PANESPEC...] [--tab TABSPEC] [--name NAME] [--command CMD] [--cwd DIR] [--all]
     [--full] [--tail N] [--json]
send (PANESPEC... | --tab TABSPEC | --name NAME | --command CMD | --cwd DIR | --all)
     ( -- TEXT | --keys KEY... ) [--enter] [--json]
rename pane PANESPEC NAME
rename tab  TABSPEC  NAME
rename session NAME NEW_NAME                                     # TASK-95。旧NAME存在確認はEXITED込み。NEW_NAMEの空文字・'/'・空白含み・先頭`-`・同一名はusage error
     # （不在はNoTarget+candidates=全行名）。NEW_NAME衝突（EXITED込み）・EXITED旧名は
     # Preflight・同一名・空文字/'/'含み新名はUsageで事前拒否（zellijがrc=0で黙って
     # 失敗する系列のため。詳細はDD-8 rename session）
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
remove session NAME... [--yes] [--dry-run] [--json]            # TASK-84。selector 4者排他:
remove session (--all | --dead | --me) [--yes] [--dry-run] [--json]  # ちょうど1つ。単一の完全削除
     # 意味論（DD-11 remove session）: 対象sessionを行（list-sessions表示）も
     # resurrection data（session_info）も含めて完全に消す（kill/deleteの意味論splitは
     # しない。zellij 0.44.3実測でkill-sessionもdataを消すため。backendは常に
     # `delete-session NAME -f`）。EXITED行は削除対象として受理（存在確認はEXITED込み）。
     # 常時gate（単一NAMEでも--yes/--dry-run必須）。--meはZELLIJ_SESSION_NAME解決の
     # 自sessionのみを対象とし、NAME/--all経路では自sessionを拒否/除外して保護する
move pane PANESPEC (left|right|up|down|rotate) [--backwards] [--json]   # 以下3形式は相互排外
move pane PANESPEC --to-tab TABSPEC [--json]
move pane PANESPEC --swap-with PANESPEC [--json]
move tab TABSPEC --swap-with TABSPEC [--json]
attach [--json]          # session解決は--session共通（DD-1.2冒頭）
detach [--json]
add session NAME [--json]                                       # TASK-96。NAMEの空文字・'/'・空白含み・先頭'-'はusage error
     # （app層Usage検査・一覧取得より前。空白含みはzellij受理後もlist-sessions行から名前が
     #  復元できないため、先頭'-'は--通過系列が下流option解釈に到達するため。空文字・'/'は
     #  zellij作成系が明示拒否だが対称性・fail-fastでwrap側でも事前検査。詳細はDD-14 add session）
watch [PANESPEC...] [filter群] [--regex] [--full] [--interval SECS] [--max N] [--json] -- PATTERN
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
- `resize equalize`: PANESPEC群と`--tab`は相互排他（clap pairwise conflicts + 既存app層の「どちらか必須」検査——`remove session`と対称の分担。TASK-100）
- `--dry-run`と`--yes`の併用は可（--yesは無視される旨を出力）
- `remove session`: `NAME...`（1件以上）/ `--all` / `--dead` / `--me` は4者相互排他かつちょうど1つ必須（clap pairwise conflicts + app層のexactly-one検証。TASK-84・DD-11 remove session）
- `move pane`: action positional（direction/rotate）/ `--to-tab` / `--swap-with` は三者相互排他かつちょうど1つ必須
- `move pane`: `--backwards`はaction=rotateとのみ併用可
- `watch`: `--`以降はPATTERNのみ・数は1（v1）
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
zelper rename session old new             # session名変更（事前衝突検査+事後一覧検証つき）
zelper resize pane 5 grow right 3
zelper resize equalize --tab 2
zelper remap agents                 # 全paneをagents layoutへ再配置（ceil(M/N)反復・全pane保存）
zelper remap --path ./three.kdl --dry-run
zelper add pane --count 2 --tab 4
zelper remove tab --empty --dry-run
zelper remove session work --dry-run       # session削除は常に--yes/--dry-run必須
zelper remove session --dead --yes         # EXITED行（dead session）のみ掃除
zelper move pane 5 left                    # 1つ左のslotへ移動（端ではno-op warning）
zelper move pane 5 --to-tab 3              # tab 3へ移動（companion・pane id不変）
zelper move pane 5 --swap-with terminal_9  # pane交換（cross-tabのみ）
zelper move tab 2 --swap-with 5            # tabのposition交換
zelper watch --command codex -- READY      # codex paneのいずれかにREADY出現まで待つ
zelper add session work                    # detached session作成（非対話）
zelper --session work attach               # 対話attach（TTY必須）
```

非例（すべてerror）:

```text
zelper read pane:12            # selector接頭辞DSLはv1に存在しない
zelper remap ./three.kdl       # pathのpositional渡しは不可（--pathを使う）
zelper send 1 2 y              # -- がないためusage error
zelper read --json --json     # 重複option
zelper move pane 5 left --to-tab 3   # directionと--to-tabは三者排他違反
```

### 1.7 shell completion

`clap_complete`による動的生成。`zelper completion bash`等がstdoutへスクリプトを出力。verb・noun・一部の値（layout名は実行時解決のため静的リストに含めない）を補完。

## DD-2. Domain / state model

```rust
pub struct SessionRef { pub name: String, pub created: Option<String>, pub current: bool, pub exited: bool }  // list-sessions -n行のparse結果（DD-3.4）。ID概念はzellijに不存在。nameは行の生成節より前の全体でzellijのtrue nameと完全一致（空白含み名・名前末尾の空白も復元。TASK-98。改行含み名は行分割で破壊されるため対象外）
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
- **成功判定は「終了code 0 かつ stderr空」**（TASK-90・2026-09-26追記）: rc=0でもstderr非空の応答は`OperationFailed`（exit 5）へ変換し、messageへstderr本文を埋め込む（stdoutは破棄）。実測確定: zellijのaction系は**対象session不在+他session生存**時にrc=0 + stdout（稼働session一覧）+ stderr（`Session 'X' not found...` + 候補）で応答する（52action測定。出典: `tmp/tasks/tm-90/`）ため、rcのみの判定は書き込み系の誤成功・read系の不正確なparse errorになる。正当系27測定（list/read/dump-screen/書き込み系12種・plugin稼働構成含む。同出典）でrc=0+stderr非空は皆無のため、この判定は正当系を偽陽性化しない。**stderr内容によるclass切替（Session not found→NoTarget等）は行わない**——非zero終了と同一classでの失敗判定の追加。`run_action`/`run_action_as`経由の全verbに適用。`run_interactive`（attach）は本判定の対象外。**例外: `list_sessions` の0件decode（TASK-93・2026-11-12追記）**——zellij 0.44.3はsession 0個の`list-sessions -n`をrc=1 + stderr完全一致 `No active zellij sessions found.`（末尾ピリオド込み・trim後比較）+ stdout空で応答する（実測確定。出典: `tmp/tasks/tm-92/`）ため、この一致のみを一般成否判定より前に`Ok(Vec::new())`へdecodeする（`list_sessions`内のみ。汎用`run()`の成否判定自体は不変）。条件を満たさない失敗（文面不一致・marker行への追加行混入・stdout非空・rc≠1）は従来どおり`OperationFailed`（fail-safe: 不明な失敗を空一覧へ黙って変換しない）。error間のclass切替を行わない規則は維持され、decode不成立時は`OperationFailed`一本
- **例外: 対話client（attach。DD-14・TASK-89）は`run_interactive()`で実行**。stdioはinherit×3（stdin/stdoutはzelperのfdをそのまま継承。stderrは起動前に親自身のfd 2を一時的なcapture fileへ差し替えた上で継承し、親子で同一fd targetを保つ——captureしたstderrは子終了後に本来のstderrへ転写され、非zero終了時のerror messageへ埋め込まれる。`backend-process.nonzero-exit-operation-failed`と同一規則で、不在session時のzellij側error〔Session not found + 稼働session候補〕をJSON経路にも届けるため）。**stderr契約（明示）**: attach実行中の子のfd 2はcapture fileであり、元stderrのTTY属性・即時可視性は失われる（fd 0/1は元targetを継承）。stderrは子終了後に一括転写され、対話中の即時転送はv1で行わない。Ctrl-C（SIGINT）は実行中のみ親側を一時的にSIG_IGNへ変更する（同一foreground groupの子zellijがSIGINTを受けて処理・終了するのを親のwaitが検出してから転写・報告するため。SIG_IGNはexecを通じて子に継承されるためinstallはspawn後・子終了検出後に旧dispositionへ復元）。capture fileは排他作成（O_CREAT|O_EXCL・0600）直後にunlinkしてFDのみ保持し、退避した元stderrのFDはclose-on-exec付きで子へ継承されない。timeoutは無効（対話sessionは時間無制限が本質で、run()のtimeoutで打ち切られるべきでない）。**既存run()は無変更**（watchのdump-screen反復もrun()のtimeout内で完結するため流用。TASK-90の成功判定追記〔上記〕によりrun()の成功判定は強化されたが、attach・watchのrun()利用構造は不変のまま）
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
    fn rename_session(&self, from: &str, to: &str) -> Result<()>;     // `--session <from> action rename-session <to>`（DD-8 rename session・TASK-95。黙失敗系の事前検査とpostcondition検証はcallerの責務）
    fn new_pane(&self, spec: NewPaneSpec) -> Result<PaneKindId>;         // stdoutのIDをparse
    fn new_tab(&self, spec: NewTabSpec) -> Result<TabId>;                // 同上
    fn close_pane(&self, pane: &PaneKindId) -> Result<()>;
    fn close_tab(&self, tab: TabId) -> Result<()>;
    fn resize(&self, pane: Option<&PaneKindId>, op: ResizeOp) -> Result<()>; // increase|decrease × direction
    fn move_pane(&self, pane: &PaneKindId, op: MovePaneOp) -> Result<()>;    // move-pane <dir> -p / move-pane -p / move-pane-backwards -p（DD-13）
    fn move_tab(&self, tab: TabId, direction: TabMoveDirection) -> Result<()>; // move-tab <left|right> -t <tab_id>（DD-13）
    fn override_layout(&self, spec: OverrideSpec) -> Result<()>;         // §3.3
    fn go_to_tab(&self, tab: TabId) -> Result<()>;                       // remapの--tab用
    fn dump_layout(&self) -> Result<String>;
    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<()>;    // remapの--embed-floating用
    fn attach_session(&self) -> Result<()>;                            // 対話attach（DD-14）。`attach <session>` positional・run_interactive（stdio inherit×3・timeout無効）
    fn detach_session(&self) -> Result<()>;                            // 対象sessionのattach clientをdetach（DD-14）。`--session NAME action detach`
    fn create_session_detached(&self, name: &str) -> Result<()>;        // detached session作成（DD-14）。`attach -b NAME`（positional。global --session形式は禁止）
    fn delete_session(&self, name: &str) -> Result<()>;                 // delete-session <name> -f（top-level positional。DD-11 remove session・TASK-84）
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
    pub cwd: Option<PathBuf>,                 // 不使用（TASK-81明確化: zellij 0.44.3のoverride-layoutにcwd引数が存在しないため〔route.rs Action::OverrideLayoutはcwd=None固定〕。cwdは生成KDL run nodeへ注入して一元化〔DD-10.8〕）
}
```

### 3.3 KDL取り扱い規則（実機制約の反映）

- zelperはlayout KDLを**読み取り**（slot数カウント・instance分割・再構成commandの注入）と**生成**（改行区切り形式）の両方を行う。parseには`kdl` crateを使用（最小限の走査: 末端pane nodeの列挙とattribute読み取りのみ。zellijのlayout engineを再実装しない）

- `--layout-string`へ渡すKDLは**改行区切り形式**（dump-layout形式）で生成する。単行`;`区切りは0.44.3で全variant parse error（実機確認済み）
- bar維持が必要な場合、layout KDL側に `pane size=1 borderless=true { plugin location="zellij:tab-bar" }`（下部はstatus-bar）を明示する。zelperの生成KDLはlayout宣言をそのまま維持する（plugin leafは保存・slotを消費しない。DD-10.8）が、**barの自動注入は行わない**（layout宣言なきbarの付与はUIの予期しない変化である一方、宣言を維持するだけでは十分と判断。v2で明確化）。`default_tab_template`由来のbar宣言はlayout解決時に正規形TabTemplate列へ反映され、**全生成tabの生成KDLに維持される**（DD-10.6/10.8。v2.2。これも宣言の維持であり自動注入ではない）
- 名前解決はbare nameのみ（`name.kdl`拡張子付きはfile扱いで失敗。実機確認済み）

### 3.4 構造化出力のparse

- `list-panes -a --json` / `list-tabs -a --json` / `current-tab-info --json` をserdeでPaneInfo/TabInfoとして直接decode（field一覧はcapabilities §2）
- `list-sessions`はJSON不在のため `list-sessions -n` のテキストをparseする。行構造はzellij 0.44.3 `print_sessions`で固定（source突合・実測確定）: `[Created <duration> ago]`節は無条件で常に存在しname直後に置かれる。suffixは`(current)`/`(EXITED - attach to resurrect)`の排他一択（両立なし）でCreated節の後の行末に置かれ、suffixなし行は行末に空白が残る。抽出は生成節へのanchor化（TASK-99・2026-11-14。設計書: `tmp/tasks/tm-99/design_parse-field-pollution.md`）: `t = line.trim_end()`（行末空白対応）として exited = `t.ends_with("(EXITED - attach to resurrect)")`・current = `t.ends_with("(current)")`・created = `t.rfind("[Created ")`以降の最初の`]`まで（`strip_prefix("Created ")`。`[Created `不在ならNone）・name = `rfind("[Created ")`位置より前の全体（TASK-98・2026-11-14。設計書: `tmp/tasks/tm-98/design_name-truncation.md`）——print_sessionsのnameと生成節の区切りはASCII空白1文字固定のため、marker直前の1文字が`' '`ならその1文字のみ剥がす（名前自身の末尾空白〔space/tab/NBSP〕と名前内部の複数空白は保存。区切り不在の非正規行は剥がさない）。remainderが空文字なら当該行をskip（実機生成不能）。marker不在時は`split_whitespace`先頭wordへfallback（0.44.3正規出力には不存在の防御parse。既存挙動との互換であり安全化でない——marker文言drift時は空白含み名の切り詰め・名前内旧markerによる誤切り詰めへ劣化するfail-mode。DD-3.5再実証対象）。名前が行区切り（改行）を含まない限り名前内容は行末に来ないため、ends_with/rfind判定は名前の部分文字列（`EXITED`・`(current)`・`[`等を含む名前）から汚染されず、名前が`[Created ...]`字面を含む場合もrfind（最後尾出現）が真の生成節を選ぶためname復元も汚染されない（空白含み名はtrue nameとして完全復元）。残余制限: 改行含み名は行分割が破壊するため世界model外（対象外。実機作成可否未測定）。EXITED行もcreated/current/exited flag付きで返し、実行中のみへの絞り込みは呼び出し側（resolve_session・list sessions表示）が行う
- `new-pane`/`new-tab`のstdout（`terminal_N` / 整数）をparseしてID取得
- layout一覧: zellijに列挙APIがないため、layout_dir（`ZELLIJ_LAYOUT_DIR`環境変数 > `~/.config/zellij/layouts`）をzelperが直接読む。dir不存在時は空リスト。config.kdlの`layout_dir`読み取りはv1対象外（利用者は`ZELLIJ_LAYOUT_DIR`で一致させる。MR-4で確定。add tab下流はzellerが`--layout-dir`〔envがOkかつ非空のとき〕を付与して同一探索先へ揃える——TASK-102。保証はzellijが名前探索として扱う入力〔拡張子なし値〕の探索dir統一までで、zellij 0.44.3は拡張子を持つ入力をpath解釈しlayout-dir探索を通さないためドット含みstem等の残余制限が残る）。SIZE/MODIFIEDのstatはsymlinkをたどったlink先の実体statを用い、参照先のstat取得失敗時は名前を一覧に残して値なし（DD-4.1。TASK-82）。layout_dir自体のread_dir失敗（不存在・壊れたsymlink・循環参照等）は空一覧のまま（既存挙動）

### 3.5 capability detection

起動時に`zellij --version`を実行し: (a) 実行可能か（zellij unavailable）、(b) 0.44.3以上か（unsupported version）、を判定。feature単位の細分検出はversion判定で代表する。zellijを呼ぶ出力系（`list sessions` を含む）にも同じgateを適用する。`list layouts` はzellijを呼ばないためgate対象外（zellij不在・未対応versionでも動く）。

**compatibility policy**（設計レビューR5）: 実行時要件はzellij >=0.44.3。**実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3（companion plugin build。DD-10.3のpin）のみ**。0.44.4以降・0.44.x系上位patchなど未実証組合せではcompanion plugin protocolの互換性は保証しないが、remapのprobe pipe（DD-10.7 step 3）とpostcondition検証（DD-10.9）が安全網となり、黙示の破壊的失敗は設計上存在しない（失敗は状態変更前に中断するか検証で検知される）。zellij新seriesのサポート時はtile pin更新と再実証をセットで実施する。0.44.x系上位patchは実行時に拒否しないため、`list-sessions`parse（DD-3.4）の生成metadata抽出・name復元が`[Created `marker文言の現行維持に依存することも再実証対象に含む（marker文言drift時はnameが行全体化・marker不在fallbackの先頭word化へ劣化。TASK-98設計§3.2/§9）。

## DD-4. Output contracts

### 4.1 human出力

- `list`系: 表形式（列 = 識別に必要な最小限: ID, 名前, 付随metadata）
  - `list sessions`: `NAME / CREATED / CURRENT / PANES`。CREATEDは `list-sessions -n` の相対表記（`1m ago`等）、CURRENTは `(current)` suffixを `*` 表示、PANESはtab毎pane数の `+` 連結（5tab目以降は `...` 省略。floating paneは `2+1f` 形式で併記。他sessionのlist-tabs失敗時は `-`）。session毎に `--session NAME action list-tabs` を発行する
  - `list tabs`: `TAB_ID / POS / ACTIVE / NAME / PANES`（tiled count、floatingがあれば `2+1f`）
  - `list panes`: `PANE_ID / TAB / TITLE / COMMAND / CWD`
  - `list layouts`: `NAME / SIZE / MODIFIED`（bytes・`3d 5h ago` 相対表記。内容実体のstat由来——symlinkはlink先をfollow〔TASK-82〕。参照先のstat取得失敗時〔壊れたsymlink・列挙後削除等〕は名前を残して両値 `-`〔JSONは`null`〕。`modified_epoch`はUnix epoch秒・mtimeがepoch以前の場合は`null`〔humanは`-`〕。誤った値（link先path長・link作成時刻・負epoch相当）を出さない）
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
- 失敗: session 0個は失敗ではなく正常系（TASK-93・2026-11-12）——0.44.3の`list-sessions -n`空応答（rc=1+stderr固定文+stdout空）はbackendが`Ok(vec![])`へdecodeするため（DD-3.1）。humanはheaderのみ+stderrへnote（`zelper: note: no running zellij sessions; create one with 'zelper add session NAME'`）・JSONは`data.sessions:[]`・compactは無出力、いずれもexit 0。真のbackend失敗（marker不一致rc=1・確認中のsession消滅等）は`OperationFailed`のまま
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
- **session（TASK-95・2026-11-13追加。設計書: `tmp/tasks/tm-95/design_rename-session.md`）**: requestは旧NAME+NEW_NAME（ともにpositional必須。`zelper rename session NAME NEW_NAME`）。実行は `zellij --session <from> action rename-session <to>`（top-level rename-sessionはzellij 0.44.3に存在しないため。実測M1/M3）。zellijは既存session名へのrename・空文字・同一名をrc=0・両stream空で**黙って失敗**し（M4 I・M95-B/C1）、'/'含み新名はcacheのsession_info directoryのみnested pathへ移動する部分的適用になる（M95-C2）。空白含み新名はzellij自体は受理する（M95-C3）が、TASK-98（DD-3.4）のname完全復元後も改行含み名は行分割で復元不能classとして残存するため、空白検査（`contains(char::is_whitespace)`）は当該classの遮断・add session NAMEとの対称性・既存契約維持のためにそのまま維持する（TASK-98設計§7。緩和はadd NAMEと両verb同時にのみ行うfuture option）。これらのためwrap側の事前検査+postcondition検証が本質: (1) app層Usage検査（空文字・'/'含み・空白含み・先頭`-`・同一名——一覧取得より前。先頭`-`はTASK-97・2026-11-14追加〔設計書: `tmp/tasks/tm-97/design_rename-leading-dash.md`〕: `--`option終端経由（`rename session NAME -- -x`）がCLI解析を通過してbackendの`--`挿入なしargv組み立て（process.rs run_action_as）による`zellij --session <from> action rename-session -x`まで到達するため——未知option値はzellij側clapがrc=2で拒む〔M95-C4実測済み〕が、有効option一致値の下流挙動は未測定でzellij側option集合はzeller管理外のためzellij委ねは契約保証不能とし入力段で確定拒否〔TASK-96 add session NAMEと対称の4種目〕。旧NAMEは文字種検査対象外——世界model〔一覧行〕にある名前はそのままrename対象とする既存契約〔不在はNoTarget+candidates〕で、NEW_NAMEと非対称）(2) 事前list_sessions 1回の取得で旧名不在（NoTarget・candidates=全行名EXITED込み・行0件は「no zellij sessions found」）・EXITED旧名（Preflight・attachによるresurrection案内）・NEW_NAME衝突（Preflight・EXITED込み）を判定 (3) rename呼出（失敗はOperationFailed exit 5）(4) 事後list_sessionsでpostcondition: 旧名行消失+新名行がrunning出現の両方（不成立は`VerificationFailed` exit 7。事後一覧取得失敗は元class/exit保持+`renamed session 'NAME' but failed to confirm: <元message>`——add sessionと同一pattern）。報告: human `renamed session 'old' -> 'new'` / JSON `data.renamed_session`=旧名・`data.name`=新名。dispatchは例外dispatch（DD-14・probe backend）

## DD-9. `resize` アルゴリズム

### grow/shrink

- request: PANESPEC、方向、STEPS（既定1）
- 実行: `resize <increase|decrease> <dir> -p`をSTEPS回
- postcondition: 各step後に`list-panes -g`でgeometry取得。変化なしが2回連続 → それ以上のstepを実行せず「これ以上変更不可」と報告（exit 0、warning）。**no-opがあり得る**ため無検証の成功を返さない

### equalize

1. 対象（--tab または PANESPEC群）のtiled pane群のgeometryを`list-panes`で取得（floating paneは対象外）。両者の併用はusage error（exit 2・DD-1.5）。PANESPEC優先での黙無視はしない
2. 目標サイズ計算: 対象が同一直線配置（横1列→cols均等、縦1列→rows均等）の場合は算術均等。それ以外（格子状）は行毎・列毎の均等を順に適用するヒューリスティック
3. 収束ループ: 最大20 iteration。各iterationで最大偏差のpaneに`resize`を1 step適用し`list-panes -g`で再取得。偏差が1行/列以下または進捗なし（2連続no-op）で終了
4. 結果報告: 達成geometryをJSON/humanで返す。**完全均等を保証しない**（helpとJSONの両方に明記）
- 反復上限とno-op検出により無限ループは構造的に不存在

## DD-10. `remap` アルゴリズム（v2）

初版: 2026-08-21 / 全面改訂: 2026-09-03（TASK-34/37。E6実験によるcross-tab移動の実証を受け、basic-design 4.6の原本仕様へ復帰）/ 改訂: 2026-09-15（TASK-74。run一致照合のkeyを`terminal_command`（invoked_with序列化）へ変更・`default_tab_template`の生成KDL反映・poll中の空応答retry。改訂の根拠と証拠は`remap-v2-matching-fix-task74.md`）/ 改訂: 2026-09-15 v2.2（TASK-75。multi-tab layout全体再現: 反復単位をlayout全体へ一般化・tab名/tab focus/pane focus再現・`focus-pane-id`追加・正規形TabTemplate列へのlayout解決一元化。根拠と証拠は`remap-v3-multi-tab-task75.md`）。改訂: 2026-09-19（TASK-81。`zellij run`起因paneのcwd常時Some確定〔T0実験 + zellij 0.44.3ソース解析〕に伴う照合修正: occupied slotへのcwd注入・shell slotのadaptive反転・自spawn複製paneのclose・dry-run floating緩和・検証(e')(e'')追加）。改訂: 2026-09-20（TASK-81第1回E2E判明不具合F1/F2。reconciliationのdup/slot対応付けを位置規則から多段規則〔属性・数突合・一意性判定・安全側fallback〕へ全面置換〔幾何前提の全廃〕・過剰検出gateのexited/held拡張・短命command dupのheld化回収・検証(e'')の対象集合拡張・10.1事実5へE2E確定事項〔押し出し位置非整合・起源別invoked_with分類〕追記）。改訂: 2026-09-20後半（TASK-81再E2E判明 R1/R2/R3: bare dup対応づけのvisual order補助を追加辺から**属性辺と両立するマッチングへ絞り込むtie-break**へ変更〔R1・10.7 step 6〕・対象tab内に残留するsource外held paneの`source_excluded_panes`報告〔R2・10.9〕・10.13へ幾何検証対象外〔bare pane群のswap・配置とsource外held paneの巻き込み〕を追記〔R3〕）。本節が旧DD-10（適用単位=tab、M>N既定error、fill/nest/tabs 3 mode、`--overflow tabs`のkill+command再起動、`--session-scope`のper-tab独立適用）全体を置き換える。設計レビューR1〜R7・R9を反映済み。

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
   - TASK-74精密化（spike S実証・2026-09-15）: 照合keyのdata sourceは`list-panes`の**`terminal_command`**（= paneの`invoked_with()`〔起動時Run〕の序列化）である。**`pane_command`はPTYの現在の前景process**（/proc由來）であってinvoked_withとは無関係（shell paneでは常にshell自身）。bare `pane` / `new-pane` 起動のpane（既定shell）はinvoked_with=Noneでありbare slotと完全一致する（layout bare slot起動は経路依存: session開始layout由来はNone、CLI new-tab経由はRun::Cwd化する。下記TASK-81訂正）
   - **TASK-81訂正（2026-09-19・T0実験 + zellij 0.44.3ソース解析）**: `Run::Command`の等価比較はcwdを含む全field一致である。**`zellij run`は--cwdなし起動でもinvoked_withのcwd=Some(クライアントcwd)である**（zellij-utils cli.rs / actions_from_cliが`cwd.map(join).or_else(|| Some(current_dir))`で常に付与。TASK-74の「--cwdなしはcwd=Noneのため無cwdslotと照合する」は誤りであり、T0でquote無しargvのrun paneも重複spawnした実測と整合する）。**hold属性は既定で一致する**: -cなし`zellij run`のhold_on_close=true（cli.rs `--close-on-exit` default false）== KDL未指定command slotのhold_on_close=true（kdl parser `unwrap_or(true)`）、hold_on_start=false・use_terminal_title=falseも両者一致——**通常の`zellij run`起因paneではcwdが唯一の照合差分**（差分が残るのは`-c`/`-s`/originating_plugin付き等のみ。10.13）。plugin leaf slotは既存plugin pane（Run::Plugin）と照合し宣言位置へ配置される
   - **`pane_cwd`（list-panes）は/proc由来の「現在の」cwd**であり、起動時cwd（invoked_with.cwd）そのものを得るCLI経路は存在しない。プロセスがchdirしない限り両者は一致するため、cwd注入の情報源として使う（10.8。chdir・正規化差の残差は10.13）
   - **CLI `new-tab`（action）の既定paneはinvoked_with=Some(Run::Cwd(クライアントcwd))**である（actions_from_cliが常にcwd=Someを添付 → plugins NewTabのadd_cwd_to_layoutがbare slotをRun::Cwd化 → ptyはshellをspawnし、paneのinvoked_with=layout.run）。KDLの`pane cwd="X"`（commandなし）はRun::Cwdを表現可能。shell paneのinvoked_withクラス（None系: session開始layout/new-pane/keyboard new-tab由来 vs Run::Cwd系: CLI new-tab由来〔zelper自身の空group生成tabを含む〕）はlist-panes上で判別不能なため、shell slotは10.7 step 6のreconciliationでadaptiveに判定する
   - **TASK-81 E2E確定（2026-09-20・第1回E2E〔T0'/T0p/T0q/T0k。一次記録: `tmp/tasks/tm-81/`〕）**:
     - **起源別invoked_with実測分類の確定（従来の未確定事項を解消）**: None系 = session layout起点 / `new-pane` / keyboard new-tab（T0k実測）・Run::Cwd系 = CLI `new-tab`既定pane（T0'/T0k実測）・Run::Command系 = `zellij run`（T0実測）。list-panes上で起源を直接判別する手段は変わらず存在しない（分類はE2E観測による裏付けであり、実行時の判定は10.7 step 6 reconciliationがadaptiveに行う）
     - **押し出し位置の非整合**: run一致照合でslotに入った既存paneの介在によりslot外へ押し出されたpaneの位置は**slot参照幾何と一致せず、visual order（pane_y, pane_x昇順）とslot順の一致を壊す**（実測: t0p (100,0)・t0q (0,15)）。旧reconciliation位置規則「visual order s番目 == slot s」がこの介在で破綻したことが第1回E2E不具合F1の根拠であり、10.7 step 6の対応規則を多段規則（属性・数突合中心）へ全面置換した理由である（10.7 step 6・10.13）
     - **短命command paneのdupのheld化**: 照合不成立slotから再spawnされた複製paneが即座にexitしhold（exited / is_held）化する系列が実在する（t0q qp8実測）。held化したdupは生きpane集合（`is_remap_source`相当）から外れるため、dup close・pane数検証から逃避し残留する。10.7 step 6のgate条件拡張・検査対象集合拡張（exited/held込み。10.9 (e'')）で回収する

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
| 14 | shell slotのrun表現（TASK-81） | pass1はbare。reconciliation（10.7 step 6）で当該slotの不一致を検出した場合のみ`pane cwd="<当該paneのpane_cwd>"`へ反転（高々1回） | 常にbare（CLI new-tab由来shell〔zeller自身の空group生成tabを含む〕で常に重複spawn。T0実測）/ 常にcwd node固定（None系shell〔session開始layout・new-pane・keyboard new-tab由来〕で常に重複spawn。shellのinvoked_withクラスはlist-panesで判別不能のため、単一encodingではT0のような混合sessionを解けない） |
| 15 | 照合不成立でspawnした複製paneの扱い（TASK-81） | **closeする**（対象は当該実行開始前snapshotに存在しないid・安定観測後・**対応規則特定（exited/held化したdupを含む。10.7 step 6多段規則）**のもののみ。close実績は成功/失敗双方で報告〔10.9〕。要件2.6の限定条項） | closeしない（重複プロセスが恒久二重実行し続ける。T0でheartbeat書込が約2倍に加速した実測） |
| 16 | 生成KDLのhold属性併記（TASK-81） | **併記しない**（-cなし`zellij run`のhold_on_close=true == KDL未指定既定〔10.1事実5〕。T0'実証後確定） | `close_on_exit=true`併記（hold_on_close=falseとなり正常系paneと逆に不一致を生む。実測で必要性が示されるまで禁止） |

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
  - **move系との共用（DD-13・TASK-88）**: plugin binary・protocol・permissions seed・probe機構をremapと完全共用（変更なし）。moveの`--to-tab`/`--swap-with`は既存`break-id`の部分利用のみ（新command追加なし・version skew riskゼロ）。runtime orchestration側（companion setup + probe pipe・polling定数・break-id payload組み立て・権限hint）は`src/companion.rs`へ切り出し済みでremap/move双方から利用する

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
- floating pane: scope内にselectable floating terminal paneがあればpreflight error（既定。**実行時のみ**。dry-runは10.10の緩和規定〔仮想embed計画+警告+exit 0〕。TASK-81）。`--embed-floating`で`toggle-pane-embed-or-floating -p`（プロセス保存・実証済み）によりtiled化してsourceに組入れる。tiled化はlayout解決・plan確定**後**の状態変更第1歩とし、dry-runでは実行しない（計画はtoggle前snapshotから仮想的にfloating paneを含めるためdry-runと実行の計画が一致する。PR#1対応の維持）。tiled化の対象はpreflightで特定したselectable floating terminal paneのみ（probe自動launchで現れるcompanion plugin pane（floating・non-selectable）はtoggleしない。10.3）。companion plugin paneはsourceに現れない（10.3）

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
  - (ii) slot対応は**run（command+args+cwd。hold属性は既定値一致〔10.1事実5〕）に一意なpane**について決定論的に定まる（run一致照合。TASK-81: 照合keyにcwdを含む）
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
   - **照合不成立のreconciliation（TASK-81。2026-09-20 E2E改訂: dup/slot対応付けを位置規則から多段規則〔属性・数突合中心〕へ全面置換。幾何前提の全廃）**: step 6の各groupへのoverride-layout後に**安定観測**を行う——当該tabのselectable tiled terminal pane id集合（**exited / is_heldのpaneを含む。E2E改訂F2**）がpolling（250ms間隔・10s deadlineは既存と同一）で**2回連続で同一、かつ要素数 >= N_t**（spawnは追加単調なため安定=反映完了。1回きりのlist-panesでは判定しない。**TASK-81レビューC2注記**: 実装の安定観測完了条件はid集合の2回連続同一のみとする——「>=N_t」相当の過不足検知はreconciliation発動条件〔生きpane数超過・snapshot非存在のexited/held pane存在のgate〕とpass 2後過剰判定・残余close〔下記5.〕が時系列的に後に担っており、安定観測の完了条件に要素数下限を重ねる必要はない。gate通過後の呼び出しであるため実害は限定的であり、実装は不変とする）。**発動条件**: 生きpane数（exited/held除く）> N_t、**または生きpane数 == N_tでも当該tab内に実行開始前snapshot非存在のexited/held paneが存在する**（短命command dupのheld化回収。E2E改訂F2。正規spawn〔空slot由来〕のheld化との弁別は下記規則で行い、空slot相当の新規paneはclose対象外）。発動した場合:
     1. **自spawn候補の定義**: 当該tab内のterminal pane全体（exited/held含む）かつ実行開始前snapshot非存在id。dup特定・close・残余close・検査の対象集合をこの候補に揃える（`is_remap_source`は生きpaneのrun照合候補という本来用途に戻し、exited/heldの除外はsource選定のみに適用）
     2. **command dup（argv一致逆引き・位置不問）**: command slot割当（run=Some）ごとに、候補中の`terminal_command` == 当該slot argv `join(" ")` であるpaneを逆引きしてcloseする（reason=command dup）。押し出し介在やheld化の影響を受けない属性一致特定であり、command dupはcwd反転の対象表現を持たないためcloseのみとし検証失敗経路へ合流する
     3. **bare dup（数突合+消去法）**: 候補中`terminal_command=None`群（D個）と空slot数（E個）を突合し、超過分 U = D - E を不成立shell slot由来のdupとみなす。不成立shell slot（割当ありshell slotのうち割当paneのpane_cwdがSomeのもの）との対応を**属性照合**（dup paneのpane_cwd == 割当paneのpane_cwd）による対応候補と**一意対応判定**（完全マッチングがちょうど1通り）で特定する。shell slot 1つのtabでは対応は一意。shell slot複数で属性のみ一意にならない場合（同一cwd shell slot複数+dupの曖昧構成を含む）は補助規則としてvisual order位置（exited/held込みの順位 == slot s。従来の位置規則）を**属性辺と両立する（全pairが補助辺を満たす）マッチングへ絞り込むtie-break**として使用し（2026-09-20再E2E R1改訂: 追加辺〔属性or補助〕では属性辺の曖昧さを解消できないため絞り込みに用いる。実測ではdupはslot参照位置にspawnされる〔t0p2 terminal_7・t0q2 terminal_14〕ため補助辺の精度は十分。属性辺で完全マッチングが立たない系列は従来どおり追加辺として使用）、残りがちょうど1通りなら採用する。それでも一意にならない場合は**安全側fallback（当該tabのbare dupを反転もcloseもせず検証失敗経路へ。誤close・誤反転回避）**とする
     4. **close + pass 2（再override）**: 対応づいたbare dupをcloseし、対応shell slotのみ`pane cwd="<当該割当paneのpane_cwd>"`に置き換えた生成KDLでoverride-layoutを再実行する（**close単独で完了扱いにしない**——closeだけではslot充填が起こらないため、closeは必ず再overrideとセット）。反転は高々1回・**複数slotの同時反転可**・**command dup close（2.）も同一passに載せる**（close群→pass 2 overrideの順）
     5. **残余close**: pass 2後も過剰（生きpane数 > N_t、またはsnapshot非存在のexited/held pane残存）なら以降の反転を行わず、残った新規spawn paneを**属性・数突合版**（空slot正規spawn相当〔terminal_command=None群のうち空slot数に相当する分〕をclose対象から除く。command dup相当はargv一致でclose）でcloseして検証失敗（10.9）へ合流する（無限反復の構造的排除）。close試みができなかった・対応づかなかった新規id pane（exited/held含む）は検証失敗で報告する（誤close回避の安全側）
7. 検証（10.9）→ **最終go-to**（best-effort。v2.2）: layoutにfocus=true鋳型があれば文書順最初のfocus=true鋳型 t* のblock 0 tab (0, t*)へ、無ければanchorへ復帰（T=1では前者も (0,0)=anchor と一致し現行挙動と同一）。**複数のfocus=true鋳型がある場合は文書順最初を採用しwarningを出す**（E2E要因改修・2026-09-16明文化: zellijはdump上常にactive tabへfocus=trueを付けるため複数指定を機械的に判別できず、zelper側規則として文書順最初に確定し採用を利用者へ通知。10.10のwarning伝達経路）

各phaseで状態を再取得し、長寿命のtab id参照を持たない（DD-2）。**DD-2原則の適用範囲（v2.2明確化・TR75-4）**: 「長寿命」とは実行を超えた保持（構造体field・global・file等への残置）の禁止であり、**単一execute実行scope内のtargets配列（`targets[b*T+t]`・現行から同じ運用）での逐次参照は許容**する。tab idはclose後再利用される不安定キーであるため、実行内でも参照のたびに存在確認を行う: **step 6の各group処理直前にlist-tabs（lenient）でtargets[j]の存在を確認し、不在なら再解決する**——割当ありgroupはpane所属基準（10.7 5-bと同一のmembership特定）・空groupはrename済み生成tab名基準（list-tabsのname一致。go-to-tab-name〔20260915_doc_task75_go_to_tab_name_help.txt〕は同名tab曖昧性のため補助にしか使わない）。再解決不能は`OperationFailed`（中断時報告）。自己作成tabは自実行内にcloseされない限りid再利用の競合sourceが存在しないため、この保持・確認で十分である。

**空groupのみの実行（M=0・またはsource全てanchor内で新規tabが空groupのみ。TR75-5）**: companion plugin・probeは不使用（break系pipe 0件）。preflightは現行と同一depth（layout解決・N_t=0検証・10.6正規形化LayoutInvalid・plan・生成KDL計算）を**状態変更前に完了**し、new-tab実行時点に未知の検証は残さない。new-tab経路の実証（tab id parse・既定paneが生成KDL適用でN_tへ正規化されること）はfake/E2E必須条件とする（TASK-75設計書§5）。

移動phase完了時点で各target tabはちょうどそのgroupのpaneのみを持ち（空groupのnew-tabは既定1 paneのみ）、layout適用のrun一致照合に残りpane（未照合の入れ子・重複spawn）が介入しない状態になっている。

### 10.8 生成KDLの規則（v2.2: 正規形subtree化）

- 全生成tab (b,t)（M<=S単一block含む）で共通の統一規則。生成KDLは**tab nodeなしの`layout { subtree }`形式**（per-tab個別適用。tab属性〔name・focus〕は生成KDLに載せずrename-tab-by-id・go-to-tab・focus-pane-idで再現）:
  - base = 当該tab鋳型の**正規形subtree**（default_tab_templateのchildren置換済み・tab属性除去済み。v2.2で置換を10.6 layout解決へ一元化し、generatorは置換済みsubtreeのみを受取る）。改行区切り形式・値quote必須（DD-3.3）。`override-layout`へはargvで直接渡す
  - occupied slot: 当該paneの**`terminal_command`（invoked_with序列化）**を注入する。`command` = terminal_command空白分割のargv[0]、`args` = 残り。**`cwd` = 当該paneの`pane_cwd`を注入する**（TASK-81改訂: `zellij run`起因paneはcwd=Someが常に成立し無cwdslotと照合しないため〔10.1事実5〕。pane_cwdは/proc由来の現在cwdで起動時cwdの近似〔10.1〕。**pane_cwd=None〔enrich 100ms timeout欠落等〕はwarningを出して注入をskip**〔10.13〕。TASK-74の旧規則「pane_commandがSomeなら注入する」はpane_command=PTY前景processであるため廃止したまま）。hold属性（close_on_exit/start_suspended）は併記しない（10.2 #16）
  - `terminal_command` = None（bare・既定shell起動pane）: **既定はbare pane slotのまま**（pane_commandの値は参照しない）。10.7 step 6のreconciliationで当該slotが反転対象と判定された場合のみ、再適用時の生成KDLで`pane cwd="<当該paneのpane_cwd>"`（Run::Cwd表現。10.1）へ置き換える
  - 空slot: 常にbare pane（10.2 #9）。**空slotへのcwd注入は行わない**（再作成paneが存在しなくなったため。occupied slotはspawnしない・空slotは既定shell。TASK-81: occupied slotへのcwd注入〔上記〕と区別）
  - **default_tab_template反映（TASK-74。v2.2で一元化）**: template由来のbar leaf等は正規形subtreeに含まれたまま生成KDLへ出力される（template内plugin leaf〔compact-bar等〕はslotを消費せず維持され、既存plugin paneとRun::Plugin照合して宣言位置へ配置される。10.1 事実5。新規tabにbar paneが既にあれば照合で消費され、無ければ新規spawnされる——いずれもbar 1件）。`children` node不在・置換後leaf数不一致の`LayoutInvalid`判定は10.6正規形化（layout解決時点）で行う
  - それ以外のnode属性（size / split_direction / name等）はlayout宣言をそのまま維持する。plugin leafはslotを消費せず維持（MR-32規則）
- 既知制限（10.13）: terminal_command空白分割は引数のquoteを復元できない。同一commandのpane群どうしのslot割当順はzellijのrun一致順に依存する

### 10.9 検証（postcondition）

layout適用phase完了後にlist-panes / list-tabsを再取得し:

- (a) 全source pane idが生存し、割当group（block×tab）のtabに所属（**pane id基準**。tab id基準の照合はid再利用回避のため補助にしか使わない）
- (b) **各生成tab (b,t) のselectable tiled terminal pane数 == N_t**（**anchor (0,0)を含む全生成tab**が対象。anchor例外は名前・位置のみであり形状・slot数の例外ではない。command照合missによる重複spawn・未照合paneの入れ子残留を検出。「pane数不一致も検証失敗」のPR#1規則を継承）
- (c) 生成tab名が10.6命名規則どおり（**(0,0)=anchorは名前検証から除外**——anchor tab名保持の唯一の例外。10.2 #6注記）。tab一覧にanchorが残存
- (d) **pane focus（v2.2・E2E要因A改訂: 位置ベース）**: 各生成tab (b,t)で、当該tabのterminal paneをvisual order（geometry y,x昇順）に並べた**s番目（s = 鋳型pane_focus_slot、無ければslot 0）のpane**の`is_focused == true`（list-panes経由。非active tabもis_focusedを持つ。10.1 事実6）。10.7 step 6のfocus対象決定と同一の位置基準であり、**全生成tabが対象**（空slot tab除外は撤廃）
- (e') **run一意割当の位置照合（TASK-81）**: 当該tab内でrun（command+args+cwd）が一意な割当に限り、適用後の当該tab visual order（geometry y,x昇順）s番目のpane id == 割当pane idであることを検証する（決定論性保証範囲(ii)の対象のみ。同一run間・shell pane間は(iii)例外のまま位置検証対象外〔E2E幾何比較で担保〕。reconciliation後の「元paneがslotから押し出されたまま」をcount検証では検出できないための追加）
- (e'') **割当slot位置の新規id残存検査（TASK-81。2026-09-20 E2E改訂: 対象集合拡張）**: 割当ありslot位置（visual order s番目。**順位付けはexited/held paneを含む当該tabのterminal pane全体**）に、当該実行でspawnした新規id pane（実行開始前snapshotに存在しないid・**exited/held含む**）が残存していないことを検証する（dup close漏れ・反転不成立・**短命command dupのheld化残留〔t0q qp8〕**の検出）。加えてreconciliation（10.7 step 6）で対応づけ・closeできなかった新規id pane（exited/held含む）は検証失敗で報告する（誤close回避の安全側）。**(b)との補完関係**: (b)の生きpane数検証はexited/held paneを数えないため、held化dupの残留は(b)では検出されず、本検査とreconciliationの安全側fallback（10.7 step 6 安全側fallback・残余close）が検出する
- **leftover_tabs報告（v2.2）**: tabs_afterのうちtargetsに含まれないtab（source外paneのみのtab・companion plugin host tab等）を`leftover_tabs`（id/name/position/selectable pane数）として報告する（humanはwarning行・JSONはdata field）。**closeしない**（閉じる場合はremove verbの明示利用。要件2.6）
- **対象tab内source外pane報告（TASK-81再E2E R2・2026-09-20）**: 対象tab（targets内）に残留する「実行開始前snapshotに存在するがexited/heldのためsource選定から外れたterminal pane」（10.5の対象外規則により元tabに残すpane。対象tab再利用時は対象tab内に残留）は、`leftover_tabs`（targets外tabの列挙）にも生きpane数検証（(b)の`is_remap_source` filter）にも現れないため、`source_excluded_panes`（pane id・tab id・title・exited/held）として報告する（humanはwarning行・JSONは成功`data`と失敗`error.data`の双方。t0s sl1実測）。**closeしない**（snapshot存在paneはkill制約の対象。DD-12）・検証失敗にもしない（幾何への巻き込みは10.13対象外）
- **検証の観測経路とdump-layoutの扱い（v2.2・TR75-8）**: 検証はlist-panes/list-tabs経由で行い、**dump-layoutとの完全一致は保証・検証対象外**である。E2E比較における比較対象は「tab毎slot数・幾何・bar存在・検証可能focus（is_focused・active tab）」に限定し、dumpの正規化表現（size="33%"等）・noise（new_tab_template残存・hide_floating_panes・zellij:link pane・active tabへ常に付くfocus=true等）・保証外項目（tab自動名・pane title・pane id）の除外対応表はTASK-75設計書§4.3に定める
- 失敗時: class=`VerificationFailed`（exit 7）+ 期待/実測差分 + 実行済み/失敗/未実行の区分 +（pipe効果が皆無だった場合）10.4の権限hint。`--json`時は成功envelopeを出さず、mapping/missingと`leftover_tabs`（Z-5・TASK-81）と`closed_duplicates`と`source_excluded_panes`（TASK-81再E2E R2）を`error.data`に載せた**単一の**error envelopeをmainから出す（PR#1規則の維持）。error文面は10.13の文面規則（cwd差異・hold flag差の可能性案内）に従う
- 成功時のJSON `data`: `mapping[]`（pane → block/tab/slot/TabId。全件preserved）・作成tabのidとposition・`snapshot_len`（従来fieldの継続）・`t`/`s`/`n_slots`（v2.2追加）・**`closed_duplicates[]`**（TASK-81: 当該実行でspawnしcloseした複製paneのid/slot/reason。成功・失敗双方で報告する報告義務。10.2 #15/DD-12）・**`source_excluded_panes[]`**（TASK-81再E2E R2: 対象tab内に残留するsource外exited/held paneの報告。成功・失敗双方）
- 最終go-to（focus復帰）失敗は操作失敗に加えない（best-effort。従来通り）

### 10.10 dry-run（v2.2拡張）

- 出力: (a) source pane一覧（visual order・現在のtab付き。TASK-74改訂: 各paneの`terminal_command`〔invoked_with〕を含める。`pane_command`〔前景process〕は表示目的で残す）、(b) M / S / k に加え **T（鋳型数）とper-tab slot数列 N_t**、(c) pane → (block, tab, slot, 生成tab名)割当表（全件preserved・empty slot数）、(d) **生成KDL preview**（生成tab (b,t) 毎。humanは`[plan]` block、`--json`は`instances[].kdl`）、(e) 実行予定backend操作列（companion setup・probe pipe・break pipe〔break-id/break-new〕・new-tab（空group）・rename・go-to・override・**focus-pane-id**・最終go-to先を種別表示）
- JSONの`n` fieldには**S（layout全体slot数）**を格納する（T=1では従来値Nと同一。stable fields〔DD-4.2〕外のdata内部構成拡張のため契約変更にならない）。新規に`t`・`s`・`n_slots`（N_t列）・鋳型のname/focusを追加
- **preflight warningの伝達経路（E2E改修・2026-09-16明文化）**: 複数focus=true鋳型/pane検出・quoting等のpreflight warningは、human実行・--json実行・dry-runの全経路で**stderrの`warning:`行**へ出力する（--json実行時もstdoutのenvelope契約を壊さない）。加えて機械可読経路としてdry-run/成功時のJSON dataへ**`warnings`（文字列列）**を格納する
- 状態変更は一切なし: tab切替・pipe・wasm extract・permissions.kdl書換・focus-pane-idを行わない（DD-12）
- **floating保有scopeの緩和（TASK-81・破壊的変更）**: `--embed-floating`なしのdry-runは、scopeにselectable floating terminal paneがあってもpreflight errorにしない（従来はexit 7で計画が見えなかった）。**仮想embed**（MR-1/MR-34の`--embed-floating` dry-runと同一logic）で計算した計画を表示し、`warnings`配列へ「実行には`--embed-floating`が必要（floating paneはtiled化して組入れられる）」警告を出し、**exit 0**で終了する（changelogに破壊的変更として明記。実行〔dry-run以外〕は10.5のとおりpreflight errorのまま）

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

- terminal_command空白分割: **arg境界はjoinで消失しており、quote文字の有無にかかわらず空白を含むargは全ケースで復元不能**（例: `bash -c "echo hello world"` はquote文字を含まないのに誤argv化する）。該当paneのrun一致が外れ、(b)検証（pane数 == N_t〔tab毎〕。v2.2）が重複spawnとして検出する。preflight warningは (1) terminal_commandに`"`/`'`/`\`/制御文字を含む (2) 空白分割argv[0]がshell系（*sh/bash/zsh/fish等）かつargv[1]が`-c`（高疑いheuristics）のいずれかで出力（実行は可。検出は完全でない）。恒久対応はzellij側のargv列API待ち（TASK-74→TASK-81拡張）
- **cwd起因の照合阻害は10.8のcwd注入で解消**（TASK-81）。残る照合外れ要因: (1) プロセスが起動後にchdirした（pane_cwd≠起動時cwd） (2) bind mount・path正規化差（pane_cwdは/procの正規化path、invoked_with.cwdはクライアント側文字列） (3) 生成KDLのcwd値に`$`を含む（zellij側でshellexpandされる）。これらは救済なし。**検証(b)失敗のerror文面規則**: 「pane count mismatchはcwd差異（chdir・正規化・bind mount）とhold flag差（`-c`/`-s`起因）の可能性によりrun一致が外れたことによる」旨を案内する（TASK-81。従来のstartup cwd一文を拡張）
- **pane_cwd欠落**: list-panesのcwd enrichは100ms timeoutでpane_cwdが欠けうる（T0 r2afterで実在）→ 該当occupied slotはwarning+注入skip（10.8）となり照合が外れうる。(b)検証で検知。E2Eでの欠落注入は任意の検証項目
- **hold flag・use_terminal_title・originating_plugin・EditFile pane**（TASK-74→TASK-81拡張）: `hold_on_close=false`（`zellij run -c`）・`hold_on_start=true`（`-s`履歴）起動pane、`use_terminal_title=true`・`originating_plugin=Some`起動pane（ともにKDL run nodeから表現不能）、`zellij edit` pane（Run::EditFileはterminal_commandに現れない）は照合から外れ(b)検証で検知する（10.2 #16のとおりhold属性の挽回併記は行わない）。`exited`/`is_held`のpaneは従来どおりsource外
- 同一commandの複数pane: それらの間のslot割当順はzellijのrun一致順依存（順序は保証しない——10.6決定論性保証範囲(iii)の要件例外。**requirements.md §2.6にも同一の例外を明記済み**〔TASK-74設計レビューTR74-3〕）。プロセス保存・pane数・所属tabは保証
- shell pane（terminal_command None）: bare slotとのみ一致。複数shell pane間の順序は保証しない（同(iii)の要件例外）
- tab id再利用: 「list-tabs直前取得・直後に消費」（DD-2）+ 新規tab特定はpane所属基準（10.7 5-b）+ index系API不使用、で防御
- 並行client操作: remap実行中の他client操作（pane追加等）は競合して検証失敗になり得る（再実行で解消）。排他機構は提供しない。TASK-81追加: dup close（10.2 #15）は「実行開始前snapshot非存在id ∧ 対応規則特定（10.7 step 6多段規則）∧ 過剰数==反転対象数の整合」の条件下でも、実行中に他clientが追加したpaneがたまたま対応候補の属性（argv一致・pane_cwd一致）を満たす場合の誤close競合が原理的に残る。報告された`closed_duplicates`のidを利用者が確認できる経路で緩和する
- swap layout / template: SKIP_NODES規則でslot数から除外（MR-32）。remap後のswap自動切替の挙動は保証しない（従来通り対象外）。`new_tab_template`は対象外のまま（zellijが新規tab生成時に使う予約領域。dumpへの残存は実機挙動で、remapの再現対象外。TASK-75）
- **multi-tab再現の対象外（TASK-75・v2.2）**: (1) pane nodeの`name`属性→pane titleは保証外・観察のみ（実測ではtitleへ反映されるが、remapはtitleを設定せずrun一致照合とも無関係） (2) 名無しtab鋳型のzellij自動名「Tab #N」は再現しない（採番がCLIから制御不能のためzelper側命名に置換。10.6） (3) ~~focus-pane-id対象が空slotのtabはfocus設定・検証から除外~~ **撤廃（E2E要因A改訂）**: focus対象を適用後のlist-panes位置から決定する設計へ変更し、空slot（spawn pane）も位置から特定する（10.7 step 6） (4) anchor tabの位置はlayout最初のtab位置と一致する保証がない（anchor再利用・10.2 #6の帰結。名前と同様に例外扱い。**形状・slot数は例外ではない**——10.9 (b)はanchorを含む全生成tabへ適用） (5) 過剰tab（targets外の残存tab）は放置し報告のみ（10.9）。**要件側にも(1)(2)を含む同一の例外を明記済み**〔TASK-75設計レビューTR75-1。2026-09-16。requirements.md §2.6〕
- **短命command paneのremap再spawn dupのheld化（TASK-81 E2E改訂・一般制限）**: 短命command（即座にexitする`zellij run`起因pane等）をslotに持つremapでは、run一致不成立で再spawnした複製paneが即座にexitしexited/held化する系列が一般に存在する（t0q qp8実測）。held化dupは生きpane集合から外れるため、reconciliationの対象集合・gate条件・検証(e'')をexited/held込みに拡張して回収する（10.7 step 6・10.9 (e'')）。close試みができなかった・対応づかなかったheld新規id paneは検証失敗で報告する（誤close回避の安全側）
- **reconciliation対応規則の既知限界（TASK-81 E2E改訂）**: shell slot複数のtabで不成立slot特定は属性（pane_cwd一致）のみでは一意にならない場合がある（t0p trio実測: 割当shell pane・dupとも同一cwd）。visual order補助は実績1例（work tab 2-pane）に依存する位置基準であり、押し出しpaneの介在で順位==slot対応が崩れるため（10.1事実5 E2E確定）、一意性が得られる場合のみの補助に格下げした（2026-09-20再E2E R1: 補助の用法を追加辺から属性辺と両立するマッチングへ絞り込むtie-breakへ変更。同一cwd shell slot複数+dupの曖昧構成〔t0p2 terminal_7・t0q2 terminal_14〕で追加辺では曖昧さが解消できずdup残留でexit 7となっていた系列を解消）。いずれでも一意にならない場合は安全側fallback（反転もcloseもせず検証失敗）とし、誤反転（成立済みslotのpane追い出し）・誤closeを回避する。将来の拡張点として、list-panes幾何をslot参照幾何（生成KDLのsize/splitから計算した期待矩形）と直接照合する方式が挙げられる（押し出しpaneの位置を幾何的に弁別できる可能性。本改訂では実装しない）
- **幾何検証の対象外（TASK-81再E2E R3・2026-09-20）**: (e')の位置照合が検査するのはrun（command+args+cwd）一意な割当のみのため、run paneの視覚swap・宣言`split_direction`の非実現は検証missingとして捕捉される一方、**bare pane群（shell pane・空slot正規spawn）のswap・配置は検査対象外**である（同一run間・shell pane間は決定論性保証範囲(iii)の例外。10.6。E2E幾何比較でのみ観察可能）。また**対象tab内に残留するsource外held pane（snapshot存在・source選定外。10.9 `source_excluded_panes`報告の対象）は当該tabの幾何を巻き込み**、slot参照幾何（生成KDLのsize/splitから計算した期待矩形）との不一致が生じうる（t0s sl1実測）が、kill制約のためcloseせず報告のみ行う（巻き込み自体の検証対象外）
- permissions.kdlはcompanionのversion別pathのnodeを蓄積し得る（旧versionのnodeは残る。無害・手動削除可）

### 10.14 error経路（v2で残るもの）

M>N既定errorとoverflow指定漏れerrorは廃止。残る経路とclass:

- `Usage`: 3source排他違反等（DD-1.5）
- `LayoutNotFound` / `LayoutInvalid`: layout解決不能・parse失敗・N_t=0（terminal slotを持たないtab鋳型を含む。v2.2）・default_tab_templateのchildren不在/置換後leaf数不一致（10.6正規形化時点）
- `Preflight`: floating pane存在（--embed-floatingなし）/ companion setup失敗（wasm書込不可・permissions.kdl parse/書込不可）/ `--tab`解決不能は`NoTarget`
- `ZellijUnavailable` / `UnsupportedVersion`: DD-3.1（最小0.44.3）
- `OperationFailed`: pipe CLI失敗（blockによるtimeoutを含む）・polling timeout・go-to/override/rename失敗
- `VerificationFailed`: 10.9 (a)(b)(c)(d)(e')(e'')の不一致（TASK-81: (e')(e'')追加。失敗時のerror.dataへleftover_tabs・closed_duplicatesを載せる〔10.9〕）

## DD-11. `add` / `remove` 操作設計

### add

- pane: `new-pane`をcount回（`--cwd`/`--name`/`--tab-id`/`--`commandをmap）。戻りIDをresultsへ
- tab: `new-tab`をcount回。`--layout`は`-l NAME`へ、`--path`は`-l PATH`へ、`--inline`は`--layout-string`へmap
- **tab `--layout`の受理仕様（TASK-91・探索先統一TASK-102）**: `--layout`の値はzellij `-l`へそのまま渡り（`-l`値そのものの拒否・書換えなし）、名前解決の探索責任はzellij側。zellij 0.44.3の`-l`はlayout名とfile path（相対・絶対）の双方を正規受理する（top-level `-l` help原文「Name of a predefined layout inside the layout directory or the path to a layout file」・action new-tab -lへpath 5種でrc=0/tab作成を実測）ため、'/'入り（や`.kdl`拡張子付き）の値はpathとして解決され`--path`と等価に機能する。**zellerは'/'入り値を拒否しない**（value_parserもapp層検査も適用しない。設計書: `tmp/tasks/tm-91/design_add-tab-layout-restriction.md`）。規範入力はlayout名（DD-1.2 grammarの`--layout NAME`表記は維持）で、path形式は互換入力。**探索先統一（TASK-102。設計書: `tmp/tasks/tm-102/design-layout-dir-unification.md`）**: `ZELLIJ_LAYOUT_DIR`が設定されている（envがOkかつ空でない）とき、`--layout`値（name field）には`-l NAME`に加え`--layout-dir <env値>`を付与する（`-l`の前。未設定時・空文字時・`--path`・`--inline`はargv不変）。これによりzellijが名前探索として扱う入力（拡張子なし値）について探索dirが一覧基準（DD-3.4）と一致し、同名fileが双方のdirにあっても一覧・remapと同一dirの内容を読む（action clientの`ZELLIJ_LAYOUT_DIR` envは届かないためflagで揃える。TASK-91実測。zellij 0.44.3で`--layout-dir`指定時の名前探索はHOME既定dirへの切替〔併合でなくHOME既定は探索されない〕・builtin名は併存受理・path値への干渉なしを実測: `tmp/tasks/tm-102/m1_result.log`・`m2_result.log`）。残余制限: 一覧は`.kdl`を除いたstem表示だが、zellij 0.44.3は拡張子を持つ入力をpath解釈しlayout-dir探索を通さないため、ドット含みstem（`--layout foo.bar`）は一覧fileを読まない（値の書換え・拒否は行わない）。HOME既定dirにのみ存在する名前（一覧に出ない名前）は`ZELLIJ_LAYOUT_DIR`設定環境下で見つからない（rc=2→`OperationFailed` exit 5。探索先切替の帰結）。remap側positionalの'/'拒否（`parse_layout_name`）はzeller自身がlayout fileを解決・読込する経路のため維持され、addとは機構が異なる。保証境界: zellerが保証するのは`-l`値を拒否・書換えせず渡すことと（`ZELLIJ_LAYOUT_DIR`設定時の）`--layout-dir`付与までで、受理形式・探索先などの下流規則はzellij 0.44.3で確認した事実（対応版更新時の再確認事項——flagが受理されたまま探索優先順位・意味論が変わる場合は終了codeで検知できないため、同名別内容の読み分け実測を再実施する）
- postcondition: list-panes/list-tabsで存在確認
- 失敗: 作成済み分は残存させ、per-target結果報告（rollbackしない）

### remove

- pane: `close-pane -p`順次。**プロセスはkillされる**ことをhelpに明記
- tab: `close-tab-by-id`。事前に`list-tabs`でID解決・存在確認（存在しないIDがexit 0で無操作のため、postconditionで消失確認し`VerificationFailed`を検出）
- `--empty`: 「空tab」= selectable な pane（tiled+floating両方）が0個のtab。tab-bar等のnon-selectable pluginのみのtabが該当。対象をdry-run形式で列挋してから削除
- 安全: 2対象以上の破壊的削除、または`--empty`は`--yes`または`--dry-run`必須（非対話環境で確定的な挙動）。単一pane/tab削除は確認不要
- 失敗: 順次実行・部分失敗報告（残対象も実行）
- **session（TASK-84・2026-11-13追加。設計書: `tmp/tasks/tm-84/design_remove-session.md`）**: selectorは `NAME...`（1件以上・重複は除去し指定順保持）/ `--all`（全行・EXITED込み）/ `--dead`（EXITED行のみ）/ `--me`（ZELLIJ_SESSION_NAME解決の自session）の4者のうち**ちょうど1つ**（DD-1.5）。意味論は単一の完全削除（行もresurrection dataも消す。kill/deleteの意味論splitはzellij 0.44.3に存在しないため不採用。実測M4〜M8）で、各対象のbackend操作は `zellij delete-session <NAME> -f`（top-level positional・状態問わず1呼び出し）に統一。kill-session / kill-all-sessions / delete-all-sessions はzellerから呼ばない（kill-all/delete-allは非TTYでpanic rc=101・-y付きでも無操作rc=0等の応答が信頼できないため。実測M2/M4/M6/M7）。EXITED行は削除対象として受理（存在確認はlist_sessionsのEXITED込み。不在時のcandidatesは全行名〔EXITED込み〕でDD-1.2の--session事前確認〔running名のみ〕と異なる。行なし残data〔registry掃除済み・cacheにsession_info残留〕は対象外——既知制限としてhelpに記載し`zellij delete-session NAME`手動を案内）
  - **安全: sessionは対象数・selector種別によらず常にgate**（単一NAMEでも`--yes`/`--dry-run`必須。最小構成でも全pane・tab・resurrection dataを不可逆に巻き込むため。remove tab --emptyの単一gate先例と同じ理由）
  - **自sessionの保護（user決定2026-11-13）**: 自session削除は`--me`（selectorとして単独指定）のみ許可。`NAME...`にZELLIJ_SESSION_NAME該当名が含まれる場合は状態変更前にPreflight 7で拒否（`--me`へ案内）。`--all`は対象集合から自sessionを除外し、除外をstderrのwarning行で通知（dry-runにも反映。stdout契約は不変）。除外後0件（自sessionのみ存在）はNoTarget 3（`--me`案内文面）。`--me`はenv不在でPreflight 7（dry-runでも。plan表示に解決名が必要）・env設定時も他NAME対象と同一の事前存在確認（不在はNoTarget 3）。`--me`実行は通常code pathと同一（検証・報告の意図的skipなし）だが、自壊で報告・exit status・検証が実行されない可能性があるためbest-effort契約をhelpに明記し、実行前にstderr warning行（`warning: removing the current session 'X'; zelper may be terminated before reporting`）を出す
  - **dry-run**: per-targetのplan形式。state（running/dead）+ `list_tabs_for(name)`由来のtabs/panes数（list sessionsのPANES列と同一取得経路・floating込みpane数）。dead対象はlist_tabs_forを呼ばずtabs/panes=null固定（EXITED名へのaction系呼び出しの副作用が未検証のため。呼んで失敗→nullでなく呼ばない）。running対象の取得失敗もnull。`--me`はplan行/JSONへcurrent session注記
  - **postcondition**: 全target実行後にlist_sessionsを1回取得し全対象行（RUNNING/EXITED問わず）の消失を確認。残存はper-target失敗（`session 'X' still listed after removal`）へfold（VerificationFailed不使用・全体はexit 5/6）。一覧取得失敗は元class/exitを保持しmessageへ文脈付与（`removed session(s) ... but failed to confirm: <元message>`。add session DD-14と同一pattern）
  - **dispatch**: 例外dispatch（DD-14）でprobe backendから呼ぶ（0-session環境で事前確認まで到達）
  - 報告: human `removed session 'foo'` / `'X' (was dead)` / `'X' (current session)`（--me）/ `FAILED <name> : <msg>` 行。JSON `data.results[]`（target=session名）。error/exit対応の詳細は設計書§2.9表

## DD-12. Safety model

- preflight: 全mutating verbで (1) version判定 (2) 対象解決 (3) plan生成 の順。remapはこれに (4) companion setup（移動が必要な場合のみ。wasm extract・permissions seed。DD-10.3/10.4）が続き、いずれも失敗は状態変更前に停止
- snapshot: remap / bulk remove は実行前に `list-panes -a --json` と `dump-layout` を取得し、JSON出力時に`_snapshot`として添付可能（障害診断用。dry-run表示にも利用）
- dry-run: v1は `remap` / `remove` が対応（remove pane/tabは複数対象・`--empty`、remove sessionはselector種別・対象数によらず常時。DD-11 remove session）。planのみ出力しbackendのmutating呼び出しを一切行わない。remapのdry-runはcompanion setup（wasm extract・permissions.kdl書換）も行わない（DD-10.10）。floating保有scopeのremap dry-runは警告+仮想embed計画表示で緩和する（DD-10.5/10.10・TASK-81。実行はpreflight errorのまま）。`resize equalize` / `add` のdry-runはP1（要件3.7はSHOULD。MR-7で確定）
- 確認gate: `remove pane/tab`は2対象以上または`--empty`のみgate。**`remove session`は対象数・selector種別によらず常にgate**（単一NAME指定でも`--yes`/`--dry-run`必須。session削除は最小構成でも全pane・tab・resurrection dataを不可逆に巻き込むため。TASK-84・DD-11 remove session）
- atomicity: 主張しない。部分失敗時は (a) 実行済み操作の一覧 (b) 未実行の一覧 (c) 推奨復旧操作（例: 再度remap、またはdump-layoutからの再構成）を報告
- remap実行中失敗の扱い: 移動・layout適用のどこで失敗しても、既に適用済みの変更は保持（戻さない）。失敗手順以降を未実行として報告（DD-10.11）。resurrection（session serialize）が保険として機能する旨を文書化
- **自spawn複製paneのclose（TASK-81）**: 要件2.6 kill/recreate禁止（L93相当項）の適用対象は「remap実行開始前のlist-panesに存在したpane」であり、**zelperが当該実行内にrun一致不成立でspawnした複製pane（実行開始前snapshotに存在しないid・安定観測後・対応規則〔exited/held化したdupを含む。DD-10.7 step 6多段規則〕でdupと特定されたもの）のcloseは自己副作用の除去としてkill制約の対象外**と定義する。closeしたpane id・対応slot・理由は成功・失敗双方のJSON data（`closed_duplicates[]`）とhuman出力で報告する（報告義務。DD-10.9）
- **move（DD-13・TASK-88）**: dry-run対象外（単一対象の即時操作であり計画表示の意味が薄い。`remap --dry-run`で全体計画を確認してからmoveする運用）。pane交換（`--swap-with`）の**atomicityは主張しない**（2 pipeの間で失敗しうる。順序規則によるtab自動closeの構造回避 + 部分状態の正確な報告 + 手動完了hintで緩和。DD-13）。対象はtiled terminal paneのみ（floating/plugin paneは状態変更前にPreflight error。DD-13）
- 廃止optionのtest固定（設計レビューR6。TASK-43で即時削除に方針変更）: `--session-scope` / `--overflow nest|tabs` はv0.1.xで削除済み。指定はusage error（exit 2・clap経由のstderr出力。`--json`併用時もstdoutには何も出ない）であることと、clap helpに出現しないことをtestで固定する

## DD-13. `move` 操作設計（TASK-88・2026-09-25追加。TASK-87設計書が元。実測M1/M2/M8反映）

5操作を1 verb `move`に載せる（verb×noun構造維持。remapとの役割分担はDD-1.1）:

```text
zelper move pane PANESPEC (left|right|up|down|rotate) [--backwards]   # native wrap（方向移動・rotate）
zelper move pane PANESPEC --to-tab TABSPEC                            # tab間移動（companion break-id）
zelper move pane PANESPEC --swap-with PANESPEC                        # pane交換（companion break-id ×2）
zelper move tab TABSPEC --swap-with TABSPEC                           # tab交換（native move-tab合成）
```

- **排他規則（DD-1.5）**: action positional / `--to-tab` / `--swap-with` は三者相互排他かつちょうど1つ必須（clap conflicts + app層のexactly one検証〔LayoutSpec::validate_exclusiveと同一pattern〕）。`--backwards`はaction=rotateとのみ併用可（runtime検証）。`--swap-with`の相手と同一対象（自分自身）はusage error
- **対象はtiled terminal paneのみ（本節で確定した規則）**: floating paneはM1実測で方向移動がrc=0の座標shiftとして動作し（slot swapでない）rotateがno-opになるため、companion系と同一に**全経路で状態変更前にPreflight error（exit 7）**とし`remap --embed-floating`併用（tiled化してから移動）をhintする。plugin paneはpipe経路で対象を特定できないため同じくPreflight error。exited/held pane・non-selectable paneも`is_remap_source`相当の単一pane版検査で事前遮断する
- **native系（方向移動・rotate）**: argvは`move-pane <dir> -p <spec>`（direction positionalが先。`-p`で非focus paneを移動可・focusは元paneに追従しzeller側のfocus制御は不要）/ `move-pane -p <spec>`（bare = rotate forwards）/ `move-pane-backwards -p <spec>`。M1実測: `move-pane`は**slot幾何完全不変のpane id付け替えswap**で、edge方向移動はrc=0・出力空の**silent no-op**。そのためexit codeのみでは成否判別不能であり、postconditionとしてlist-panesを再取得して対象paneの幾何を比較し、不変ならno-op warning報告 + exit 0（resizeのno-op報告規約に準拠）。pane id・title不変はM8実測基準（title不変も検証）
- **tab間移動（`--to-tab`）**: 対象・宛先（`selector::resolve_tab`共通解決）解決 → preflight → **宛先==現tabならpipeなしwarning付きexit 0（no-op）** → companion setup + probe（`src/companion.rs`の共用helper。不成立は状態変更前にOperationFailed + 権限hint） → `break-id <宛先tab_id> <単一pane_id>` pipe + list-panes polling（250ms/10s。完了条件 = pane.tab_id == 宛先） → postcondition（pane id不変。M8基準）。polling timeoutはOperationFailed（権限hint + pipe遅延実行の注意 + 未移動報告。remap 10.7と同一文面規則）
- **pane交換（`--swap-with`）**: 2 pane解決 → preflight → **同一tab内はPreflight error**（break系はtab間移動のみで、tab内位置交換の機構がzellijに存在しない。`remap`をhint） → **順序規則**（移動元tab自動close〔E6事実1〕の構造回避。singleton判定は当該tabのselectable pane数==1基準）:
  - 両tab非singleton → A→T_Bが先（決定論的tie-break）
  - Aのみsingleton → B→T_Aが先（T_AはBの到着後も空にならない）
  - Bのみsingleton → A→T_Bが先（対称）
  - **両singleton → Preflight error（exit 7）**: どちらのtabも閉じて再作成が必要になりtab位置・idを保存できない。hintは`move tab --swap-with`（tab交換。**pane内容の位置関係は入れ替わるが、tab名・tab idの帰属は入れ替わらない点がpane交換と異なる**）か`remap`
  - 各stepは`break-id` pipe + polling（相手の元tab idへの到着）で追跡
  - **途中失敗は移動済み/未移動の正確な報告 + 手動完了hint**（`zelper move pane <B> --to-tab <T_A>`）+ pipe遅延実行の注意。**atomicityは主張しない**（DD-12準拠）
  - postcondition: 両pane id不変・互いの元tabへ所属（不一致はVerificationFailed）
- **tab交換（`move tab --swap-with`）**: 2 tab解決（同一指定はusage error）→ position取得（i<j・d=j-i）→ **隣接transposition合成**: 左側tab Aを`move-tab right`×d、相手Bを`move-tab left`×(d-1)の計**2d-1回**の`move_tab -t`呼び出しで純position交換（M2実測「`move-tab -t`は1回で非active tabをfocus不変で1 position移動・隣接とは1回で純swap」からの導出。各呼び出しはedgeを越えないためwrap〔全体rotation〕は不発火。d=1はright×1の1回）。step毎にlist-tabsでid存在確認（id基準追跡）。`-t`がfocus不変のためgo-to-tab併用・focus復帰は不要（M2確定）。postcondition: position交換・tab id/name不変をlist-tabsのid追跡で検証（不一致はVerificationFailed・期待/実測差分付き）
- **error/exit対応**（新classなし・全verb既存11 class）: 構文違反（三者排他・`--backwards`単独・自己swap）はUsage 2 / pane不在・tab不在・曖昧はNoTarget・AmbiguousTarget 3 / floating・plugin pane・同一tab swap・両singleton swap・companion setup失敗はPreflight 7 / zellij非zero・probe不成立・polling timeout・move-tab呼出失敗はOperationFailed 5 / 完了後検証不一致（pane id不変違反・position不一致）はVerificationFailed 7
- **JSON/human報告**（DD-4.2/4.3の変更系1行サマリ規約）: `moved terminal_3 left` / `rotated terminal_3 backwards` / `moved terminal_3 -> tab 5 (agents)` / `swapped terminal_3 <-> terminal_4` / `swapped tab 2 <-> tab 5`。no-opはwarning行 + exit 0。JSON dataは各操作の対象・結果（swap-paneは`panes[]`・swap-tabは`tabs[]`の配列）
- **実測記録**（podman sandbox・zellij 0.44.3・2026-09-25/26）: M1（move-pane系。`tmp/tasks/tm-88/m1_result.log`・`m1c_result.log`）・M2（move-tab。`m2_result.log`・`m2b_result.log`）・M8（pane id/title不変。`m8_result.log`）・M9（E2E acceptance。`m9_result.log`）・M9b（selectable pluginのみ残るtabの自動close観察。`m9_result.log`追記分——S5〔non-selectable compact-barのみ→close〕との対で自動closeのselectable不在基準を確定）

## DD-14. `attach` / `detach` / `add session` / `remove session` / `rename session` のdispatch（session lifecycle。TASK-89・2026-09-25追加。TASK-87設計書が元。remove sessionはTASK-84・2026-11-13追加。rename sessionはTASK-95・2026-11-13追加。add sessionのNAME事前Usage検査はTASK-96・2026-11-14追加。rename sessionのNEW_NAME先頭`-`検査はTASK-97・2026-11-14追加）

- grammar: `zelper [--session NAME] attach [--json]` / `zelper [--session NAME] detach [--json]` / `zelper add session NAME [--json]` / `zelper remove session ... [--json]`（TASK-84。詳細はDD-11 remove session）/ `zelper rename session NAME NEW_NAME [--json]`（TASK-95。詳細はDD-8 rename session）。attach/detachのsession解決は--session共通（DD-1.2）。**対象session名を扱い対象session解決（mk_backend）を必要としない操作（`list sessions`・`add session`・`remove session`・`rename session`）は例外dispatch**: 対象session解決（mk_backend）を経ずprobe backend（`ZellijCliBackend::new("zelper-probe-unused")` + check_capability）を直接構築する（sessionが1つも存在しない環境——add sessionの主要ユースケース・remove/rename sessionの事前確認到達——でも動作するため。共通経路のまま実装するとresolve_sessionがNoTargetで失敗する）
- **attach**: TTY検証（`std::io::IsTerminal`でstdin・stdout両方）をzellij起動前にfail-fast（非TTYは`Preflight` exit 7。誘導文「attach requires a TTY; for non-interactive session creation use `zelper add session`」）。M4実測: 全fd非TTYの素attachはzellijがpanic exit 101・stdout汚染、stdin=TTY/stdout=pipeではUI出力先不在でblockするため、それらに到達させる前に止める（stdout片側のみ非TTYの構成もblock防止のため拒否）。backend経路は`attach_session()` = `run_interactive(["attach", <session>])`（positional形式。DD-3.1例外——実行中の子fd 2はcapture file・fd 0/1は元targetを継承・stderrは子終了後に一括転写で対話中の即時転送なし・実行中は親のSIGINTを一時SIG_IGN）。対話終了後のみ報告（対話中は一切出力しない。stdoutはzellijが占有）: human `attach session 'X' ended` / JSON `data.session`。zellij非zero終了は`OperationFailed` exit 5（messageへexit statusとstderr本文を埋め込む）。**不在名はzelper側で事前検出（TASK-93）**: session解決（DD-1.2）の--session明示時は`list_sessions`による事前存在確認を先行し、不在はNoTarget exit 3（zelper文面+candidates。EXITED名は存在扱いでpass-through）でzellij呼出前に失敗する——zellij側rc=1「Session not Found」経路（Session not found + 稼働session候補表示）には原則到達しない。0個・不在名時のNoTargetはTTY検査（`Preflight` exit 7）より先に立つ（mk_backend_namedによるsession解決がTTY検査より先のため）
- **detach**: `--session NAME action detach` wrap（現行run_actionの枠・新機構なし）。M3実測: 単一client存在時は外部detachでclient即切断・client exit 0・sessionはRUNNING継続・detach自身はrc=0 silent。**client不在でもrc=0 silent（no-op成功）**のため、zelperはclient数を検査せずrc=0を無条件で成功扱いする（client不在とclient存在がzellerから同一に見える）。報告: human `detached clients from session 'X'` / JSON `data.session`。不在session名はsession解決（DD-1.2）の事前存在確認でNoTarget exit 3（zelper文面+candidates。zellij呼出不発。TASK-93）。それ以外のzellij非zero終了は`OperationFailed` exit 5
  - 注記（M3 (c)実測）: **複数attach client存在時のdetachはzellijが最新のattach clientのみdetachし、先発clientは描画凍結のまま残留する**（2回目のdetachでも先発clientは残留——反復detachは全client切断の手段として機能しない）。zellij側のclient選択挙動でありzelperにclient数依存の分岐はなく、rc=0を成功として報告するのみ
- **add session**: 事前Usage検査（TASK-96。設計書: `tmp/tasks/tm-96/design_add-session-name.md`）——app層run_session冒頭（一覧取得より前）でNAMEの空文字・'/'含み・空白含み・先頭`-`の4検査を行い、違反は`Usage` exit 2（文面は`NAME must not be empty` / `NAME must not contain '/'` / `NAME must not contain whitespace` / `NAME must not start with '-'`の検査順。app層検査のためerror envelope経由でclapのparse error形式ではない）。空白含みNAMEはzellij自体は受理する（M96-A）が、TASK-98（DD-3.4）のname完全復元後も改行含み名（行分割で復元不能。実機作成可否未測定）が残存するため空白検査（`contains(char::is_whitespace)`）は当該classへの現存防衛として維持する——既存契約の維持・DD-8 rename session NEW_NAMEとの対称性・緩和のコスト（Usage条件反転4件・空白含み名lifecycle条件新設・入力側検査設計）に対し便益（zeller経由の空白含み名作成という限分的需要）が見合わない（TASK-98設計§7の再anchor。緩和は両verb同時にのみ行うfuture option）。先頭`-`は`--`option終端経由（`add session -- -x`）がCLI解析を通過して`attach -b -x`まで到達するため（未知optionはzellij側clapがrc=2で拒むが、有効option一致値の下流挙動は未測定でzellij側option集合はzeller管理外のためzellij委ねは契約保証不能。先頭`-`名はzellij CLI経路で作成不能な系列）。空文字・'/'含みはzellij作成系がrc=1+stderrで明示拒否する（M96-B/C。rename系のrc=0黙失敗〔M95-B/C1/C2〕と対照）ためzeller単独でも安全だが、rename NEW_NAMEとの対称性・fail-fast（capability検査通過後の一覧取得・作成呼出を省く。zellij不在・非対応版ではUsageより`ZellijUnavailable` exit 4が優先——環境検査が先の既存順序）のためwrap側でも事前検査する（安全性でなくAPI整合性の付け足し）→ 事前確認（list_sessionsに同名running存在 → `Preflight` exit 7「session already exists」。作成を試みないfail-fast。EXITED行はrunningとみなさずzellijへ委ねる。0-session環境はbackendのlist-sessions 0件decode（DD-3.1・TASK-93）により空一覧として扱い作成へ進む——同名衝突はzellij本体のattach -bがrc=1「Session already exists」で拒むため安全側。一覧取得の失敗は0件へ丸めずclass/exitを保持して伝播する（`ZellijUnavailable`等も含む。事前確認の失敗なら作成呼出〔attach -b〕は不発））→ `create_session_detached(name)`（argvは `attach -b NAME`。**global --session形式は禁止**: M5 run2実測でrandom別名sessionを作るため。positional形式必須）→ postcondition: list_sessionsにNAMEがrunning出現（DD-11 addの存在確認規則に準拠。作成成否をexit codeのみで信用しない。不在は`VerificationFailed` exit 7。事後一覧取得の失敗もclass/exitを保持して伝播し、messageへ文脈を付与——`created session 'NAME' but failed to confirm: <元message>`。作成呼出が成功済みの可能性があるため）→ 報告: human `created session 'X' (detached)` / JSON `data.session`・`data.created=true`。作成呼出の非zero終了（EXITED同名衝突・race等）は`OperationFailed` exit 5（EXITED同名の実際の挙動は実測不能〔M5 run2/run3〕のためzellij委ね・結果報告の設計）
  - 作成直後のsessionは空skelton（tab 1・pane 0件。M5実測）で、**first attach時にdefault layoutが具現化する**。v1は素の作成のみ（layout等の作成optionなし——`attach -b`にlayout引数が存在しない。M5 (0)実測）。layout適用は作成後の`remap`合成（`zelper add session work && zelper --session work remap three`）で担う
  - **add session×remap合成の注意**: 作成直後のpane 0件・空sessionに対するremapになる（remapのM=0系条件——`remap-sequence.new-tab-only-needs-no-companion`（M=0・k=1・companion不使用）・`layout-planner.r7-empty-session-all-slots-empty`（M=0: k=1・全tab全slot空）——のとおり、k=1・全slotが既定shellの適用になる）。first attach後の具現化paneをsourceにしたい場合はattach後にremapすること
- dry-run対象外（DD-12）。attach/detach/add sessionともに状態変更前のfail-fast（TTY検証・同名事前確認）を除けば単一backend呼び出しで、snapshotは取得しない

## DD-15. `watch` 操作設計（TASK-86・2026-09-25追加。TASK-87設計書が元）

- grammar: `zelper watch [PANESPEC...] [--tab T] [--name N] [--command C] [--cwd D] [--all] [--regex] [--full] [--interval SECS] [--max N] [--json] -- PATTERN`。PATTERNは`--`以降（`last = true`。sendの`-- TEXT`規則の流用）。**`--`分離は必須**（PANESPEC positionalとPATTERNが「数字1つ」等で衝突するため）で、v1はPATTERN数==1厳格・空pattern拒否（usage error, exit 2）。将来`-- PATTERN...`（any-match拡張）へwidenできるようVecで受ける
- request: 対象（FilterArgs・read/sendと同一形・複数pane可・any-hit）+ PATTERN + 照合形式（`--regex`。大文字小文字無視は`(?i)`表現のみで`-i` flagは設けない）+ `--interval SECS`（f64・default 1.0・**1.0未満と非有限〔NaN/inf〕はusage error**）+ `--max N`（u32・default 30・最大試行回数。1以上）+ `--full`（dump-screen -f。readと同名同義）
- 引数検証（違反はUsage exit 2。対象解決・polling開始より前）: pattern数==1 / pattern非空（空substringはcontains常真・空regexは全行matchのため即時偽hitになる） / interval >= 1.0かつ有限・かつ`Duration`へ変換可能（巨大有限値〔例: 1e30〕も拒否。REVIEW-1） / max >= 1 / `--regex`時の`regex::Regex::new`（compile error文を含む）。app層引数検証はbackend初期化（version確認・session解決）の後に実行されるため、zellij不在時はexit 4（`ZellijUnavailable`）が引数不正（exit 2）に優先する（既存verbと同一の共通backend経路に従う）
- 実行: polling loop（`dump-screen`反復。backend/trait無変更・loop実装はapp/watch.rs内部に閉じる。subscribeはv2差し替え候補として余地のみ）
  - 対象解決は開始時に1回（`list_panes` → build_spec → `selector::resolve` → TargetSet。0件はNoTarget exit 3）。毎poll再解決しない
  - attempt 1はt=0即時実行（sleepなし。既出テキスト即hit。`--full`時はscrollback込みの既出もhit）
  - 照合は`dump-screen`生テキストの**行単位**: substring（既定）は行へのcontains、`--regex`は行への`regex.is_match`。ANSI正規化は不導入（M7実測〔2026-09-25・zellij 0.44.3・podman sandbox〕でdump-screen出力へのescape混入なし。導入条件を満たさないため。read出力への同一正規化はもともと別変更・範囲外）
  - 対象paneはTargetSetのvisual order順に照合し、hitありで即終了。**hit paneはvisual order順の最初**（同一attempt複数hitの決定論的tie-break）
  - attempt < maxの間、前poll終了後に`sleep(interval)`（drift許容）。最終attempt後はsleepなし
  - `dump_screen`失敗（pane消滅等）のpaneは以後のattemptから除外し`pane_errors`へ記録（他paneのpollingは継続・生存paneでのhit優先）。中断条件はhit/exhaustのみ（SIGINT/Ctrl+Cはdefault終了・進捗表示なし）
- postcondition: なし（読み取り専用操作の反復）
- 失敗: 回数切れ（no hit・pane障害なし）と全pane障害は`OperationFailed`（exit 5）。一部pane障害を伴う回数切れは`PartialFailure`（exit 6）
- JSON: hit時の`data`は7 field: `{ target, pattern, regex, attempts_used, elapsed_secs, matched_lines, pane_errors }`（envelopeはDD-4.2共通）。回数切れの`error.data`は`{ attempts_used, elapsed_secs, pattern }`（PartialFailure時は失敗paneのみの`results[]`〔TargetedResult〕）
- human: readと同型のpane header（`=== terminal_3 (title, tab:N name) ===`）+ match行すべて（+ pane_errorsがある場合はFAILED行）
- **DD-4.2一般規定に対する例外（明示）**: watchのhit報告は`data.results[]`でなく**hit pane単一のobject**である。multi-target操作（read/send/remove）が`data.results[]`にper-target結果を格納するDD-4.2の一般規定に対し、watchは「最初にhitした1 paneのみが結果」（any-hit契約・複数paneの同時報告はしない）のため単一objectとし、障害paneは`pane_errors`（hit時）/`error.data.results[]`（PartialFailure時）へ載せる
- 依存: `regex` crate（`--regex`時のみ使用。TASK-86で追加）

## 実装モジュール構成（basic-design §9を踏襲、DD-10 v2に伴い更新）

```text
plugin/                            # cargo workspace member zelper-companion-plugin（DD-10.3）
                                   # bin crate・zellij-tile =0.44.3・wasm32-wasip1 release buildを
                                   # zelperのbuild.rsがbinaryへ埋込
src/
  main.rs  cli/  app/{list,read,send,rename,resize,remap,add,remove,move_,attach,watch}.rs
                                   # move_（`move`はRust keywordのためfile名はmove_。DD-13）
  companion.rs                     # wasm extract・permissions.kdl seed（DD-10.3/10.4）+
                                   # runtime orchestration（companion setup + probe pipe・
                                   # polling定数・break-id payload・権限hint。remap/move共用。
                                   # DD-10.7 step 3・TASK-88設計§2.8）
  domain/  zellij/{process,parser,capabilities}.rs
  layout/{resolver,generator}.rs   # generator: KDL生成（改行区切り・slot正規化。DD-10.8）
  output/{human,json}.rs  error.rs
```

Rust library選定: `clap`（derive）+ `clap_complete` / `serde`+`serde_json` / `thiserror`（library境界）+ `anyhow`（binary境界） / `kdl` crate（layout KDLの最小parse: slot数カウント・末端pane列挙。生成は文字列組み立て） / `regex` crate（watch `--regex`照合。DD-15） / CLI testは`assert_cmd`+`predicates`。subprocessは`std::process::Command`で十分（async不要）。
