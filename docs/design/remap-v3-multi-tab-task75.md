# TASK-75 設計: remapのmulti-tab layout全体再現（DD-10 v2.2）

作成日: 2026-09-15
対象: TASK-75。goal: 「zelper remapにlayout file全体（全tab・tab名・focus属性・default_tab_template由来bar）をzellij --layout適用時と同一に展開する仕様を設計・実装し、sandbox E2Eでzellij --layout <file>起動時と同一のtab構造・形状が再現されることを検証すること。TASK-74の不具合修正（単tab 3x3収容）とは分離した仕様拡張」
規約上の位置づけ: 本fileはTASK-75の設計記録（仕様確定の根拠・data構造・変更範囲・検証計画・E2E acceptance手順・実験記録）。規範的な仕様変更は`detailed-design.md` DD-10本体へv2.2改訂として反映（§6に対応表）。要件の拡張は`first/260821/requirements.md` §2.6へ反映。

## 1. 背景とgoal・TASK-74との境界

- 現行（DD-10 v2.1）の反復単位は「layout最初のtab subtree」（`base_subtree`・slot数N）。multi-tab layoutを指定した場合、最初のtab形状のみが反復され、2番目以降のtab・tab名・tab focus・tab毎のpane focusは失われる
- TASK-74は「単tab適用の正しさ」（run一致照合key・template bar保持・poll空応答retry）を対象とし、その設計書は「multi-tab layout全体再現（tab名・focus・template bar含む全体像）はTASK-75へ分離済み」と明記して本taskへ引き継いだ。本設計はその分離対象を扱う。TASK-74の成果（terminal_command照合・children置換・空応答lenient化）は前提として再利用する
- 本設計のgoal: 反復単位をlayout全体へ一般化し、`zellij --layout <file>`起動時と同一の「tab構造（T tab）・per-tab形状・tab名・tab focus・pane focus・default_tab_template由来bar（全tab）」をremapで再現する。判定はsandbox E2E（§5.2）で行う

## 2. 仕様確定（ユーザー承認済み決定の条文化）

確定済み決定（2026-09-15時点・再議不要）を仕様として条文化する。決定番号D1〜D11は本節の参照用。

### 2.1 正規形TabTemplate列（D2・D8・D1）

**layout解決の出力を「最初のtab subtree」から「正規形TabTemplate列」へ変更する**:

- layoutの各`tab` nodeを文書順に列挙し、各tabについて鋳型`TabTemplate`を構築する:
  - **subtree**: layoutに`default_tab_template`がある場合、TASK-74のchildren置換規則（template subtreeの文書順最初の`children` nodeをtab subtreeのnodesで置換。`children` node不在・置換後のterminal leaf数がtab subtreeのslot数と不一致は`LayoutInvalid`）を**layout解決時点で**適用済みのsubtree。template無しlayoutではtab subtreeをそのまま
  - **name**: tab nodeの`name`属性（Option）。subtree本体からはtab属性を除去する
  - **focus**: tab nodeの`focus=true`属性（bool）
  - **n_slots**: subtreeの末端terminal pane slot数（plugin leaf除外・SKIP_NODES規則。現行`count_terminal_slots`と同一規則）
  - **pane_focus_slot**: subtree内で`focus=true`を持つ文書順最初のterminal pane leafのslot index（Option<usize>）。複数のfocus=true leafがある場合は文書順最初を採用しwarning（zellijの複数focus=true挙動は未検証のため、zelper側規則として文書順最初に確定。§4参照）
- tab nodeが1つもないlayout（`--inline 'layout { ... }'`・tab無しfile layout）は「1つの名無しtab鋳型」（subtree = layout直下。現行`base_subtree`相当）であり、T=1
- **正規形化への一元化（D8）**: TASK-74がgenerator引数としていたchildren置換・slot数一致検証は、正規形TabTemplate列の構築（layout解決）へ集約する。generatorは置換済みsubtreeのみを受け取り、template引数を持たない（§3.3）
- `new_tab_template`はSKIP_NODES維持・対象外（zellijが新規tab生成時に使う予約領域であり、remapの生成KDLへは載せない。dumpへの残存はE2E比較の除外項目。§5.2）。`pane_template`/`tab_template`もSKIP_NODES維持（v2.1と同一。§4）
- **anchor tab名保持（D1・ユーザー承認C2）**: 生成tab (0,0)（block 0・鋳型0）はanchor（現active tab）を再利用し、**名前を保持してrenameしない**。同一性検証（10.9・E2E acceptance）においてanchor tab名はlayout最初のtab名との一致検証から唯一除外する

### 2.2 配分則（D3・式）

- T = 鋳型数、N_t = 鋳型tのslot数（t=0..T-1）、**S = sum_t N_t**（layout全体の反復単位slot数）
- M = source pane数、**k（block数）= max(1, ceil(M/S))**
- 割当: source visual orderのpane index i（0開始）→ **block b = floor(i/S)**・block内offset **o = i%S** → 累積slot数 C_t = N_0 + ... + N_{t-1}（C_0 = 0）として、o in [C_t, C_t + N_t) を満たすtと**slot s = o - C_t**（visual order逐次充填）
- **生成tabは全block全tab（k × T）**。(b, t) = (0, 0) はanchor。それ以外は新規tab
- 空slot: 最終block（b = k-1）の各tabでのみ発生（割当pane数がN_tに満たないtab）。空slotは常にbare pane（既定shell。DD-10.2 #9の現行規則維持）
- **N_t = 0 tab（terminal slotを持たないtab鋳型・pluginのみのtab等）は`LayoutInvalid`**（v2.2。§4の判断根拠）
- M = 0: k = 1・全tab全slot空（anchor含む各tabに生成KDL適用のみ）

### 2.3 tab名生成規則（D3・D7）

生成tab (b, t) の名前:

- (b, t) = (0, 0): anchor名（renameしない。D1）
- それ以外: **幹 = T >= 2 かつ鋳型tがname属性を持つなら鋳型名、そうでなければbase**（base = layout名 / --pathのfile stem / `remap`（--inline）。現行定義）。**b >= 1 なら幹に`-<b+1>`を接尾**
- 名無し鋳型はzellij自動名（「Tab #N」）に依存せずzelperが命名する。zellij --layout起動時の自動名採番はCLIから制御・予測できないため、自動名の再現は対象外とする（10.13へ明記。§4参照）

### 2.4 tab focus・pane focusの再現（D4・D5）

