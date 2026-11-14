# zelper Test Plan (Phase 5)

作成日: 2026-08-21
前提: detailed-design.md DD-1〜DD-12（DD-10はv2全面改訂。TASK-37設計レビューR1〜R9反映済み）
原則: 安定した純粋ロジック（selector・planner・parser）については実装前にfailするテストを書く（development-plan Phase 5）
更新: 2026-09-13（TASK-58条件書移行第4段）: §2の条件定義本文を`tests/design/`の条件書へ移譲し対応表へ縮小。§2.8・§2.11・旧§3は§3「L4統合test」節へ集約

## 1. テスト階層と実行環境

| 階層 | 対象 | 実行環境 | zellij依存 |
|---|---|---|---|
| L1 unit | selector / planner / KDL parse・生成 / JSON serialize | `cargo test`（in-module + tests/unit） | なし |
| L2 fake-backend | ZellijBackend traitのfakeによる多段操作・部分失敗 | `cargo test`（tests/fake_backend） | なし（fake） |
| L3 CLI契約 | 引数parse・help・exit status・stdout構造・fake shim呼び出し列 | `cargo test`（tests/cli、`assert_cmd`） | fake zellij shim（test実行時にtests/cli/shim_common.rs正本から生成。argvを記録しfixtureを返す実行可能script） |
| L4 統合 | 実zellij 0.44.3での実挙動・preservation実証 | podman sandbox（debian:12-slim + ホストバイナリ ro mount、--network=none、config.kdl配置でSetup Wizard抑止、`script -qec` PTY駆動） | あり |

L4の実行形: Phase 1と同一のpodman構成（docs/research/zellij-capabilities.md §3）。テストランナーはコンテナ内で (1) session起動 (2) zelperバイナリ実行（/workにmount） (3) heartbeat / list-panes --json で検証 (4) trapでsession全kill。**実行環境のホストでzellijは実行しない**。

## 2. 領域別テスト仕様（正本: tests/design/の条件書）

領域別の検証条件の正本は `tests/design/<stem>.toml`（test-structure skill準拠）。条件id・given/expect・出典（source欄）・excluded記録は各条件書を参照。条件書一覧・R番号→条件id対応表は `tests/design/README.md`。

**R番号（remap v2 matrix R1〜R50）の定義正本は条件書へ移譲された**（各条件のsource欄・given/expect。旧§2.7本文は上記更新で削除）。R番号で来た参照は`tests/design/README.md`の対応表経由で条件へ到達できる。

TASK-74改訂（2026-09-15）: R13/R14/R15/R18/R20/R48/R49の定義はremap照合key変更（terminal_command〔invoked_with序列化〕基準・quoting warning対象変更）・default_tab_template反映・dry-run出力改訂に伴い条件書側で更新済み（該当: layout-generator.r13〜r15/r18・layout-generator.template-children-substitution・layout-planner.r20系・layout-planner.run-derives-from-invoked-with・remap-cli.r48/r49。設計書 remap-v2-matching-fix-task74.md §2/§3）。§5等の歴史的参照（fail-first履歴）は改訂前定義を指す。

TASK-87/88改訂（2026-09-26・verb拡張の条件書）: `watch`（TASK-86）・`attach`/`detach`/`add session`（TASK-89）に続き`move`（TASK-88）の条件書tests/design/move.toml（22条件）を追加。L2はtests/fake_backend/move.rs（fake_move binary。Cargo.toml [[test]]登録）でnative系argv形式・edge no-op検出（M1基準値）・companion系実行sequence（probe→break-id）・pane交換の順序規則（singleton判定）・部分失敗報告・tab交換の隣接transposition合成（2d-1回・M2基準値）・postcondition検証（pane id不変=M8基準値・position交換）を検証する。polling timeout系条件（to-tab-poll-timeout・swap-partial-failure）は10s deadlineを実時間で消費する（fake_remap r36と同一方式）。L3はshim拡張: FAKE_FAIL_MOVE_PANE / FAKE_FAIL_MOVE_TAB（非zero終了のexit 5分類）・FAKE_TABS_JSON（4-tab応答のmove-tab状態管理）・FAKE_LOG全argv記録（argv列担保）。cli-grammar.tomlへmove系5条件（parse排他・native argv/no-op JSON・fail exit 5・tab swap argv列・help出現）・docs-verb.tomlへ2条件（配布物のmove文法・verb set v2=12記載）を追加（TASK-88がwatch/attach/detach分のhelp出現・配布物条件も継承）。実測との対応: M1→move.pane-*のfake基準値・M2→move.tab-swap-argv-sequence・M8→move.companion-postcondition-verification-failed等・M9（acceptance）→§3.3記録。

