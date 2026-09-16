# TASK-74 修正設計: remap適用経路のrun一致照合・plugin bar保持・poll空応答の修正（DD-10 v2.1）

作成日: 2026-09-15
対象: TASK-74（ユーザー報告「zelper remapでまったくlayoutが守られない」の修正）。原因特定（前セッション・podman実験 a/b/c）と本設計のspike S（後述）に基づく実装設計
規約上の位置づけ: 本fileはTASK-74修正の設計記録（根拠・変更範囲・test-first計画・E2E acceptance手順）。規範的な仕様変更は`detailed-design.md` DD-10本体へv2.1改訂として反映済み（§5に対応表）。multi-tab layout全体再現（tab名・focus・template bar含む全体像）はTASK-75へ分離済みであり、本設計は「単tab適用の正しさ」に限定する

## 1. 原因の確定（設計前提。spike Sの証拠を含む）

前セッションの原因特定（body: 主因A=B「bare slotとshell paneのinvoked_with不一致」）は**一部誤りだったため、spike S（2026-09-15・podman sandbox・`tmp/task74/spike74/`）で再実証し、以下のとおり確定した**。zellij 0.44.3 source（`tmp/20260914_src_task74_*.rs`）と実測の双方による:

| # | 確定事実 | 根拠 |
|---|---|---|
| F1 | `list-panes --json`の`terminal_command`はpaneの`invoked_with()`（起動時Run）の序列化（`Run::Command`の`to_string`）。plugin paneでは`plugin_url`が同等の役割 | tab/mod.rs pane_info生成部（`pane_info.terminal_command = pane.invoked_with()...`）。実験b/c fixture |
| F2 | `list-panes --json`の`pane_command`は**PTYの現在の前景process**（/proc由來。`enrich_pane_with_running_command`・100ms timeout）であってinvoked_withとは無関係。shell paneでは常にshell自身（例: `/bin/sh`） | route.rs enrich系。実験b fixture（全pane `pane_command:"/bin/sh"`） |
| F3 | bare `pane` / `action new-pane` / layout bare slot で起動したpane（既定shell）は`invoked_with=None`。bare slot（run=None）と**完全一致照合が成立する** | pty.rs SpawnTerminal handler（instruction引数の`terminal_action`がNoneのままinvoked_withを計算）+ spike S（bare pane×4がbare slot×4へ新規spawnなしで配置） |
| F4 | `Run::Command(RunCommand)`の等価比較は**cwdを含む全field一致**。slot側にcwdを書かないと、invoked_withのcwd=Someであるpaneとは照合しない（新規spawnが発生） | spike S（`cwd="/tmp"`付きseed起動のsleep 800×2に対し、cwdなしslot×1+cwd付きslot×1のtargetを適用→新規sleep 800が1件spawn・元paneは不成立でgrid内挿入）。**検証済みなのは絶対path cwd明示の場合のみ**（TR74-7。相対cwd・layout継承cwd・`zellij run --cwd`の正規化差異は未検証。§2.6参照） |
| F5 | 同一runの複数paneは1対1で消費される（slot順はzellij側run一致順依存・DD-10.6 (iii)のとおり保証外） | spike S（sleep 700×2 → 同command slot×2へ spawn 0で配置） |
| F6 | 生成KDLに`pane size=1 borderless=true { plugin location="zellij:compact-bar" }`を書くと、既存compact-bar paneがrun一致で**宣言位置へ配置される**（grid内挿入されない） | spike S（適用後もbarはy=59・rows=1・cols=200の最下行） |
| F7 | 照合不成立の既存paneは`handle_remaining_tiled_pane_ids`でgrid cell内に分割挿入され、gridを崩す。同時に不成立slotは新規spawnされpane数が増える | 実験b（9 pane→18 pane・dump崩壊）+ spike S（不成立sleep 800がPane #8のcellを分割） |
| F8 | screen繁忙中（27 paneへのlayout適用中等）の`list-panes`は、screen応答1s timeoutで`Ok(None)`→CLIは**stdout空・exit 0**を返す | route.rs `request_panes_from_screen`（recv_timeout 1秒）。実験c（`EOF while parsing a value at line 1 column 0`で中断・中途断状態） |