- **tab focus**: layoutに`focus=true`の鋳型があれば、文書順最初のfocus=true鋳型 t* について **block 0 の tab (0, t*) を最終go-to先**とする。無ければanchor復帰（現行）。focus復帰はbest-effort維持（復帰失敗は操作失敗にしない）。**複数のfocus=true鋳型がある場合は文書順最初を採用しwarningを出す**（§5.2 (v)系列b。zellijはdump上常にactive tabにfocus=trueを付けるため複数指定を機械的に判別できず、zelper側規則として文書順最初に確定し採用を利用者へ通知する。2026-09-16 E2E改修で明文化）
- **pane focus（D5・本taskで新設。E2E acceptance後の改訂: 対象を位置ベースへ）**: layout適用phaseで各生成tab訪問時（go-to-tab → override-layout の直後）に `zellij action focus-pane-id terminal_<id>` を実行し、tab毎のfocus paneを設定する:
  - 対象slot = 鋳型の`pane_focus_slot`（§2.1）があればそのslot、無ければ**slot 0**（reference実測: focus無指定時、各tabの最初のterminal paneがfocused。tmp/task75 hetero実験）
  - **対象paneの決定は「mappingのpane id」でなく「適用後の実状態の位置」で行う**（E2E要因A改訂。2026-09-16）: override-layout適用直後にlist-panes（lenient・空応答はpoll再試行）で当該tabのterminal paneをvisual order（geometry y,x 昇順）に並べ、**s番目（s = 対象slot番号）のpane id**へfocus-pane-idする。mapping slot sのpane idをそのまま使う設計は、bare pane群（同一terminal_command=null）のrun一致配置が割当順と一致しないため実幾何とずれる（acceptance b系列: mapping slot 0 = terminal_0 は実幾何(134,40)=3x3右下に配置され、幾何(0,0)のpaneがfocusedにならない。R75-4「pane idはlayout宣言順に振られない」の類型がbare pane配置でも発生）
  - 位置ベース決定のため**occupied slot限定（空slot対象除外）は撤廃**（E2E改訂）: 対象位置のpaneは空slot（bare shellの新規spawn pane）でも位置から特定できるため、全生成tabでfocus-pane-idを実行する。検証(d)も同じ位置基準で全生成tabを対象にする
  - focus-pane-idの呼出失敗はwarning（継続）。反映は検証(d)（is_focused経由）で検知する。ただしzellijが「Pane ... is already focused」でexit 2を返す場合はfocus状態が実際に成立しているため**成功扱いとする**（warningを出さない。acceptance b系列実測: tab1/tab2のalready focusedはis_focused==trueのまま合格）
- 実行順序は「go-to → override-layout → （list-panesで対象特定）→ focus-pane-id」。override-layout適用がfocus状態をリセットしうるため、最終操作をfocus設定とする

### 2.5 過剰tab（D6）

- 明示的なcloseは行わない。remap後もtargetsに含まれないtab（source外paneのみのtab・companion plugin host tab等）は**放置し、human出力のwarning行とJSONの`leftover_tabs`（id/name/position/selectable pane数）で情報報告する**。閉じる場合はremove verb（DD-11）を明示的に使う

### 2.6 生成KDLと適用経路（D7）

- 生成KDLは**現行per-tab形式を維持**: tab nodeなしの`layout { subtree }`形式（改行区切り・値quote必須・DD-3.3）を、生成tab (b, t) 毎に`override-layout --layout-string <KDL> --apply-only-to-active-tab --retain-existing-terminal-panes --retain-existing-plugin-panes`で個別適用する
- tab属性（name・focus）は生成KDLに載せず、**rename-tab-by-id・go-to-tab（最終go-to・focus-pane-id）で再現**する（tab nodeなし形式のため。D7）
- slotへのrun注入規則（terminal_command基準・bare正規化・plugin leaf保存・bar leaf維持）はv2.1のまま変更なし。baseは当該tabの正規形subtree（template反映済み）を用いる

### 2.7 T=1後方互換（割当・k・命名・tab focus復帰の式レベル一致。D9・証明）

**保証範囲の名称確定（TR75-3）**: 本保証は「計画レベルの一致」——割当式・k式・命名・tab focus復帰先の4項目がv2.1実装と式レベルで一致すること——であり、**実行操作列全体の一致ではない**（pane focus再現のfocus-pane-id呼出はT=1でも新たに実行されるため。下記）。

T=1（単tab layout・tab無しlayout・--inline）では、N_0 = N・S = N であり:

- 割当式: b = floor(i/S) = floor(i/N)（現行instance jと一致）・o = i%S = i%N で t=0（C_0=0・C_1=S=o の上限）、s = o = i%N（現行slot式と一致）
- k式: max(1, ceil(M/S)) = max(1, ceil(M/N))（現行と一致）
- 命名: T=1では幹は常にbase。b=0はanchor（現行 j=0 と一致）・b>=1は`<base>-<b+1>` = 現行`<base>-<j+1>`（厳密一致）
- 空slot: 最終blockのみ = 現行「最終instanceのみ」と一致
- tab focus復帰: focus=true鋳型は t*=0 しか存在せず、block 0 tab 0 = anchor。よってfocus=true指定の有無によらず最終go-to先はanchorとなり、現行のanchor復帰と同一の挙動になる

以上よりT=1では割当式・k式・命名・tab focus復帰の4項目がすべて現行と厳密一致する。この一致を回帰test（既存R1〜R7・R39系）で固定する。**pane focus再現（focus-pane-id）はD5によりT=1でも新たに実行される**（D9の列挙〔割当式・k式・命名・focus復帰〕には含まれず、fake backend testの期待値にはfocus-pane-id呼出が加わる。破壊的変更としてCHANGELOGへ記載予定）

### 2.8 --tab指定時（D10）・pane name属性（D11）

- `--tab TABSPEC`指定時は、layoutの**最初のtab鋳型（鋳型0）による単tab適用**を維持する（S = N_0としてT=1経路でplan・適用。source・anchorはそのtab。DD-10.5へ明記）
- pane nodeの`name`属性→pane title（`pn-1`等）は**保証外・観察のみ**とする（実測ではtitleに反映されるが、remapはtitleを設定せずrun一致照合とも無関係。DD-10.13へ明記）

## 3. data構造・変更設計

### 3.1 TabTemplate型と正規形化（src/layout/mod.rs）