旧節番号（§2.1〜§2.11）は条件書source欄のlocaterとして下表の行で存続させる:

| 旧節 | 領域 | 条件書 | 主なtest target |
|---|---|---|---|
| §2.1 | CLI文法・排他・PANESPEC・session解決・completion | cli-grammar.toml | cli_list_read_send |
| §2.1（list系） | list表示 | list-display.toml | cli_list_read_send + lib |
| §2.2 | target resolver | selector.toml | selector |
| §2.3 | backend parser | backend-parser.toml | parser |
| §2.4 | read / send | read-send.toml | cli_list_read_send |
| §2.5 | rename | rename.toml | fake_rename_add_remove |
| §2.6 | resize | resize.toml | fake_resize |
| §2.7 (a)(b) | remap planner / 生成KDL | layout-planner.toml / layout-generator.toml | layout_planner / layout_generator |
| §2.7 (c) | permissions seed | companion-seed.toml | companion_seed |
| §2.7 (d)〜(f)(j)・(i)実行側（R47・R50のL2側） | remap実行sequence | remap-sequence.toml | fake_remap |
| §2.7 (g)(h)(i) | remap CLI契約 | remap-cli.toml | cli_remap |
| §2.9 | failure injection（L2/L3分） | 各機能の条件書へ分散 | - |
| §2.10 | JSON contract | json-contract.toml | error_map + cli_list_read_send |
| §2.8 | L4 integration harness | （条件書なし。L4節へ集約。remap-sequence.tomlの[[excluded]]が参照） | L4（repo管理外） |
| §2.11 | compatibility（L4任意） | （条件書なし。L4節へ集約。remap-cli.tomlの[[excluded]]が参照） | L4（repo管理外） |
| §3 | preservation実証（L4） | （条件書なし。L4節へ集約。remap-sequence.tomlの[[excluded]]が参照） | L4（repo管理外） |

- L4でしか検証しない要求（§2.8・§2.11・§3）は§3「L4統合test」節へ集約し、該当条件書の`[[excluded]]`から参照行で指す
- 条件化しない要求（未test・test支援資産の能力等）は各条件書の`[[excluded]]`に記録
- test file冒頭のheader comment（`// L1: target resolver（test-plan §2.2）`等）は、参照先の節番号が対応表として存続するため更新を必須としない。新規test fileは条件書を参照するheaderとする

### 2.9 failure injection: L3 shim envとL2 FakeBackend注入APIの語彙対応（TASK-83）

failure injection条件は各機能の条件書へ分散するが、同一の失敗・黙失敗をL2（FakeBackendへのprogrammatic注入）とL3（shimのFAKE_* env注入）でどう担うかの語彙対応は本表で担保する（機構分類・命名規則の正本はtests/cli/shim_common.rs doc comment）:

| 分類 | L3 shim env（tests/cli/shim_common.rs） | L2 FakeBackend API（tests/fake_backend/fake.rs） |
|---|---|---|
| 対象指定の失敗注入（rc=1 + stderr） | `FAKE_FAIL_<ACTION>`系（FAKE_FAIL_DETACH / FAKE_FAIL_LIST_TABS / FAKE_FAIL_DUMP_SCREEN / FAKE_FAIL_WRITE_CHARS / FAKE_FAIL_WRITE / FAKE_FAIL_SEND_KEYS / FAKE_FAIL_MOVE_PANE / FAKE_FAIL_MOVE_TAB / FAKE_FAIL_DELETE_SESSION / FAKE_FAIL_RENAME_SESSION） | `inject_failure`（op先頭一致で次回呼出を失敗）/ `inject_failure_as`（ErrorClass指定）・`fail_override_layout_nth` / `fail_companion_wasm_extract` / `fail_companion_permissions_seed`等の個別注入 |
| 成功回数超過から失敗（polling・postcheck系列） | `FAKE_FAIL_DUMP_SCREEN_AFTER`（`pane:成功回数`のspace区切りlist） | `dump_fail_after`（watch pollingでのpane消滅）・`fail_list_sessions_after_create / _rename / _delete`（事後list_sessions失敗） |
| 黙失敗（rc=0だが作用なし） | `FAKE_MUTE_DELETE_SESSION` / `FAKE_MUTE_RENAME_SESSION` | `mute_delete_session` / `mute_rename_session` / `mute_create_session` / `mute_rename_tab`等の`mute_*`系 |

## 3. L4統合test（実zellij・repo管理外）

旧§2.8（integration fixtures / sessions）・§2.11（compatibility）・旧§3（preservation実証）を集約したL4要件の正本節。L4は実zellij依存のため条件書の対象外（L1〜L3の自動test対象外。各条件書の`[[excluded]]`から参照行で指される）。

### 3.1 harness構成・要件

- 共通harness: repo管理外の作業dirにpodman実行helper（隔離env生成・config.kdl配置でSetup Wizard抑止・session起動/破棄）。恒久化する場合は`tests/integration/`へ昇格する
- harness要件: 十分な表示サイズ（200x60）を保証（Phase 1副次発見: 狭いとnew-paneが静かに失敗する）
- v2 remapのharness要件: XDG_CACHE_HOMEを隔離envへ指向しcompanion plugin wasmのextract（version入cache path）を観測可能にする・permissions.kdlはsession起動前にseed（E6実証条件。L4 S-v2-1ではsession起動後のruntime seedでも成立を確認）・zellij 0.44.3 + zellij-tile 0.44.3（DD-3.5 compatibility policyの実証済み組合せのみ）

### 3.2 シナリオ・要件

- 主要シナリオ: list/read/send/rename/add/remove/resize/remap各1以上・実行後list-panes --jsonによるpostcondition assert。remapはk=1（plugin不使用）とk>=2（companion setup・probe・cross-tab移動を含む）の両方
- preservation実証（acceptance criteria 7・Phase 7要件）: 「kill/restartではなく同一processが生存」を**pane identity**で証明する（v2は全paneプロセス保存であり再作成paneは存在しないため、旧3'のrecreated pane検証は廃止）:
  1. 対象pane内でheartbeatプロセス（pid・連番を定期的にlogへ書く）を起動
  2. zelper remap実行
  3. 検証: (a) heartbeat logのpidと連番が途切れなく継続、(b) list-panesのpane ID同一・割当instanceのtab所属（DD-10.9 (a)）

  この手法はPhase 1で確立済み（hb.sh）。v2では同一tab適用（worked example #2相当）に加え、cross-tab移動（#3: break-new、#6: break-idによるanchor集中）で実施する。移動前後でpid・pane IDが不変であることを実zellijで証明する（E6実証の再確認）
- compatibility: 実行時要件はzellij >=0.44.3。実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3のみ（DD-3.5 compatibility policy）。0.44.4以降など未実証組合せは、probe pipe（R30/R31）とpostcondition検証（R37〜R41）が安全網として機能することをfake（L2）で検証する。実機確認はzellij新seriesサポート時にtile pin更新とセットで実施（P1）

### 3.3 実施記録（repo管理外の検証作業dirに残置）