これにより3原因は以下のように確定する:

- **主因A（照合keyの誤り）**: 旧DD-10.8規則「pane_commandがSomeなら単語1つのshell起動paneでも注入する」が、F2/F3によりinvoked_with=Noneのshell pane（ユーザーの全paneがこれ）へ`command="<pane_command>"`を注入し、bare slotとの照合成立を妨害していた。**DD-10.1 事実5「bare paneはshell pane（invoked_with=None）のみ一致」自体は正しく、注入側のdata sourceが誤り**
- **主因B（template barの脱落）**: `base_subtree`（=最初のtab subtree）には`default_tab_template`のbar（compact-bar）が含まれないため、生成KDLからbar leafが脱落し、bar paneが不成立paneとしてgrid内挿入されていた（F7）。tab subtree内に直接書かれたplugin leafは従来どおり生成KDLに維持されるため問題ない
- **副因C（poll中の空応答即abort）**: F8の一時空応答をparse errorとして即中断していたため、移動済み・rename未実施の中途断状態で終了していた

## 2. 修正設計

### 2.1 主因A: 照合keyを`terminal_command`（invoked_with）へ変更

- **data model**: `PaneState`（src/domain/mod.rs）に`pub terminal_command: Option<String>`を追加。`PaneInfo`（src/zellij/parser.rs）に`terminal_command`fieldを追加（`#[serde(default)]`付き。旧fixture互換）し、`to_pane_state`でmap
- **planner**（src/app/remap.rs `plan_v2`）: `V2Assignment.run`の算出を`pane.terminal_command`基準へ変更:
  - `terminal_command = None`（bare・既定shell起動）→ `run = None`（生成KDLはbare slotのまま。**`pane_command`（前景process）は参照しない**）
  - `terminal_command = Some(s)` → `run = Some(s.split_whitespace())`（command=argv[0]・args=残り。**cwdは注入しない**）
- **注入規則の変更点**: 旧「pane_commandがSomeなら単語1つのshell起動paneでも注入する」を廃止。pane起動時に明示commandを持つpane（layout `command=` / `zellij run`）は`terminal_command`がSomeになり注入される。`zellij run`（--cwdなし）のinvoked_withはRunCommand{cwd=None}であるため無cwdslotと照合する（pty.rs handlerがdefault shell補完前にinvoked_withを計算するため）
- **quoting warning**（`needs_quoting_warning`）: 対象を`pane.command`から`pane.terminal_command`へ変更（表示textも読み替え）。理由は同一（空白分割でquoteが復元できずrun一致が外れうる）
- **`pane_command`の役割**: 変更なし。表示・`--command` selectorのfilterはpane_commandのまま（DD-1.4「pane_command部分一致」の契約不変）
- **N/M/k計算・`--embed-floating`・source順序**: 変更なし（DD-10.5/10.6のまま）。照合はlayout側runがpane側invoked_withと対称になるため、「実際のcommandを持つpaneの照合優先順位」という問題自体が消滅する（bareはNone同士・command paneはargv同士）
- **採用しない案**: cwdとして`pane_cwd`を注入する案（S2）は、invoked_withのcwd=Noneかつprocess cwd=Some（F4のsleep 700側）の照合を壊すため不採用（spike Sで両ケースを直接比較）。pane_cwdは常にSomeに近く、invoked_withのcwd None/Someを区別するdataがCLI表面に存在しないため、無cwd（S1）が広く成立する側を選ぶ

### 2.2 主因B: `default_tab_template`の生成KDL反映（children置換）