```rust
/// 正規形tab鋳型（DD-10.6 v2.2）。default_tab_template反映済み・tab属性保持
pub struct TabTemplate {
    pub name: Option<String>,       // tab nodeのname属性
    pub focus: bool,                // tab nodeのfocus=true
    pub subtree: KdlDocument,       // template反映済み・tab属性除去済み
    pub n_slots: usize,
    pub pane_focus_slot: Option<usize>,  // 文書順最初のfocus=true terminal leaf
}

/// layout docから正規形TabTemplate列を構築（tab無しlayoutは1要素）
pub fn normalize_tab_templates(doc: &KdlDocument) -> Result<Vec<TabTemplate>, ZelperError>
```

- `normalize_tab_templates`が`base_subtree`・`default_tab_template_subtree`を置換統合する（両関数は削除し、呼び出し側〔remap.rs・tests〕は正規形化経路へ移行）。children置換（generator.rs `replace_children_marker`）とslot数一致検証もlayout解決へ移動する
- pane_focus_slot抽出: `walk_slots`でfocus=trueを持つ文書順最初のterminal leaf（plugin leaf除外）のslot index。複数個あればwarning用の情報を返す（planner warningsへ接続）

### 3.2 plan一般化（src/app/remap.rs・型とplanner）

```rust
pub struct V2Plan {
    pub tabs: Vec<TabTemplate>,   // T鋳型（plannerはname/focus/n_slots/pane_focus_slotのみ使用）
    pub s_slots: usize,           // S
    pub m: usize,
    pub k: usize,                 // block数 = max(1, ceil(M/S))
    pub anchor: TabId,
    pub base: String,
    pub blocks: Vec<V2BlockPlan>, // k個
    pub warnings: Vec<String>,
}
pub struct V2BlockPlan { pub index: usize, pub groups: Vec<V2TabPlan> }  // groups長T
pub struct V2TabPlan {
    pub block: usize, pub tab: usize,          // (b, t)
    pub name: Option<String>,                  // 生成tab名。（0,0)はNone（anchor保持）
    pub assignments: Vec<V2Assignment>,        // slot割当（V2Assignmentは不変: slot/pane/run）
    pub empty_slots: usize,                    // N_t - 割当数
    pub pane_focus_slot: usize,                // focus-pane-id対象slot番号（pane_focus_slot.unwrap_or(0)。
                                               //  E2E要因A改訂: 対象pane idは計画時でなく実行時に
                                               //  適用後のlist-panes位置から決定する）
    pub tab_focus_target: bool,                // 最終go-to先（block 0のt*のみtrue）
}
```

- `plan_v2`のsignature: `plan_v2(source, templates: &[TabTemplate], anchor, base)`（純粋関数の維持。fake backend不要）。N_t=0鋳型は`LayoutInvalid`（§2.2）
- 割当実装: §2.2の累積slot式。空group（割当0件）もV2TabPlanとして生成（§3.4のnew-tab --layout-string経路で具体化）
- 命名規則: §2.3。`(0,0)`はname=None

### 3.3 生成KDL（src/layout/generator.rs）

- `generate_instance_kdl_v2(base: &KdlDocument, runs: &[SlotRun]) -> Result<String, ZelperError>`へ**template引数を削除**（正規形subtreeは置換済みのため）。run注入・bare正規化・plugin leaf維持・bare plugin除去（layout/tab直下）規則は不変
- `instance_kdls`（remap.rs）は`Vec<Vec<String>>`（blocks × tabs）を返すよう一般化

### 3.4 移動系列の一般化（src/app/remap.rs `execute`）

group = (block b, tab t)。順序と手段:

- **5-a（不変条件の維持）**: group (0,0)（anchor group）のanchor外paneを`break-id <anchor_id> <ids>`でanchorへ。M >= 1ならgroup (0,0)は常に1 pane以上を持ち（§2.2逐次充填により割当は必ずslot 0側から詰まる）、5-aのinboundを後続outboundより先に行うことでanchor自動closeを構造的に排除する（現行DD-10.7 5-a不変条件の一般化）
- **5-b**: block b昇順・tab t昇順で (0,0) 以外の各groupを処理:
  - **割当pane 1件以上のgroup**: 現行どおり`break-new <ids>`（fire-and-forget）→ membership poll（tabのselectable tiled terminal pane集合 == group・pane id基準。tab id再利用耐性）→ `rename-tab-by-id <target> <生成tab名>`
  - **割当pane 0件のgroup（空group）**: `new-tab --layout-string <生成KDL_{b,t}>`（tab作成時点で生成KDLを適用）→ stdoutのtab id parse（DD-3.2/11の実績経路）→ `rename-tab-by-id <id> <生成tab名>`。**E2E要因B改訂（2026-09-16・実験4）**: 旧経路（new-tab layout引数なし → go-to → override-layoutで既定paneを正規化）は、既定pane（bare shell・invoked_with=None）がoverride-layoutのbare slotに照合されず残存しN_t+1 paneで検証(b)がfailするため不採用（U75-10解決: bare slotはtab作成経路でのみ具体化され、override-layoutは既存paneとrun一致しない限り常に新規spawnする）。`new-tab --layout-string`なら全slotがtab作成時に具体化されpane数==N_tが成立（実験4実測: 1-slot+bar生成KDLでterminal 1 pane+compact-bar・2-slot+bar生成KDLで左右2分割2 pane・stdoutにtab id）。DD-10.2 #7の却下理由（2呼び出し・id受け渡しrace）は空groupでは問題にならない（renameのためのid受け渡しは旧候補経路でも同一・当該tabをcloseしないためid再利用の競合源もない）。対抗案「new-tab（引数なし）後にclose-paneで既定paneを閉じる」は不成立（実験4: 最終paneのcloseでtab自体が自動closeされる）