- **S1〜S11全PASS**（修正後再実行含む。remap保存はheartbeat pid継続+pane ID同一で実証、overflow tabs modeの再作成paneのcommand一致も検証、plugin config子nodeのslot除外はbare/wrapper両形式で検証）。**旧remap仕様v1による実証**であり、v2の再実証はS-v2-1〜6（cross-tab移動保存・permissions seed・probe込み）として実施済み（requirements-traceability §6）。v2.2（multi-tab layout全体再現・TASK-75）の実証はS-v3-1〜5（下記）
- **S-v3-1〜5全PASS**（TASK-75 remap v2.2・2026-09-16。手順は設計書 docs/design/remap-v3-multi-tab-task75.md §5.2・改修経緯は§7.1。成果物: `tmp/task75/acceptance/`〔verdict.txt・acceptance.sh・採取panes/tabs JSON+TSV・dump・remap JSON〕。podman sandbox〔§1構成〕container通算15回）:
  - S-v3-1 = (i) 単tab回帰 7条件: TASK-74 (ii)手順（9-pane-single.kdl）の再実行。terminal 9 pane・幾何reference一致・bar（y=59 rows=1 cols=200）・dumpのsplit nesting一致。T=1後方互換の実機確認（focus-pane-id 1回が増えること込み）
  - S-v3-2 = (ii) 異構成multi-tab M=12（ref-hetero.kdl・T=3〔9+1+2 slot〕・S=12・k=1）18条件: 3 tab生成・tab名（anchor=seed名保持・T-single〔鋳型名〕・base名〔名無し鋳型〕）・active tab=anchor（focus=true鋳型T-3x3がblock 0 tab 0=anchor）・per-tab幾何reference一致（3x3/1 pane/左右2分割・anchor含む全生成tab）・bar全3 tab・各tab pane focus slot 0・source pane全生存
  - S-v3-3 = (iii) 空groupのnew-tab経路 M=3（S=12 > Mでtab1/tab2が割当0件の空group）16条件: new-tab --layout-string経路の実証。bare spawn込み総terminal 12 pane・tab名・per-tab幾何・既定paneのN_t正規化（tab1=1・tab2=2）・bar全tab・pane focus（空group tab含む）
  - S-v3-4 = (iv) M>S block反復 M=15（k=2・6 tab）26条件: block 1生成tabの`-2`接尾・block 1空group2件のnew-tab経路・全6 tab幾何（block 1含む）・bar全6 tab・pane focus位置ベース（block 1空group tab含む全生成tab）・15 source pane全生存（総24 pane）
  - S-v3-5 = (v) tab focus決定則matrix 3系列 13条件: 系列a=focus=true鋳型が文書順2番目（T-single）→ active=T-single（anchorでない新規tab側）・系列b=複数focus=true（tab0+tab2）→ 文書順最初（=anchor）採用+複数focus検出warning出力・系列c=focus指定なし→ anchor復帰。各系列exit 0・ok:true・active tab期待どおり・pane focus
  - 改修経緯（§7.1）: 1巡目は55 PASS/18 FAIL。要因A（focus-pane-id対象をmapping pane idから適用後のlist-panes位置〔visual order s番目〕ベースへ変更・occupied限定撤廃・already focused exit 2は成功扱い）と要因B（空groupをnew-tab --layout-string経路へ変更・step 6のoverride-layout skip。実験4で経路確定）を改修し全PASS。vb系列は複数tab focus鋳型検出warningが未実装の退化（1巡目vb PASSはalready focused警告の偶発的担い）で、normalize_tab_templatesへの複数focus warning追加+stderr/JSON data.warnings伝達で解消（条件: remap-cli.preflight-warning-paths-stderr-and-json）
  - harness: acceptance.sh（5系列・judge採点。TASK-74 harnessと同一構成）・musl static buildのzelper binary・geom75.awk（list-panes JSON→TSV幾何抽出）・tabs.awk・reference幾何file（ref_geom_*）
  - 既知の検証限界: dump-layoutとの完全一致は検証対象外（比較はtab毎slot数・幾何・bar存在・is_focused/activeに限定。dump除外項目対応表は設計書§4.3）・anchor tab名は保持例外（layout最初のtab名と一致しないことを確認するのが条件）・pane title（name属性→title）は保証外・pane idは検証の手段であって比較対象でない（生存検証はremap前後のid集合一致で実施）
