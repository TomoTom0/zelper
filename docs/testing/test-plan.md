# zelper Test Plan (Phase 5)

作成日: 2026-08-21
前提: detailed-design.md DD-1〜DD-12（DD-10はv2全面改訂。design-review DR-1〜DR-10・TASK-37設計レビューR1〜R9反映済み）
原則: 安定した純粋ロジック（selector・planner・parser）については実装前にfailするテストを書く（development-plan Phase 5）
更新: 2026-09-13（TASK-58条件書移行第4段）: §2の条件定義本文を`tests/design/`の条件書へ移譲し対応表へ縮小。§2.8・§2.11・旧§3は§3「L4統合test」節へ集約。移行仕様は`docs/design/conditions-migration.md` §6

## 1. テスト階層と実行環境

| 階層 | 対象 | 実行環境 | zellij依存 |
|---|---|---|---|
| L1 unit | selector / planner / KDL parse・生成 / JSON serialize | `cargo test`（in-module + tests/unit） | なし |
| L2 fake-backend | ZellijBackend traitのfakeによる多段操作・部分失敗 | `cargo test`（tests/fake_backend） | なし（fake） |
| L3 CLI契約 | 引数parse・help・exit status・stdout構造・fake shim呼び出し列 | `cargo test`（tests/cli、`assert_cmd`） | fake zellij shim（tests/fixtures/fake-zellij。argvを記録しfixtureを返す実行可能script） |
| L4 統合 | 実zellij 0.44.3での実挙動・preservation実証 | podman sandbox（debian:12-slim + ホストバイナリ ro mount、--network=none、config.kdl配置でSetup Wizard抑止、`script -qec` PTY駆動） | あり |

L4の実行形: Phase 1と同一のpodman構成（docs/research/zellij-capabilities.md §3）。テストランナーはコンテナ内で (1) session起動 (2) zelperバイナリ実行（/workにmount） (3) heartbeat / list-panes --json で検証 (4) trapでsession全kill。**実行環境のホストでzellijは実行しない**。

## 2. 領域別テスト仕様（正本: tests/design/の条件書）

領域別の検証条件の正本は `tests/design/<stem>.toml`（test-structure skill準拠）。条件id・given/expect・出典（source欄）・excluded記録は各条件書を参照。条件書一覧・R番号→条件id対応表は `tests/design/README.md`。

**R番号（remap v2 matrix R1〜R50）の定義正本は条件書へ移譲された**（各条件のsource欄・given/expect。旧§2.7本文は上記更新で削除）。R番号で来た参照は`tests/design/README.md`の対応表経由で条件へ到達できる（conditions-migration §10.3）。

TASK-74改訂（2026-09-15）: R13/R14/R15/R18/R20/R48/R49の定義はremap照合key変更（terminal_command〔invoked_with序列化〕基準・quoting warning対象変更）・default_tab_template反映・dry-run出力改訂に伴い条件書側で更新済み（該当: layout-generator.r13〜r15/r18・layout-generator.template-children-substitution・layout-planner.r20系・layout-planner.run-derives-from-invoked-with・remap-cli.r48/r49。設計書 remap-v2-matching-fix-task74.md §2/§3）。§5等の歴史的参照（fail-first履歴）は改訂前定義を指す。

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
- 条件化しない要求（未test・test支援資産の能力等）は各条件書の`[[excluded]]`に記録（conditions-migration §8）
- test file冒頭のheader comment（`// L1: target resolver（test-plan §2.2）`等）は、参照先の節番号が対応表として存続するため更新を必須としない。新規test fileは条件書を参照するheaderとする

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
- 既知の検証限界: zellij 0.44.3では空tabを作成できない（new-tabが必ずpaneを作る）ため、`remove tab --empty`の実削除は代替検証（dry-run計画・非空tab保護・error契約）のみ

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