- **step 6（layout適用phase）**: (b, t) 全groupをblock昇順・tab昇順で: `go-to-tab <target>` → `override-layout`（生成KDL_{b,t}・retain 2 flag・apply-only-to-active-tab）— **ただし空groupはnew-tab --layout-stringで作成時点で生成KDL適用済みのためoverride-layoutをskip**（apply済みlayoutの再適用は不要。実験4で作成時点の具体化を確認）→ **`focus-pane-id terminal_<id>`**（§2.4: 適用後のlist-panesで当該tabのterminal paneをvisual orderに並べs番目のpane idへ。呼出失敗はwarning・already focusedは成功扱い）
- **step 7**: 検証（§3.6）→ 最終go-to（§2.4: tab focus指定があれば targets[t*]〔block 0〕・無ければanchor。best-effort）
- move_needed判定の一般化: k > 1 または group (0,0) にanchor外paneが存在、**または新規tab（k*T - 1 > 0）が存在**。空groupのみで新規tabを作る場合もcompanion pluginは不要（break系pipe不使用）のため、probe要否は「break系を使用するか」（k > 1 または group(0,0) outbound非空）に分離する
- **targets配列のdata flow（TR75-4）**: `targets: Vec<TabId>`（長さ k*T・block昇順tab昇順。targets[b*T+t]）をexecute単一実行scope内で保持する（現行実装と同じ運用）。DD-2「長寿命のtab id参照を持たない」原則の適用範囲は**実行を超えた保持（field・global・file残置）の禁止**であり、単一実行scope内の逐次参照は現行から許容される——この解釈をDD-10.7末尾へ明記した。tab id再利用（不安定キー）への実行内防御として、**step 6の各group処理直前にlist-tabs（lenient）でtargets[j]の存在確認を行い、不在なら再解決**する:
  - 割当ありgroup: pane所属基準（5-b membership特定と同一。「groupの全paneが同一tabに所属し、かつ当該tabのselectable tiled terminal pane集合 == group」）
  - 空group: rename済み生成tab名基準（list-tabsのname一致。§2.3命名によりblock 0内は鋳型名・b>=1は`-<b+1>`接尾付きで特定可能。go-to-tab-nameコマンド〔tmp/task75/20260915_doc_task75_go_to_tab_name_help.txt〕は同名tab曖昧性のため補助にしか使わない）
  - new-tab由来idはstdout parse直後のrenameで「取得直後に消費」し、以降はtargets配列参照のみ
  - 再解決不能はOperationFailed（中断時報告）。自己作成tabは自実行内にcloseされない限りid再利用の競合sourceが存在しないため、この保持・確認で十分
- **空groupのみの実行（M=0・source全てanchor内で新規tabが空groupのみ）のpreflight depth（TR75-5）**: companion plugin・probe不使用（break系pipe 0件）。preflightは他caseと同一depth——layout解決・N_t=0検証（LayoutInvalid）・正規形化children置換検証（LayoutInvalid）・plan・生成KDL計算——を**状態変更前に完了**し、new-tab実行時点に未知の検証を残さない。new-tabの失敗（非zero exit・stdout parse不能）はOperationFailedで中断（部分状態報告）。この経路の実証（tab id parse・既定paneが生成KDL適用でN_tへ正規化）はfake/E2Eの**必須条件**とする（§5.1・§5.2 (iii)）

### 3.5 focus-pane-id backend追加（src/zellij/mod.rs・process.rs）

```rust
fn focus_pane(&self, pane: &PaneKindId) -> Result<()>;  // action focus-pane-id <PANE_ID>
```

- argv: `zellij --session <NAME> action focus-pane-id terminal_3`（PANE_ID形式は`PaneKindId::as_spec()`。実測help: tmp/task75/20260915_doc_task75_focus_pane_id_help.txt）
- **責務分担（TR75-10）**:
  - backend（process.rs実装・trait）は**argv組み立てと`Result`を返す薄い実装のみ**（warning化・retryなし。現行の他actionと同一構成）。`PaneKindId`の系列分離（terminal_N / plugin_N）は`as_spec()`が担い、backendは分岐を持たない。zellij仕様としてはplugin_N・bare Nも受理するが、**remapのfocus対象はterminal slotのみ**（§2.4）であり、zelper経路でのplugin paneへのfocus呼出は設計上存在しない
  - `Err`のwarning化（実行継続・検証(d)で検知）は**execute側の責務**（§2.4・§3.4）。fake.rsも同型の薄い実装+呼出記録とする

### 3.6 verify拡張（src/app/remap.rs `verify`）

- (a) 全source pane idの生存と割当groupのtab所属（pane id基準。現行維持・group参照をinstance→(b,t)へ）
- (b) **各生成tab (b,t) のselectable tiled terminal pane数 == N_t**（tab毎のslot数に一般化）
- (c) 生成tab名が§2.3どおり（(0,0)=anchorは名前検証から除外・D1）+ anchor残存
- (d) **pane focus（新設。E2E要因A改訂: 位置ベース）**: 各生成tab (b,t)で、当該tabのterminal paneをvisual order（geometry y,x 昇順）に並べた**s番目（s = pane_focus_slot.unwrap_or(0)）のpane**の`is_focused == true`（list-panes経由。非active tabもis_focusedを持つ〔実測〕）。step 6のfocus対象決定と同一の位置基準であり、全生成tabが対象（空slot tab除外は撤廃）
- **leftover_tabs報告**: tabs_afterのうちtargetsに含まれないtabを`leftover_tabs`（id/name/position/selectable tiled/floating pane数）として報告（closeしない。§2.5）。失敗条件にはしない
- 失敗時のerror.data: `m`/`n`（=S）/`k`/`t`（T）/`n_slots`（N_t列）+ mapping（pane → block/tab/slot/TabId）

### 3.7 dry-run/JSON出力拡張（DD-10.10）

- (b) M / S / k表示に加え**per-tab slot数列**（`N_t = 9,1,2`等）とT、(c) 割当表を pane → (block, tab, slot, tab名) へ、(d) KDL previewは生成tab (b,t) 毎、(e) 操作列へnew-tab（空group）・focus-pane-id・最終go-to先を追加
- JSON（`dry_run_json`・成功時data）: `m`/`k`は維持、**`n`にはSを格納**（T=1では従来値Nと同一）。新規に`t`・`s`・`n_slots`（N_t列）・`tabs`（鋳型のname/focus）を追加。mapping系のkeyへ`block`/`tab_index`追加（`instance`はblock値で維持）。DD-4.2のstable fields（`schema_version`/`ok`/`data`/`error.class`等）は不変であり、data内部のfield構成変更は契約変更にならない
- **preflight warningの伝達経路（2026-09-16 E2E改修で明文化）**: 複数focus=true鋳型/pane検出・quoting等のpreflight warningは、human実行・--json実行・dry-runの全経路で**stderrの`warning:`行**へ出力する（--json実行時もstdoutのenvelope契約を壊さない）。加えて機械可読経路としてdry-run/成功時のJSON dataへ**`warnings`（文字列列）**を格納する

### 3.8 変更対象file一覧