**選択肢比較**（要件: pane保存・非破壊〔L93/L94系〕・layout宣言の維持）:

| 案 | 内容 | 評価 |
|---|---|---|
| (a) close+再生成 | plugin paneをcloseし、生成KDL側のplugin slotで再生成（retain_plugin=false運用） | pane idが変わり「plugin paneを誤って破壊しない」（要件2.6）の境界判断が必要。状態を持つplugin（zjstatus等）の再初期化も起きうる。bar宣言の維持手段として成立するが、id保存の観点で劣る |
| (b) 退避 | plugin paneをfloating化等でgrid外へ退避し、適用後に戻す | plugin paneに対するtoggle系の挙動はE6未検証（DD-10.2 #13と同様の未検証依存）。適用前後の追加状態変更2回が入り、失敗経路が増える |
| (c) 挿入対象除外（元位置に残す） | zellij側の`handle_remaining_tiled_pane_ids`挿入をzelperから制御する手段が存在しない（retain旗はclose制御であり挿入位置制御ではない） | 実現手段なし（実装不能） |
| **(d) 採用: template leafの生成KDL反映** | 生成KDLにbar leaf（`pane size=1 borderless=true { plugin ... }`）を含め、既存plugin paneとRun::Plugin照合させて宣言位置へ配置 | pane id保存・状態保存・状態変更追加なし。F6（spike S）で「宣言位置へ照合配置」を実証済み。close・退避の失敗経路を持たない |

- **template取得**: src/layout/mod.rs に`default_tab_template_subtree(doc) -> Option<KdlDocument>`を追加（`default_tab_template` nodeのchildren。`SKIP_NODES`のslot数えへの影響なし。`new_tab_template`は対象外のまま）
- **引渡し経路（TR74-2）**: template subtreeは**layout解決時に1回だけ取得し、plan実行系へ明示的に渡す**（`V2Plan`へのfield追加は行わない——planは純粋関数でslot/runのみを持ち、KDL生成は実行時の引数で完結する現行構成の維持）。関数signature単位の経路:
  1. `remap::run`: `layout::default_tab_template_subtree(&doc)` → `Option<KdlDocument>`を取得（dry-run・execute双方で使用。`None`はtemplate無しlayout）
  2. `instance_kdls(plan, base_sub, template)` — 第3引数を追加（src/app/remap.rs:568の現行signature `instance_kdls(plan, base_sub)`からの変更）
  3. `generate_instance_kdl_v2(base, runs, template)` — 第3引数を追加（src/layout/generator.rs:26の現行signatureからの変更。`template: Option<&KdlDocument>`）
  4. dry-run経路（`remap::run`内の`instance_kdls(&plan, &base_sub)?`呼び出し）も同signatureへ更新
- **生成KDL例（TR74-2）**: 9-pane.kdl（template有り・base subtree = 3x3入れ子grid）のinstance 0:
  ```kdl
  layout {
      pane split_direction="horizontal" {
          pane split_direction="vertical" { pane; pane; pane }   # 3 slot（terminal_command=Noneならbare・Someならcommand注入）
          pane split_direction="vertical" { pane; pane; pane }
          pane split_direction="vertical" { pane; pane; pane }
      }
      pane size=1 borderless=true {                              # template由来（children置換後に残るleaf）
          plugin location="zellij:compact-bar"
      }
  }
  ```
  template無しlayout（例: `--inline 'layout { tab { pane; pane; pane } }'`）の生成KDLは現行どおりslot木のみ（bar leafは出現しない）。