- **TASK-81 remap照合修正E2E 3巡全項目合格**（2026-09-19〜20。cwd注入・reconciliation多段規則・dry-run floating緩和・closed_duplicates/source_excluded_panes報告の実機実証。一次記録: `tmp/tasks/tm-81/`〔t0p3_/t0q3_/t0s3_/t0k3_/t0s3b_〕。第1回でF1/F2、第2回でR1〜R3を検出し修正、第3回で全項目合格・新規不具合なし。podman sandbox〔§1構成〕）:
  - t0p3 = 同一cwd shell pair構成（R1実証）: anchor dup close+cwd反転でexit 0達成・新規id pane残存ゼロ・heartbeat単速（二重稼働なし）・幾何一致・closed_duplicates 2件（slot/reason付き）
  - t0q3 = quote pane・短命command混在構成: held化短命command dupのclose回収（gate拡張+argv一致逆引き）・anchor解消・closed_duplicates 5件・幾何一致
  - t0s3 = 短命command新規系列（R2実証）: 対象tab内source外held paneの`source_excluded_panes`報告（成功data・error.data・human warning 3経路）・closeなし（kill制約どおり）・2回remap通過で当該pane id不変
  - t0k3 = 回帰系列: k>=2・reconciliation不発動・exit 0・幾何一致の完全維持
  - 全巡でdry-run緩和（floating保有でもexit 0・仮想embed計画・状態不変）と`--embed-floating`なし実行のPreflight error維持を確認
  - 既知の検証限界（TASK-81で確定。DD-10.13幾何検証の対象外）: bare pane群（shell pane・空slot正規spawn）のswap・配置は検証(e')の位置照合対象外（run一意割当のみ検査。E2E幾何比較でのみ観察可能）・対象tab内に残留するsource外held pane（`source_excluded_panes`報告対象）は当該tabの幾何を巻き込みうるが、巻き込み自体は検証対象外
- 既知の検証限界: zellij 0.44.3では空tabを作成できない（new-tabが必ずpaneを作る）ため、`remove tab --empty`の実削除は代替検証（dry-run計画・非空tab保護・error契約）のみ
- **TASK-88 move系実測（M1/M2/M8・2026-09-25・podman sandbox〔§1構成〕。一次記録: `tmp/tasks/tm-88/`）**: M1（`m1_result.log`・`m1c_result.log`）= `move-pane <dir> -p`はslot幾何完全不変のpane id付け替えswap・`-p`で非focus paneを移動可（focusは元paneに追従）・bare `move-pane -p`はrotate forwards・`move-pane-backwards -p`はサイクル逆回し・全操作でpane id/tab id不変・floating paneはrc=0の座標shift（slot swapでない）・rotateはno-op・edge方向移動はsilent no-op（rc=0・出力空・幾何不変）。M2（`m2_result.log`・`m2b_result.log`）= `move-tab <left|right>`はdirection positional必須・`-t <tab_id>`が存在し非active tabをfocus不変で1 position移動・隣接tabとは1回で純swap（A0B1→B0A1）・edgeはwrap（全体rotation）・tab id/name不変。M8（`m8_result.log`）= 方向・rotate系全操作でpane id・title不変
- **TASK-88 M9 E2E acceptance（2026-09-26・podman sandbox・zellij 0.44.3 + musl static zelper実binary。一次記録: `tmp/tasks/tm-88/m9_result.log`・`m9_podman_run.log`〔M9〕・`m9b_podman_run.log`〔M9b〕。5-tab layout〔A/B各2 pane・C/D各1 pane・P=terminal 1+tiled compact-bar 1〕）。S1〜S4・S3bは全PASS。S5・M9bは観察記録（PASS/FAIL判定でなく自動close基準の実証観察）**:
  - S1 break-id単独（`zelper move pane terminal_0 --to-tab B`）: rc=0・移動先tab所属・pane id不変（zellerのpostcondition検証込み）。focus観察: active tabは移動先へ遷移・移動先tab内のpane focusは移動paneでない（should_change_focusはtab単位の遷移。zellerはpane focusを保証しない——設計どおり対象外）
  - S2 pane swap 2連発（両tab非singleton）: rc=0・両pane交換・pane id不変・両tab残存
  - S3 両singleton tabのswap: exit 7で事前拒否（hint文面を確認）
  - S3b 片方のみsingletonのswap: rc=0・順序規則（非singleton側が先）で移動元tab自動close回避・両tab残存
  - S4 tab交換（`zelper move tab A --swap-with D`）: rc=0・position 0↔3交換・tab id/name不変（id基準追跡）
  - S5（観察）: 最後のselectable pane（terminal）が抜けたtabは、tiled non-selectableなplugin pane（compact-bar）のみ残っていても自動closeした（実証された構成）
  - **M9b（REVIEW-3指摘7追加実測・観察）**: tiledなselectable plugin pane（`zellij:link`・is_selectable=true）のみ残るtab（layoutで明示的に配置）は自動closeしなかった。S5とM9bの対により**zellijのtab自動closeは「selectable pane不在」基準であることが確定**し、設計§3.3・実装のsingleton判定（selectable基準）と一致する（順序規則はこの基準の下で移動元tabのselectable空虚を構造回避している）
  - 副観察: 新規sessionにzellij:link pane（selectableなplugin pane）が現れる場合がある——zelperのsingleton判定はselectable pane全数（plugin込み）を数えるため保守側（tabを非singleton扱い）に働く。M9bによりselectable pluginの残存するtabはcloseされないことが実証済みのため、この保守側判定は安全方向であることも実裏付けられた

## 4. tests/構成（tests/README.mdで管理。MR-8で実態に合わせ更新）

```text
tests/
  README.md                      # 本構成の索引（file→条件書）
  design/                        # テスト設計条件書（検証条件の正本。README.mdに一覧・R番号対応表）
  unit/                          # L1純粋ロジック（selector / parser / error_map / layout_planner / layout_generator）
  fake_backend/                  # L2（fake.rs = FakeBackend本体 + 各領域テスト）
  cli/                           # L3（assert_cmd。fake zellij shimはテスト実行時に生成）
  fixtures/
    zellij/                      # 実出力JSON/text fixture
```

検証条件の正本は`tests/design/`の条件書（§2対応表参照）。testと条件の紐付けは`// [covers:<id>]`tagであり、`mise run verify-conditions`（scripts/design/verify-conditions.py）がid対応・網羅・付け漏れを機械検証する。

L4統合テストはrepo管理外の作業dir（エフェメラルharness。podman構成は§1・要件は§3）で実行し、結果を記録する。恒久化する場合は`tests/integration/`へ昇格する。

## 5. 実装前テスト（fail-first対象）

v2実装（TASK-39）開始時に最初に書くテスト（実装前にfailすることを確認）:

1. remap plannerのgrouping matrix（§2.7 (a) R1〜R12）
2. 生成KDLの全条件（§2.7 (b) R13〜R20）
3. permissions seed判定（§2.7 (c) のうち内容→action決定の純粋部分: 無変更/権限追記/純append/Preflight分岐）

初回実装（Phase 6a）のfail-first対象（selector・parser・error対応）は実装済みで、v2後もそのまま有効（§2.2/2.3/2.10）。うちversion判定のみ0.44.3基準へ更新する（§2.3・R46）。既存テストのうちremap旧仕様テスト（fill/nest/tabs・session-scope・再作成pane・cwd注入）は、旧§2.7冒頭の経過措置どおりv2実装（TASK-39）で置換完了（旧仕様テストは削除済み）。

（履歴note: §2.7の条件定義本文はR番号とともに条件書へ移譲済み。上記R番号は`tests/design/README.md`対応表で条件idへ解決できる）