| file | 変更 |
|---|---|
| src/layout/mod.rs | `TabTemplate`型・`normalize_tab_templates`追加（children置換・slot数一致検証・tab属性抽出・pane_focus_slot抽出を一元化）。`base_subtree`/`default_tab_template_subtree`削除（正規形化へ統合） |
| src/layout/generator.rs | `generate_instance_kdl_v2`からtemplate引数と`replace_children_marker`を削除（正規形subtreeのみ受取） |
| src/app/remap.rs | `V2Plan`/`V2BlockPlan`/`V2TabPlan`型・`plan_v2`一般化（§3.2）・`instance_kdls`一般化・`execute`移動系列一般化（§3.4: 空groupのnew-tab経路・focus-pane-id・最終go-to先）・`verify`拡張（§3.6: N_t毎・pane focus・leftover）・dry-run出力拡張（§3.7） |
| src/zellij/mod.rs（trait）・src/zellij/process.rs | `focus_pane`追加（§3.5） |
| tests/fake_backend/fake.rs | `focus_pane`実装（呼出記録）・new-tab空group状況の構築helper |
| tests/unit/layout_planner.rs | §5条件のtest追加・既存testの`plan_v2`呼出修正（TabTemplate引数化。期待値はT=1後方互換〔式レベル一致〕により不変） |
| tests/unit/layout_generator.rs | template引数削除に伴う既存test修正（正規形subtree入力へ。期待値不変）+ 新規条件test |
| tests/fake_backend/remap.rs | §5条件のtest追加（移動系列一般化・focus-pane-id・leftover・pane focus検証） |
| tests/cli/remap.rs | dry-run出力期待値更新（S・N_t列・tab名・focus表示） |
| plugin/・companion.rs・src/cli.rs・main.rs | 変更なし（protocol・seed・CLI grammar不変） |

**旧API移行設計（TR75-9）**: 本改訂で削除・置換する旧APIと全参照更新の順序:

| 旧API | 現行参照箇所 | 移行先 |
|---|---|---|
| `layout::base_subtree` | src/app/remap.rs:203（`run`内layout解決）・tests/unit/layout_generator.rs（base構築helper） | `normalize_tab_templates(&doc)`の鋳型0 subtree |
| `layout::default_tab_template_subtree` | src/app/remap.rs:204 | `normalize_tab_templates`内へ吸収（外部参照は消滅） |
| `generate_instance_kdl_v2(base, runs, template)`（template引数） | src/layout/generator.rs:26-49・src/app/remap.rs:577-595（`instance_kdls`）・tests/unit/layout_generator.rs全条件 | `generate_instance_kdl_v2(base, runs)`（正規形subtree入力） |
| 単一instance `V2Plan`（`instances: Vec<V2InstancePlan>`） | src/app/remap.rs:17-125（型・plan_v2）・203-265（run内plan・dry-run）・577-595・tests/unit/layout_planner.rs全条件・tests/fake_backend/remap.rs・tests/cli/remap.rs | `blocks × groups`構造（§3.2）へ全面置換 |

移行の実施順序（compile到達を先行確認）: (1) `TabTemplate`・`normalize_tab_templates`追加（旧APIと並存）→ (2) remap.rs本体（plan_v2・instance_kdls・execute・verify・dry-run）を新構造へ切替え・旧API呼出を除去 → (3) 旧API（`base_subtree`・`default_tab_template_subtree`・template引数）を削除し、`rg "base_subtree\|default_tab_template_subtree"`の残存0を確認 → (4) tests/unit/layout_generator.rs・layout_planner.rsの入力を正規形TabTemplate化（旧test fixtureのbase_subtree直接組み立てを`normalize_tab_templates`経由へ。期待値はT=1後方互換〔§2.7〕により不変）→ (5) fake_backend/cliの期待値追従（focus-pane-id呼出・new-tab経路・S/N_t表示）。各段で`cargo check --all-targets`を先に通してからtest期待値を更新する（compile不能stateを長く持たない）。

## 4. 制約と未検証事項

### 4.1 実験で確定した事実（設計の根拠。tmp/task75/）

| # | 事実 | 根拠 |
|---|---|---|
| R75-1 | 異構成multi-tab layout（3x3+1+名無し2pane）起動: 名前ありtabは名前保持・名無しtabはzellij自動名「Tab #N」で具体化・default_tab_templateは全tabに展開され各tab末尾にbar leaf・new_tab_templateがdump末尾に残存 | ref-hetero.kdl起動・hetero_dump.kdl/hetero_tabs.json |
| R75-2 | tab focus: 指定tabがactive。focus無指定でも最初のtabがactive。dumpは常にactive tabにfocus=trueを付け、指定有無で区別不能 | hetero/nofocus比較・各tabs.json |
| R75-3 | pane focus: layoutのpane focus=trueは起動後に反映（非先頭pane・非active tabも有効）。list-panesのis_focusedは全tabでlayout指定どおり各1 pane。無指定時は各tab最初のterminal paneがfocused。dumpのpane focus=trueはactive tabのみ（非active tabのfocus位置はdumpから復元不能） | panefocus_panes.json・hetero_panes.json（focused: 各tab terminal 0/9/10）・各dump.kdl |
| R75-4 | pane idはterminal/plugin系列で独立採番（list-panes上id重複・is_plugin区別必須）。pane idはlayout宣言順に振られない（幾何との対応実測あり。id順=layout順の仮定禁止） | panefocus_panes.json（tab0: (0,0)=0,(0,20)=1,(0,40)=2,(67,0)=3,(134,0)=4,(67,20)=5,...） |
| R75-5 | `zellij action focus-pane-id <PANE_ID>`が存在（PANE_ID形式 terminal_N / plugin_N / bare N） | 20260915_doc_task75_focus_pane_id_help.txt |
| R75-6 | 1-pane tabのactive_swap_layout_nameはnull（多pane tabは"BASE"）。dumpのhide_floating_panes・zellij:link command pane等のdump noiseあり | hetero_tabs.json・panes.json（id 0 plugin zellij:link） |

### 4.2 未検証のまま残す事項（設計判断と根拠）