- **slot数え**: 変更なし。`count_terminal_slots(base_subtree)`のまま（template配下は数えない。bar leafはslotを消費しない MR-32規則の維持）
- **生成**: src/layout/generator.rs `generate_instance_kdl_v2`に`template: Option<&KdlDocument>`引数を追加:
  - `Some(template)`の場合: template subtree内の**文書順最初の`children` nodeを、base subtreeのnodesで置換**したtreeを出力する。`children`不在は`LayoutInvalid`（"default_tab_template has no children node"）。template配下のそれ以外のnode（plugin leaf・size等）はそのまま維持
  - `None`の場合: 現行どおりbase subtreeのみ
  - plugin leaf（bareおよびpane内plugin）は従来どおりslot indexを消費しない（`leaf_is_plugin`規則の維持。F6によりbar paneは宣言位置へ照合配置される）
  - **preflight追加検証**: 置換後のtreeのterminal leaf数が`count_terminal_slots(base_sub)`と一致することを確認（template配下に`children`以外のterminal pane nodeがある場合は不一致→`LayoutInvalid`で事前中断。誤注入防止）
- **要件との関係**: 「plugin leafは保存・slotを消費しない」（DD-3.3/10.8）の適用範囲を「最初のtab subtree内」から「+ default_tab_template内」へ拡張する。barの自動注入ではなく**layout宣言の維持**である（宣言なきbarの付与は従来どおり行わない）
- **TASK-75との境界**: 本修正で生成KDLは全instance（j>=1含む）にbar leafを載せる。anchor tab（j=0）のbarは既存paneとの照合で保持され、j>=1の新規tabではbarが新規spawnされる（一貫した外観・pane破壊なし）。tab名・focus・k>1の全体再現は引き続きTASK-75の対象。bar新規spawnの実機確認はE2E acceptance (iii)で行う

### 2.3 副因C: poll中の一時空応答のretry化

- **backend interface**（src/zellij/mod.rs trait + process.rs + fake）: `list_panes_lenient() -> Result<Option<Vec<PaneState>>>`と`list_tabs_lenient() -> Result<Option<Vec<TabState>>>`を追加:
  - stdout空・exit 0 → `Ok(None)`（条件未成立として扱える値）
  - 非空でparse失敗・非zero exit → 従来どおり`Err`（fatal）
  - 実装は既存`list_panes`/`list_tabs`のparse分岐で空入力を`Ok(None)`へ分岐させる（parser.rsに`parse_panes_opt`/`parse_tabs_opt`を追加）
- **適用範囲**（src/app/remap.rs）:
  - `execute()`内の全poll closure（probe poll・5-a完了poll・5-b membership poll）: `list_panes()?`を`list_panes_lenient()?`へ置換し、`Ok(None)`は`Ok(None)`（未成立）として扱いpoll継続
  - `verify()`冒頭: `list_panes`/`list_tabs`を取得できるまでpoll（deadline 10s。既存`poll` helperを再利用。空が続けばdeadline超過で`OperationFailed`）
  - `run()`冒頭のsnapshot取得（`panes_now`/`tabs_now`）: 同様にlenient取得へ変更（状態変更前なので空が続いても中断は安全。deadline超過は従来のparse error相当のerrorへ）
- **fatal条件の定義**: (1) 空応答がdeadline（10s・DD-10.7の定数不変）を超えて継続 (2) 非空でのparse失敗 (3) 非zero exit。(1)は既存のpolling timeout error（phase文言付き）へ合流させ、F8の機構（screen busyの1s timeout）をerror文面に1行注記する
- **C3（poll deadline再検査）との整合**: `poll`本体は不変。leniencyはcond内の`Ok(None)`扱いのみであり、deadline超過後の条件成立を成功としないC3規則は維持される

### 2.4 dry-run出力の変化（DD-10.10）

- **human出力**（`print_dry_run_human`）: source行の`cmd:`表示を`terminal_command`基準へ（`inv:`等の表記に変更。前景processの`cmd:`との混同防止のため両方表示しない）。KDL previewはbare slotが増え（shell pane）、template適用時はbar leafが出現
- **json出力**（`dry_run_json`）: `source[]`に`"invoked_with"`fieldを追加（値はterminal_commandのstring or null）。`"command"`（pane_command）は表示目的で残す。`assignments[].run`はterminal_command由來のargv（schema不変。stable fields外のため追加fieldは契約変更にならない）
- **影響を受ける条件書**: `remap-cli.r48`（source一覧）・`remap-cli.r49`（KDL preview）の期待値更新

### 2.5 変更対象file一覧

| file | 変更 |
|---|---|
| src/zellij/parser.rs | `PaneInfo.terminal_command`field追加（serde(default)）・`parse_panes_opt`/`parse_tabs_opt`追加 |
| src/zellij/mod.rs（trait）・src/zellij/process.rs | `list_panes_lenient`/`list_tabs_lenient`追加 |
| src/domain/mod.rs | `PaneState.terminal_command`追加 |
| src/layout/mod.rs | `default_tab_template_subtree`追加 |
| src/layout/generator.rs | `generate_instance_kdl_v2`へtemplate引数・children置換・terminal leaf数検証 |
| src/app/remap.rs | `plan_v2`のrun算出・warning対象変更、poll closureのlenient化、`verify`冒頭のlenient poll、snapshot取得のlenient化、`instance_kdls`/dry-run経路のtemplate引数対応、dry-run出力（human/json） |
| tests/fixtures/*.json（fake zellij shim用） | panes fixtureへ`terminal_command`追加 |
| tests/fake_backend/fake.rs | `PaneState`構築helperへfield追加・lenient系の記録（空応答系列の失敗注入用） |
| tests/unit/parser.rs・layout_planner.rs・layout_generator.rs | 条件書追従のtest更新・追加 |
| tests/fake_backend/remap.rs | 空応答retry・terminal_command照合のtest追加 |
| tests/cli/remap.rs | dry-run出力期待値更新・fixture更新 |
| 以下PaneState struct literal全箇所（TR74-1。`rg "PaneState \{"`で列挙・field追加により**既存literalがcompile不能になる箇所**。helper関数のfield追加で一括解決できるものはその旨注記） | tests/unit/layout_planner.rs:24（helper `pane()`内。helper引数で一括）・tests/unit/selector.rs:15,45,67（helper + literal 2件）・tests/fake_backend/remap.rs:28（helper。terminal_command値を引数化）・tests/fake_backend/rename_add_remove.rs:8（helper）・tests/fake_backend/resize.rs:9（helper）・tests/fake_backend/fake.rs:197,345,415,596（fake状況構築literal 4件。selector/list系の用途のためterminal_commandはNoneでよいが、remap経路の状況には実在の値を設定）・src/zellij/parser.rs:107（`to_pane_state`の本番map。`terminal_command: i.terminal_command`） |

**PaneInfoのserde旧fixture互換（TR74-1）**: `terminal_command`fieldは`#[serde(default)]`付きで追加するため、`terminal_command` keyを欠損した既存fixture（tests/fixtures/*.json・test内JSON文字列）はfield欠損を`None`としてparseし**既存testは修正なしでcompile・実行継続できる**。remap照合に関わるfixtureのみ§3の直交ケース追加時に明示的に`terminal_command`を載せる。

plugin/・companion.rs・CLI grammar（src/cli.rs・main.rs）は変更なし。

### 2.6 既知制限の拡充（DD-10.13へ反映）

- **cwd明示paneのrun一致不可**: invoked_withが`cwd=Some`のpaneは、無cwdslotと照合せず、複製spawn+grid内挿入となり、検証(b)（pane数==N）が`VerificationFailed`で検知する（F4。CLI表面にinvoked_withのcwdを区別するdataが存在しないため。error文面に「当該paneの起動cwdが原因の可能性」を含める）。**cwd表現の差異は検証状況が分かれる（TR74-7）**:
  - **検証済み（F4・spike S）**: 絶対path cwd明示（layout `cwd="/tmp"`）→ 無cwdslotと照合しない
  - **未検証（同じ照合外れ経路と推定）**: 相対cwd指定・layout上位からの継承cwd（`Run::merge`由來）・`zellij run --cwd`の正規化差異（path解決・正規化後にRunCommandへ入るか否か）。いずれも起動時cwdがRunCommandのcwd fieldへ載る以上同じ等価比較を通るため、無cwdslotとの照合外れという同じ帰結と推定する。追加実験は実施しない（予算温存。実機で当該構成に遭遇した場合は検証(b)が検知する）
  - **preflight検出の可否**: zelperは`terminal_command`（command+argsのみの序列化）からcwdを観測できないため、**cwd=Someであること自体をpreflightで検出できない**（検出は検証(b)の事後検知のみ）。`pane_cwd`は前景processの現在cwdであってinvoked_withのcwdの代理にもならない（F4のsleep 700側: invoked cwd=None・pane_cwd="/"）
- **hold flag差異**: `hold_on_start`/`hold_on_close`付きで起動したpaneは無hold slotと照合しない（検証(b)で検知）。`exited`/`is_held`のpaneは従来どおりsource外
- **`zellij edit` pane**: invoked_with=`Run::EditFile`は`terminal_command`に現れない（null）ためbare slotと照合せず、複製spawn+挿入となる（検証(b)で検知）。EditFile paneは通常短命であり実害は限定的
- **quoting**: terminal_commandの空白分割はquote復元不可（従来のwarning機構をterminal_command向けに維持）
- 上記いずれも「黙示の破壊」ではなく検証失敗として検知される経路であること（DD-3.5 compatibility policyの安全網方針の維持）

## 3. test-first計画（段階5の指針。条件書→test codeの順で実施、fail-first確認）

| 条件書 | 変更 | id |
|---|---|---|
| tests/design/layout-planner.toml | 新規: run算出がterminal_command基準（None→bare・Some→argv）。pane_command=Some かつ terminal_command=None（shell上で前景process実行中）のpaneはbare slot | `layout-planner.run-derives-from-invoked-with` |
| 同上 | 更新: quoting warningの対象をterminal_commandへ | `layout-planner.r20-quoted-chars-in-pane-command-warn-but-do-not-fail`（description・source追記） |
| tests/design/layout-generator.toml | 新規: default_tab_templateのchildren置換（bar leafが生成KDLに残る）・children不在はLayoutInvalid・template内terminal pane検証 | `layout-generator.template-children-substitution` |
| 同上 | 更新: r13/r14/r15/r18の入力をterminal_command由來へ読み替え（r14「単語1つのshell起動paneでも注入」は「起動時明示commandの単語1件pane」の意に確定。generator APIのSlotRunは不変） | 既存idのdescription更新 |
| tests/design/remap-sequence.toml | 新規: probe/移動poll中の空list-panes応答はretryされ、成立後継続（中断しない）。verify冒頭の空応答もretry | `remap-sequence.empty-list-panes-retry-in-polls`・`remap-sequence.empty-response-retry-before-verify` |
| 同上 | 更新: r37（survive）等のfake fixtureにterminal_commandを持たせる（fake.rs helper更新。期待値自体は不変） | source_lines自動syncのみ |
| tests/design/remap-cli.toml | 更新: r48（source[]のinvoked_with追加）・r49（KDL preview: bare slot・bar leaf） | description・期待値更新 |
| tests/design/backend-parser.toml | 新規: terminal_command fieldのparse（null / 文字列・旧fixtureのdefault互換） | `backend-parser.terminal-command-field-parsed` |
| tests/design/README.md・tests/README.md | 条件数の更新 | - |

テスト階層: planner/generatorの純粋関数testはtests/unit/へ、空応答retryはtests/fake_backend/remap.rs（fake backendへ「N回空を返す」失敗注入を追加）へ、dry-run出力はtests/cli/remap.rs（shim fixture更新）へ配置。test-plan §2.7のR20等の文言調整は条件書更新時に合わせて実施する。

**両field直交fixture（TR74-4。旧実装〔pane_command依存〕が必ず失敗するfail-first条件化の核）**: description更新のみでは旧実装を検出できないため、以下の直交2 paneを最低1組、planner・generator・shim dry-run（cli）の各層に共通の入力として用意する:

| pane | `terminal_command` | `pane_command` | 期待（新実装） | 旧実装の挙動（fail-first検出） |
|---|---|---|---|---|
| P-shell-proc | `None`（bare起動shell） | `Some("vim src/main.rs")`（shell上で前景process実行中） | `run = None`（bare slot） | pane_command依存では`command="vim"`注入→照合崩壊。planner testは`run==None` assertで旧実装をFAILさせる |
| P-cmd-pane | `Some("claude --model x")`（起動時command） | `None`（enrich timeout等で欠損）または`Some("claude --model x")`と別値 | `run = Some(["claude","--model","x"])`（command slot注入） | pane_command依存では`run=None`（bare slot化）または別argv注入→照合成立しない。planner testは`run==Some(...)` assertで旧実装をFAILさせる |

parser層ではこの2 paneを含むlist-panes JSON（`terminal_command` key明記）を`tests/unit/parser.rs`へ、cli層では同一構成のfake zellij shim fixture（tests/fixtures/）を`tests/cli/remap.rs`のdry-run期待値（KDL preview: P-shell-proc slotはbare・P-cmd-pane slotはcommand注入）へ載せる。generator層はSlotRun経由で既存条件が覆盖するため、直交fixtureはplanner・parser・cliの3層に配置する。

## 4. E2E acceptance手順（実装・自動test完了後、podman sandboxで実施）

前提となる既存script: `tmp/task74/20260914_exp_task74_p2.sh`（実験 a/b/c）と同一構成（podman + debian:12-slim + host zellij read-only mount・`--network=none`・`script -qec`によるPTY駆動・rows 60 cols 200）。成果物は`tmp/task74/acceptance/`へ保存。実行は`podman run --rm --network=none -v <repo>/tmp/task74/acceptance:/work -v <zellij>:/usr/local/bin/zellij:ro debian:12-slim /bin/bash /work/acceptance.sh`（実験scriptと同一呼び出し形式）。

**検証手段の分担（TR74-5）**: 副因C（空応答retry）の**必須合格条件はfake backend test**（§3の`remap-sequence.empty-list-panes-retry-in-polls`・`remap-sequence.empty-response-retry-before-verify`。「N回空応答の後に成立」系列を決定的に注入し、中断しないこと・deadline超過でのみfatalであることをfail-first込みで検証）が担う。実機E2Eでの空応答は**観察条件**（発生が非決定的なため）とし、E2Eの合格判定からは除外する。

**(i) reference取得**（前提準備。判定対象外）: `zellij --new-session-with-layout 9-pane.kdl`でsession起動し、tab "9P-1"のlist-panes幾何（`pane_x/pane_y/pane_rows/pane_columns`）をtag `a_before`で保存（実験aと同一手順・結果の再利用可）

**(ii) 単tab適用（TASK-74本体のacceptance）**: seed-1tab.kdl（bar付き1 tab）で起動 → `new-pane`×8で計9 terminal pane → `zelper --session S remap --path 9-pane.kdl --json`

- **必須合格条件**（すべて満たすこと。1つでも欠ければacceptance失敗）:
  - exit 0・`ok:true`（zelper verification pass）
  - terminal pane数が**9のまま**（18化していない。主因A修正の直接検証）
  - 9 paneの幾何（(y,x)昇順に並べた`x/y/rows/cols`列）がa_beforeのtab 1分と一致（3x3 grid再現）
  - compact-bar paneがy=59・rows=1・cols=200に生存（grid内に紛れ込んでいない。主因B修正の検証）
  - dump-layoutの形状が入れ子3x3+bar
- **失敗条件**: 上記のいずれか不成立、またはremapがerror envelopeで終了（class問わず）

**(iii) k=3 smoke（companion移動経路の実機確認）**: 9-pane.kdlで27 pane起動 → remap

- **必須合格条件**: exit 0・`ok:true`・27 pane生存・3 tab×各9 terminal pane・各tab幾何が3x3・anchor tabのbar保持
- **観察条件**（合格判定外。結果を記録する）: j>=1 tabのbar新規spawnの有無（生成KDLのbar leafが新規tabへspawnされるはずだが、本項目はTASK-75の前置観察。spawnされない場合も失敗とは扱わず、実測を記録する）・実行中の空list-panes応答の発生有無（発生した場合は非中断で完了したことを記録。未発生の場合は「副因Cはfake backend testで担保」の旨を記録）
- **失敗条件**: 必須合格条件のいずれか不成立

## 5. DD-10 v2.1改訂の対応表（detailed-design.md本体への反映内容）

| DD-10節 | 改訂 |
|---|---|
| 10.1 | 事実5にF1/F2（terminal_command=invoked_with序列化・pane_command=前景process）の区別とF4（RunCommand等価はcwdを含む。絶対cwd明示のみ検証済みで相対・継承・正規化差異は未検証〔TR74-7〕）を追記。E6由来の「command+args一致」文言を「command+args（+起動時cwd一致。無指定同士は一致）」へ精密化 |
| 10.6 | (iii)のslot順非保証例外に、requirements.md §2.6側の例外明記（TR74-3・2026-09-15）への参照を追記 |
| 10.8 | occupied slot規則をterminal_command基準へ全面改訂（§2.1）。template children置換規則を追加（§2.2） |
| 10.7 | 空応答のlenient取得の節を新設: 対象（polling closure〔probe・5-a/5-b〕・step 1 snapshot・10.9検証冒頭。`list_panes_lenient`/`list_tabs_lenient`）・deadline（10s不変・C3規則も不変）・fatal条件（空応答のdeadline超過継続・非空parse失敗・非zero exit）（TR74-6反映） |
| 10.10 | dry-runのsource一覧にinvoked_withを追記 |
| 10.13 | §2.6の制限を追記（cwd明示pane〔表現差異の検証状況分解を含む〕・hold flag・EditFile pane・quoting対象変更・同一command複数paneの要件例外参照） |

なおTR74-3の要件側例外明記はrequirements.md §2.6へ直接追記している（DD-10改訂ではないが対応表に含める）。

## 6. 作成先の選択と根拠

本設計書をdocs/design/配下の新規file（本file）とし、規範仕様の変更はDD-10本体重丁（§5）とした。根拠: DD-10はremap仕様の正本であり、照合key変更はDD-10.8の規則そのものの改訂（正本に誤った規則「pane_commandがSomeなら注入」を残せない）である一方、test-first計画・E2E手順・証拠Inventoryはtask固有の記録でありDD-10の構成（仕様章）に属さない。分離によりDD-10は規範として簡潔を保ち、本fileがTASK-74の根拠（spike S・実験a/b/c）と手順を担う。先例: DD-10 v2改訂（TASK-37）もdetailed-design.md本体改訂+設計記録の分離構成。

## 7. 未判明・判断保留事項

設計が依存する未知事項（F3/F4/F5/F6）はspike S（1/3回・残り予算2回）で実証済み。**意図的に未検証のまま残す事項（TR74-7）**: cwd表現の差異のうち相対cwd・layout継承cwd・`zellij run --cwd`の正規化差異（§2.6。絶対cwd明示のみF4で実証済み。同じ照合外れ経路と推定し、検証(b)の事後検知で担保）。実装段階で新たな未判明が生じた場合は追加実験（最大2回）またはorchestratorへの報告で対応する。