| # | 事項 | 採用した設計判断と根拠 |
|---|---|---|
| U75-1 | 複数のpane focus=trueを持つtabのzellij挙動 | 文書順最初を採用しwarning表示（D5）。未検証挙動に依存しないzelper側規則として確定。実機で遭遇時は検証(d)が検知 |
| U75-2 | N_t=0 tab（plugin-only等）の扱い | `LayoutInvalid`で事前中断。remap経路では全生成tabにterminal pane経由の具体化（break-new/new-tab）を要し、terminal pane 0の新規tab作成は未検証のため。zellij --layoutでは成立する構成だがremapの適用経路で再現手段が確定しない |
| U75-3 | focus-pane-id連打時の全tab focus保持（focus-pane-idが他tabのfocusを解除しないこと） | reference実測（起動直後: 各tab各1 pane focused・R75-3）からの推定。step 6で各tab訪問時にfocus-pane-idを打つ設計とし、検証(d)で検知する |
| U75-4 | ~~focus対象slotが空slot（bare shell新規spawn）の場合のfocus設定~~ | **解決（E2E要因A改訂・2026-09-16）**: focus対象を適用後のlist-panes位置から決定する設計へ変更し、空slot（spawn pane）も位置から特定可能となったため除外を撤廃（§2.4） |
| U75-5 | break-newで作った新規tabへのdefault_tab_template自動適用の有無 | 生成KDLがbar leafを含むため、適用があればRun::Plugin照合で消費され、無ければ新規spawnされ、いずれもbar 1件となる。二重化はRun::Plugin照合により構造的に起きない。E2E観察項目（§5.2） |
| U75-6 | 空group（割当0件tab）のbreak-new空list挙動 | 未検証のため不使用（維持）。空groupの具体化経路は実験4で`new-tab --layout-string`に確定（§3.4・U75-10解決） |
| U75-7 | 同名tab（block反復で鋳型名が重複する場合のzellij許容・go-to-tab-name曖昧性） | zelperはtab id基準で操作するため同名でも機能する。go-to-tab-nameはremap経路で使用しない。重複時のUI上の識別は-<b+1>接尾により発生しない（b>=1は接尾必須のため同名はblock 0内のみ。T>=2で鋳型名重複はlayout自身の宣言による） |
| U75-8 | 名無しtab鋳型のzellij自動名「Tab #N」の採番規則（作成順か位置か） | CLIから制御不能なため再現対象外。zelper側命名（base幹）で確定（§2.3）。D9（T=1後方互換〔式レベル一致〕）との整合: T=1では幹=baseで現行一致 |
| U75-9 | pane_template/tab_template（SKIP_NODES）使用layoutのslot数ずれ | v2.1と同様対象外（zellijは展開するがzelperは数えない。SKIP_NODES維持）。slot数不一致は検証(b)で事後検知される。本taskの範囲外として維持 |
| U75-10 | ~~new-tab（layout引数なし）の既定paneのterminal_command~~ | **解決（E2E要因B・実験4・2026-09-16）**: acceptance c/d系列で、既定pane（bare shell・invoked_with=None）はoverride-layoutのbare slotに照合されず残存しN_t+1 paneで検証(b)がfail。bare slotはtab作成経路（--layout-string）でのみ具体化され、override-layoutは既存paneとrun一致しない限り常に新規spawnする。空group経路を`new-tab --layout-string <生成KDL>`へ変更して解決（§3.4） |

### 4.3 E2E dump比較の除外項目対応表（TR75-8。§5.2で使用）

**比較対象は「tab毎slot数・幾何・bar存在・検証可能focus（list-panesのis_focused・list-tabsのactive）」に限定する**（DD-10.9）。dump-layoutの文字列比較は実施しない（dumpは正規化表現・noiseを含み、完全一致は保証・検証対象外）。dumpに現れる項目の除外根拠と代替検証の対応表:

| dumpに現れる項目 | 除外根拠 | 代替検証（保証項目） |
|---|---|---|
| tab名（名無しtab由来の「Tab #N」） | §2.3・U75-8: zellij自動名はCLIから制御不能で再現対象外 | 名前あり鋳型は生成tab名の一致（list-tabs。§3.6 (c)）。anchor名は例外（§2.1） |
| pane `name`属性→title | §2.8・D11: 保証外・観察のみ | なし（検証しない） |
| pane id | R75-4: 宣言順に振られず幾何との対応も非決定的 | pane idは検証の**手段**（(a)(d)のpane特定）であって比較対象でない |
| 非active tabのpane focus=true不在 | R75-3: dumpはactive tabのみにfocus=trueを付け、非active tabのfocus位置はdumpから復元不能 | list-panesのis_focused（全tab。§3.6 (d)） |
| active tabのfocus=true付与 | R75-2: dumpは常にactive tabにfocus=trueを付け指定有無を区別不能 | list-tabsのactive flag（tab focus検証） |
| `new_tab_template` node残存 | R75-1: zellijがdumpへ残す実機挙動。remapの再現対象外（§2.1） | なし（bar存在は各tab subtree末尾のbar leaf/paneで検証） |
| `hide_floating_panes`・`cwd "/"`・sizeの`"33%"`等の正規化表現 | dumpの正規化表現であり元layout宣言と字面が異なる | 幾何はlist-panesのx/y/rows/colsで検証（正規化非依存） |
| zellij:link command pane等のnoise | R75-6: zellij内部pane（is_plugin） | is_plugin/is_selectableで除外（source・幾何検証はterminal paneのみ） |
| active_swap_layout_name（多pane "BASE"・1-pane null） | R75-6: zellij内部状態 | なし（検証しない） |

### 4.4 実行時間の性能特性（CR75-3・2026-09-16 コードレビューで記録）

- **現状構造**: layout適用phase（step 6）は各生成tab (b,t)ごとに、存在確認のlist-tabs（lenient poll）とfocus対象決定のlist-panes（lenient poll・E2E要因A改訂で追加）を**逐次**実行する。各pollは空応答継続時に最大10s（POLL_DEADLINE）待ちうるため、screen繁忙が続く実行環境では最悪待ち時間が**生成tab数（T×k）に比例して累積**する（例: T×k=6なら最大60s級のlist-tabs待ちと60s級のlist-panes待ちが起こりうる）
- **致命的でない理由**: focus対象決定はbest-effortでありpoll timeout時はwarning skipして実行を継続し、検証(d)（同一の位置基準）で検知する。存在確認pollのtimeoutもOperationFailedとして報告されるだけでpaneは生存する（DD-10.11）。正確性（空応答のretry・deadline規則の統一）を優先した単純な逐次構造である
- **将来の改善方向**（現状は不採用・実装変更なし）: phase内での取得結果共有（1回のlist-panes取得をfocus対象決定と検証冒頭snapshotで共有する等）・poll deadlineのphase単位への繰上げ共有・存在確認の間引き（直前group処理直後のlist-tabs再利用）。screen繁忙時の実行時間が実用上の問題になった時に検討する

## 5. 検証計画

### 5.1 追加条件の概要（test-structure skill準拠の詳細は次段階の条件書更新で実施）

| 条件書 | 追加予定の条件（idは案） |
|---|---|
| tests/design/layout-planner.toml | multi-tab割当が累積slot式に従う（i → (b,t,s)。layout-planner.multi-tab-allocation-cumulative-slots）・k = ceil(M/S)（k-uses-total-slots）・生成tab名規則（tab-names-from-templates-and-block-suffix）・N_t=0鋳型はLayoutInvalid（zero-slot-tab-template-invalid）・pane_focus_slot抽出（文書順最初・複数時warning）（pane-focus-slot-first-focused-leaf）・空groupの生成（empty-group-planned-for-all-blocks-tabs）・T=1後方互換（式レベル一致）の回帰固定（既存R1〜R7期待値不変のまま維持されることの明記） |
| tests/design/layout-generator.toml | 正規形subtreeからのper-tab KDL生成（generator API変更後の入力形式。既存条件の入力読み替え含む: template引数廃止・normalize_tab_templates経由の入力） |
| tests/design/remap-sequence.toml | 移動系列一般化（5-a→5-b block/tab昇順・rename列）（move-phase-generalized-block-tab-order）・**空groupのnew-tab経路（TR75-5昇格・fake必須条件: tab ID stdout parse・parse不能/非zero exitはOperationFailed・既定paneが生成KDL適用でN_tへ正規化・preflightは状態変更前に同一depthで完了）**（empty-group-tab-created-via-new-tab-then-rename）・focus-pane-id per tab（go-to→override→focus-pane-id順・occupied限定・失敗はwarning・backendは薄いResultでexecuteがwarning化）（focus-pane-id-after-override-per-tab）・最終go-to先（focus=true鋳型のblock 0 tab・無指定anchor）（final-go-to-honors-template-focus）・検証(b) N_t毎・anchor (0,0)を含む全生成tab（verify-per-tab-pane-count）・検証(d) pane focus（pane-focus-verified-via-is-focused）・leftover報告・非close（leftover-tabs-reported-not-closed）・**空groupのみ実行（M=0・source全てanchor内）でbreak系0件・companion不使用・preflight完了後のnew-tabのみ**（new-tab-only-needs-no-companion） |
| tests/design/remap-cli.toml | dry-run出力拡張（S・N_t列・tab名・focus・操作列のnew-tab/focus-pane-id）（dry-run-reports-s-and-per-tab-slots）・JSONのn=S互換（json-n-field-holds-total-slots） |
| tests/design/backend-process.toml | focus-pane-id argv組み立て（focus-pane-id-argv） |

テスト階層: planner・正規形化はtests/unit/、移動系列・verify・leftoverはtests/fake_backend/remap.rs、dry-runはtests/cli/remap.rs、argvはsrc/zellij/process.rs内test。

### 5.2 E2E acceptance（実装・自動test完了後・podman sandbox）

前提: tmp/task74/acceptance/と同一harness構成（podman + debian:12-slim + host zellij read-only mount・--network=none・script -qec PTY・rows 60 cols 200）。成果物は`tmp/task75/acceptance/`。

**(i) reference取得**: ref-hetero.kdl（tmp/task75/流用可）でsession起動し、list-panes（幾何・is_focused）・list-tabs（tab名・active）を採取してtag保存

**(ii) multi-tab適用（本taskのacceptance）**: seed単tab session（bar付き1 tab）でterminal pane 12件作成 → `zelper --session S remap --path ref-hetero.kdl --json`

- **必須合格条件**:
  - exit 0・`ok:true`
  - terminal pane数が12のまま（重複spawnなし）
  - tab構造: 3 tab生成（k=1・T=3）。tab 1（anchor）は名前保持（seed名のまま）・tab 2は"T-single"・tab 3はbase名（名無し鋳型）
  - tab focus: layoutにfocus=true鋳型（T-3x3）があるため最終go-to先がblock 0の当該tab（= anchor）。active tabがanchorであること
  - pane focus: 各tabで期待slotのpaneがis_focused（tab 0: slot 0相当〔無指定〕・tab 1: slot 0・tab 2: slot 0。ref-panefocus相当のlayout側focus指定があれば指定slot）
  - per-tab幾何: **anchorを含む全生成tab**で(i)のreference幾何と一致（tab 0が3x3・tab 1が1 pane・tab 2が左右2分割。TR75-6: anchor例外は名前・位置のみであり形状・slot数の例外ではない）
  - bar: 全生成tabにcompact-bar pane（y=59・rows=1・cols=200）が存在
  - anchor tab名がlayoutの"layout最初のtab名"と一致しないこと（D1例外の確認・seed名のまま）
- **観察条件**（合格判定外・記録）: U75-5（break-new新規tabへのtemplate bar自動適用有無）・dump-layoutのnew_tab_template残存（R75-1どおり）

**(iii) k>=2 smoke（block反復・空groupのnew-tab経路含む）**: pane 15件（S=12 < 15）→ remap。15 = 12+3で、block 0 = 12 pane（tab0=9・tab1=1・tab2=2）・block 1 = 3 pane（tab0=3・**tab1/tab2は割当0件の空group**）

- **必須合格条件**: exit 0・`ok:true`・15 pane生存・6 tab（k=2 × T=3）・block 1の生成tab名が`<鋳型名|base>-2`形式・各tab幾何が鋳型どおり（anchor含む全tab）・bar全tab。**空group 2件のnew-tab経路実証（TR75-5昇格）: new-tab由来tabのID parse・rename済み生成tab名・既定paneが生成KDL適用でN_t（tab1=1・tab2=2）へ正規化されること**
- **観察条件**: 空group tabの既定paneがbare shell正規化後に残存しないことの詳細記録（pane数はN_t一致で検証済み）・U75-5

**(iv) T=1回帰**: 9-pane.kdl（TASK-74と同一）で単tab適用のacceptance (ii)（TASK-74手順）を再実行し、割当・命名・focus復帰が現行と同一であることを確認（D9の実機確認。新規にfocus-pane-id 1回が増えることの確認を含む）

**(v) tab focus決定則matrix（TR75-7追加）**: (i)と同一harnessでlayout側focus指定を変えた3系列を実施し、最終go-to先の決定則を実機で固定:

- 系列a: focus=trueが**文書順2番目のtab**にあるlayout（ref-hetero.kdlでtab 0 "T-3x3"のfocus指定を外しtab 1 "T-single"へ付与）→ 最終go-to先が (0, t*=1)（新規tab側。anchorでない）であること（active tabが"T-single"）
- 系列b: **複数tabにfocus=true**があるlayout（tab 0 "T-3x3"とtab 2名無しtabの両方）→ 文書順最初（tab 0。= anchor）が最終go-to先であること（複数focus検出のwarning出力を含む）
- 系列c: **focus=trueなし**（ref-hetero-nofocus.kdl相当）→ anchor復帰であること（現行挙動と同一）
- 各系列とも必須合格条件: exit 0・`ok:true`・active tabが上記期待どおり（list-tabs）。pane focus検証(d)は(i)と同一基準

## 6. DD-10 v2.2改訂の対応表と要件反映

| DD-10節 | 改訂 |
|---|---|
| 10.冒頭改訂履歴 | v2.2（TASK-75）を追記 |
| 10.1 | 事実6（R75-1〜R75-5・R75-6）を追記 |
| 10.2 #6 | anchor tab名保持（同一性検証の例外・D1）を注記 |
| 10.5 | --tab指定時の適用規則（最初のtab鋳型の単tab適用・D10）を追記 |
| 10.6 | 配分則を全面改訂（正規形TabTemplate列・S・累積slot式・k・命名一般化・N_t=0無効・T=1後方互換〔式レベル一致〕・決定論保証範囲のper-tab化） |
| 10.7 | 移動phase一般化（5-a/5-b・空groupのnew-tab経路）・step 6へfocus-pane-id・step 7最終go-to先 |
| 10.8 | baseを正規形subtreeへ（template一元化・generator引数廃止） |
| 10.9 | (b) N_t毎・(c) anchor名例外・(d) pane focus・leftover_tabs報告 |
| 10.10 | dry-run/JSON拡張（S・N_t列・tab名・focus・n=S互換） |
| 10.13 | 対象外明記（pane name属性→title・new_tab_template・Tab #N自動名・focus対象空slot） |
| DD-3.3 | default_tab_template正規形化（10.6/10.8参照）の文言整合 |

要件側: requirements.md §2.6へ (1) multi-tab layout全体再現の要件（tab名〔anchor例外〕・tab focus・per-tab形状・bar全tab） (2) 決定論保証範囲のblock×tab配分への拡張 (3) 過剰tabの非明示close禁止と報告、を追記する。

## 7. 実験記録（予算3回消化）

TASK-75の実験予算は3回（podman sandbox・ref.sh系script）。全て消化済み:

| # | 実験 | layout | 主な成果物 |
|---|---|---|---|
| 1 | 異構成3 tab reference | ref-hetero.kdl | 20260915_exp_task75_hetero_{dump.kdl,panes.json,tabs.json}・20260915_log_task75_run.log・typescript_ref-h.log |
| 2 | tab focus無指定比較 | ref-hetero-nofocus.kdl | 20260915_exp_task75_nofocus_{dump.kdl,panes.json,tabs.json}・typescript_ref-nf.log |
| 3 | pane focus指定 | ref-panefocus.kdl | 20260915_exp_task75_panefocus_{dump.kdl,panes.json,tabs.json}・20260915_log_task75_panefocus_run.log・typescript_ref-pf.log |

静的採取（実験予算外）: action help一式（20260915_doc_task75_*.txt: focus-pane-id・go-to-tab・go-to-tab-name・tab rename等）・ref.sh/panefocus.sh/static.sh（再現script）。

### 7.1 E2E acceptance結果と改修（2026-09-16）

acceptance（§5.2・tmp/task75/acceptance/）: **55 PASS / 18 FAIL**。FAILは2要因の実装bugに集約され、ともに改修済み:

- **要因A（7行: 全系列のpane focus slot 0検証）**: focus-pane-id対象をmapping pane idで決定していたため、bare pane群のrun一致配置（割当順と無関係）で実幾何slot 0のpaneがfocusedにならない（b系列: mapping slot 0 = terminal_0が実幾何(134,40)=3x3右下に配置）。副次観察: focus-pane-idがalready focusedでexit 2を返す際、focus状態は実際に成立している。→ §2.4改訂: 対象を適用後のlist-panes位置（visual order s番目）から決定・occupied限定撤廃・already focusedは成功扱い
- **要因B（11行: (iii)(iv)のexit 7・pane数N_t+1・幾何崩れ）**: 空groupの旧経路（new-tab layout引数なし → go-to → override-layout）で既定paneがbare slotに照合されず残存（c系列: T-single tab 2 pane・ref-hetero tab 3 pane）。U75-10の設計推定が実機で不成立。→ 実験4で経路確定のうえ §3.4改訂: `new-tab --layout-string <生成KDL>`（step 6のoverride-layout skip込み）

**実験4（要因B経路確定・acceptance後の予算追加実験・2026-09-16）**: podman sandbox（harness同一構成）で1回の起動に両候補を調査。成果物: tmp/task75/exp-newtab/nb_*（20260916_log_task75_newtab_run.log）:

- 候補1（採用）: `new-tab --layout-string <生成KDL>` — 1-slot+bar生成KDLでtab作成時にterminal 1 pane+compact-bar（pane数==N_t）・2-slot+bar生成KDLで左右2分割（cols=100/100・幾何もlayoutどおり）・stdoutにtab id（parse可）
- 候補2（不採用）: `new-tab`（引数なし）→ `close-pane`で既定paneを閉じるとtab自体が自動closeされる（E6事実1の帰結。tabが残らないため不成立）

追加実験が必要になった場合は別途予算相談とする（残るU75系は検証(d)/(b)による事後検知で担保する設計）。

## 8. スコープ外（本設計で扱わないもの）

- swap layout自動切替・floating pane配置の再現（SKIP_NODES・対象外維持）
- new_tab_templateのremap後新規tabへの適用（zellij予約領域・対象外）
- pane title（name属性）の設定（D11・観察のみ）
- 過剰tabの自動close（D6・報告のみ。remove verbの明示利用）
- tab位置（position）の並替再現。生成tabはblock昇順・tab昇順でbreak-new/new-tabされるため結果的に宣言順になるが、anchorの位置（現positionのまま）はlayout最初のtab位置と一致する保証はない（anchor再利用・D1の帰結。anchor名と同様に例外として扱う）
