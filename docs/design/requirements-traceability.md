# Requirements Traceability (Phase 2)

作成日: 2026-08-21（Phase 8で最終監査により更新。2026-09-05: remap v2実装・L4実証を反映）
対象: requirements.md の全要件を Phase 1調査結果（docs/research/zellij-capabilities.md、zellij 0.44.3実機）と突き合わせて分類する

## 1. 分類定義（development-plan.md Phase 2による）

- **direct**: zellijの公開プリミティブ1回で直接実現される
- **orchestration**: 公開プリミティブの組合せ（状態取得→計画→複数操作→検証）で実現される
- **approximate**: 実現可能だがZellij側の制約により近似・限定付き
- **not possible**: 公開インターフェースでは実現不能。最も近い設計を併記

## 2. 要件分類（requirements.md章立て順）

### 2.1 構造のinspection

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| sessions一覧 | approximate | `list-sessions`にJSONなし。`-n`テキストparseで対応（capabilities §2） |
| tabs一覧・metadata | direct | `list-tabs -a --json`（TabInfo全field） |
| panes一覧・ID・metadata・geometry | direct | `list-panes -a --json`（pane_command/pane_cwd/geometry含む） |
| 名前付きlayout一覧 | approximate | zellijに「layout一覧」APIなし。layout_dirのファイル列挙をzelperが行う |
| 機械可読出力 | approximate | tabs/panes/subscribはJSONあり、sessionsはテキストのみ |

### 2.2 read

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| 単一pane読み取り | direct | `dump-screen -p` |
| 複数pane読み取り | orchestration | 複数`dump-screen`の集約・区切り表示 |
| visible vs scrollback | direct | `-f`フラグ |
| 複数pane時のper-pane失敗識別 | orchestration | zelperの結果集約 |

### 2.3 send

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| text送信（暗黙Enterなし） | direct | `write-chars -p` |
| 明示Enter | direct | `write -p 13` / `send-keys -p Enter` |
| key送信 | direct | `send-keys -p`（keyごとに独立引数） |
| 複数pane/broadcast | orchestration | 対象ごとの`-p`指定ループ+結果集約 |
| 曖昧ターゲットのerror | orchestration | zelper selectorの設計対象（zellij側機能に依存しない） |

### 2.4 rename

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| pane rename | direct | `rename-pane -p`（undo系あり） |
| tab rename | direct | `rename-tab -t` / `rename-tab-by-id` |
| 複数ターゲット | orchestration | ループ+集約 |
| 生成/パターンベース命名 | orchestration | zelper側で名前生成 |

### 2.5 resize

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| 特定paneのresize | direct | `resize <increase\|decrease> <dir> -p` |
| equalize | approximate | 反復directional resize+`list-panes -g`収束判定で近似。**no-op場合あり・刻み幅非公開**のため完全一致を保証しない |
| 相対/パーセント指定 | v1では未提供 | パーセント/相対指定の文法はv1に存在しない。equalizeとdirectional stepが代替（MRレビューで分類修正） |
| tabスコープの一括resize | orchestration | pane集合への逐次適用 |

### 2.6 remap（コア）

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| 既存terminal paneのprocess保持再配置 | orchestration | 同一tab内の配置は`override-layout --apply-only-to-active-tab --retain-existing-terminal-panes`のrun一致照合（各slotに既存paneと同一の`command`+`args`を記述）で決定論化（E6）。cross-tab移動はcompanion plugin（`break_panes_to_*`）で構成。プロセス保存を実証済み（E6・L4 S-v2-1: pid継続）。v2.2（TASK-75）: layout全体（T tab鋳型）の再現へ一般化し、空groupの新規tabは`new-tab --layout-string`経路で具体化（L4 S-v3-1〜5: 80条件全PASS） |
| layout name指定 | direct | bare name（拡張子なし）がlayout_dir解決される |
| path / inline指定の明示的代替 | direct | positional=path、`--layout-string`=inline（**改行区切りKDL必須**） |
| overflow: 追加tabへのlayout反復 | orchestration | companion pluginの`break_panes_to_*`によるプロセス保存のcross-tab移動で実現（E6実証）。v2.2（TASK-75）は反復単位をlayout全体へ一般化: S = sum(N_t)（T tab鋳型のper-tab slot数和）として k = max(1, ceil(M/S)) block × T tabを生成（DD-10.6 v2.2）。L4で実証（S-v2-1: 9 pane → k=3・全pid保存。S-v3-4: M=15 > S=12 → k=2・block 1 tabの`-2`接尾・6 tab・15 pane全生存）。v1時点の「プロセス保存付きでは不可」はCLI primitiveのみに基づく旧判定（§4-1参照） |
| multi-tab layout全体再現（tab名・focus・bar。2026-09-15要件追記） | orchestration | v2.2（TASK-75・DD-10 v2.2）: layoutの全tabを正規形TabTemplate列の鋳型として展開し、tab名（名前あり鋳型名/base幹 + `-<b+1>` block接尾。anchor名保持例外）・tab focus（focus=true鋳型のblock 0 tabへ最終go-to・複数指定は文書順最初+warning）・pane focus（`focus-pane-id`・適用後list-panes位置ベース）・`default_tab_template`由来bar（全生成tabの生成KDLへ維持）を`zellij --layout`起動時と同一に再現。同一性の例外（anchor名・名無しtab自動名・pane title・pane id）は要件§2.6・DD-10.13に明示。T=1は計画レベルで後方互換。L4 S-v3-2〜5で実証（条件書: TASK-75新規22条件〔layout-planner 8・layout-generator 1・remap-sequence 9・remap-cli 3・backend-process 1〕） |
| pane順序の決定性 | approximate | 割当順はvisual order（tab position → y → x）で固定。group（block×tab所属）とrun（command+args）に一意なpaneのslot対応は決定的。同一runの複数pane間・複数shell pane間のslot順は保証外（要件の決定論性からの明示例外・v2.2でgroup定義をblock×tabへ一般化。DD-10.6） |
| plugin paneの明示扱い | orchestration | `is_selectable`/`plugin_url`で判別しslot勘定から除外。companion plugin paneはnon-selectableとしてsource・検証からも除外（DD-10.3） |
| dry-run/plan | orchestration | zelperのplan layer（zellij機能に依存しない） |

### 2.7 add / 2.8 remove

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| pane追加（複数含む） | direct / orchestration | `new-pane`（ID返却）。count>1はループ。`--tab-id`で他tab作成可 |
| tab追加 | direct | `new-tab`（tab ID返却、`--layout`/`--layout-string`対応） |
| pane/tab削除 | direct | `close-pane -p` / `close-tab -t` / `close-tab-by-id` |
| 空tab削除 | orchestration | 「空」の定義はzelper設計（plugin paneのみのtab等） |
| 破壊的操作の確認/dry-run | orchestration | zelper safety layer。**存在しないIDへのcloseがexit 0で無操作**のためpostcondition検証必須 |

### 2.9 layout解決

| 要件 | 分類 | 根拠・備考 |
|---|---|---|
| positional=name / option=path,inline | direct | zelperの`--path`→positional path、`--inline`→`--layout-string`対応。排他検証はzelper CLI層 |
| 相互排他の拒否 | orchestration | zelper CLI層の責務 |

### 2.10 CLI構造要件（verb-first・positional-first・共通option・multi-target・JSON・dry-run・completion）

いずれもzelper自身の設計対象でありzellij機能に依存しない → **orchestration（zelper設計）**。Phase 3のdetailed-designで凍結する。

### 2.11 互換性

| 要件 | 分類 | 備考 |
|---|---|---|
| 最小サポートバージョン | 判定済み | **0.44.3**（companion plugin駆動。実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3のみで、0.44.x系列を超えるversionは未実証として拒否）。`override-layout`/`list-panes --json`/`--pane-id`全系は0.44.0〜 |
| version/feature検出 | direct | `zellij --version`文字列parse |
| 必要feature不在時の挙動 | orchestration | zelper capability detectionの設計対象 |
| 公開CLI以外へのアクセス | 原則不要 | 全要件が公開CLI (`action`/`list-sessions`/`setup`) で構成可能 |

### 2.12 安全性

| 要件 | 分類 | 備考 |
|---|---|---|
| preflight/計画/順序/部分失敗報告 | orchestration | zelper plan layer |
| rollback | approximate | zellijにatomic rollbackなし。best-effort（planの逆操作）+部分失敗の明示報告に限定し、**atomicityを主張しない**（要件8の指示どおり） |

## 3. P0要件 → detailed-designセクションマップ

detailed-design.md（Phase 3成果物）の章番号を以下のように規定し、P0要件を割り当てる:

| DD章（規定） | 内容 | 対応P0要件 |
|---|---|---|
| DD-1 | CLI grammar（凍結verb set・構文木・target option・排他規則・completion） | verb-first/小語彙/positional-first/共通target option/multi-target/shell completion/help |
| DD-2 | domain/state model（SessionRef/TabId/PaneId/Geometry/PaneKind/LayoutRef） | state discovery・ID/metadata |
| DD-3 | Zellij backend interface（typed adapter・capability detection・subprocess規則） | robust discovery・version検出・feature不在時挙動 |
| DD-4 | output contracts（human/JSON・stable fields） | JSON output |
| DD-5 | `list`操作設計 | sessions/tabs/panes/layouts一覧 |
| DD-6 | `read`操作設計 | 単一/複数読み取り |
| DD-7 | `send`操作設計 | text/key/broadcast・per-target報告 |
| DD-8 | `rename`操作設計 | pane/tab rename |
| DD-9 | `resize`アルゴリズム | 高位resize・equalize・収束と終了条件 |
| DD-10 | `remap`アルゴリズム（v2: companion plugin・配置・移動・検証） | preservation・overflow・順序・plugin扱い |
| DD-11 | `add`/`remove`操作設計 | 追加/削除・空tab cleanup |
| DD-12 | safety model（dry-run・確認・部分失敗・error分類/exit status） | dry-run・clear error handling |

Acceptance criteria（§9の14項）の最終監査は §6に実施した。

## 6. Phase 8 最終監査（P0要件・AC × 設計・実装・テスト）

分類: implemented / implemented with documented limitation（文書化された制限付き）/ not implemented。
テスト階層: L1 unit / L2 fake-backend / L3 CLI契約（`cargo test`63件）/ L4 実機統合（S1〜S10全PASS。harness・実行記録はrepo管理外の検証作業dirに残置）。remap v2実装後はL2/L3含む110テスト全green・L4 S-v2-1〜6全PASS（TASK-39。記録は`tmp/260904_l4_harness_39/`・`tmp/260904_l4_podman_run*_39.log`）。remap v2.2（TASK-75・multi-tab layout全体再現）実装後はcargo test 171件全green・L4 S-v3-1〜5全PASS（80条件・2026-09-16。記録は`tmp/task75/acceptance/`）。

| 要件（requirements.md） | 設計 | 実装 | 自動テスト | L4 | 分類 |
|---|---|---|---|---|---|
| state discovery・ID/metadata（§2.1） | DD-2/3/5 | app/list.rs, zellij/parser.rs | L1 parser・selector / L3 list | S1 | implemented |
| layout discovery/resolution（§2.9） | DD-3.4/10.2 | layout/mod.rs | L1 KDL slot count / L3 排他 | S7/S8/S10 | implemented |
| read 1/多（§2.2） | DD-6 | app/read.rs | L3 | S2 | implemented |
| send 1/多 broadcast（§2.3） | DD-7 | app/send.rs | L3 | S3 | implemented |
| rename pane/tab（§2.4） | DD-8 | app/rename.rs | L2 | S4 | implemented |
| add/remove pane/tab（§2.7/2.8） | DD-11 | app/add.rs, app/remove.rs | L2 | S5 | implemented（空tab実削除は代替検証のみ: zellij 0.44.3で空tab作成不能。MR-14） |
| 高位resize（§2.5） | DD-9 | app/resize.rs | L2 | S6 | implemented with documented limitation（近似・完全幾何保証なし） |
| remap保存（§2.6） | DD-10（v2.2） | app/remap.rs, layout/mod.rs, companion.rs, plugin/ | L1〜L3 remap v2 matrix R1〜R50・dry-run + TASK-75新規22条件（layout-planner 8・layout-generator 1・remap-sequence 9・remap-cli 3〔E2E vb回帰1込み〕・backend-process 1） | S7・S-v2-1（heartbeat pid継続+pane ID同一で実証）・S-v3-1〜5（multi-tab全体再現80条件・15 pane生存） | implemented（v2.2は全paneプロセス保存。kill/restart経路が存在しないため要件93を無条件で充足） |
| remap overflow（§2.6） | DD-10.6/10.7（v2.2） | 同上 | L2/L3 R3〜R7・R29〜R36 + TASK-75新規: layout-planner.multi-tab-allocation-cumulative-slots・k-uses-total-slots・tab-names-from-templates-and-block-suffix・empty-group-planned-for-all-blocks-tabs・remap-sequence.move-phase-generalized-block-tab-order・empty-group-tab-created-via-new-tab-then-rename | S-v2-1（9 pane→k=3・全pid保存）・S-v3-3/S-v3-4（空group new-tab経路・M=15→k=2 block反復） | implemented（E6+companion pluginで旧制約を解決。§4-1） |
| JSON出力（§3.6） | DD-4 | output/json.rs | L1 error_map / L3 json contract | S1〜S9 | implemented |
| shell completion（P0） | DD-1.7 | main.rs | L3 completion | - | implemented |
| clear error handling（P0） | DD-4/12 | error.rs | L1対応表 / L3 exit code | S10 | implemented |
| dry-run（§3.7） | DD-12 | remap/remove | L2 / L3 | S8・S5 | implemented with documented limitation（`resize equalize`/`add`はP1: MR-7） |
| AC-1〜6・9〜11・13・14 | 上記対応行と同一 | - | - | S1〜S6, S10 | implemented（AC-14はhelp文言test無し、実装と--help出力で確認） |

**AC-7（kill/recreateなしのremap）**: L4 S7・S-v2-1で実証（保存対象paneのheartbeat pidと連番が途切れなく継続、pane ID同一、geometry変化）。v2には再作成paneが存在しないため、旧S9のrecreated pane検証は廃止。S-v3-2〜4でもsource pane全生存（remap前後のpane id集合一致・最大15 pane）を再実証。
**AC-8（overflow決定性）**: S-v2-1で実証（9 pane→k=3・visual order割当・連番tab）。v2.2（TASK-75）後の実装は反復単位がlayout全体（S=sum(N_t)）へ一般化され、tab名はlayout由来（名前あり鋳型名/base幹 + `-<b+1>` block接尾。anchor名保持例外）へ変更。block反復はS-v3-4（M=15 > S=12 → k=2・block 1 tabの`-2`接尾・6 tab）で実証。v2は要件2.6の文言どおりの実装（§4-1）。旧S9時点の `--overflow tabs` による代替設計は廃止済み。
**not implemented**: なし（P0範囲）。P1（bulk rename・`--no-fill`・equalize/add dry-run・watch等）はrequirements P1リストのまま未実装。

## 4. 制約と設計への指示（impossible要件の代替）

1. **cross-tab pane移動**（旧制約: not possible → **解決・implemented**）: CLI actionには移動primitiveが存在せず（実機確認済み）、override-layout単体でも既存paneは第1tabにしか配分されない（実験E1〜E5で確定）。ただしPlugin API（zellij-tile）の `break_panes_to_*` をcompanion plugin + `action pipe` でCLI駆動すればプロセス保存のcross-tab移動が可能（capabilities §4.1 E6で実証）。zelper v2の実装（detailed-design DD-10）:
   - M pane + N slot → k = max(1, ceil(M/N)) 個のlayout instance反復で全paneを配置（M > N でもerrorなし。v2.2〔TASK-75〕では反復単位がlayout全体へ一般化: S = sum(N_t)・k = max(1, ceil(M/S)) block × T tab。§2.6参照）。kill/restart経路は存在しない
   - instance 0はanchor tab（既定active tab・`--tab`で指定可・tab名保持）。以降は `break_panes_to_new_tab` / `break_panes_to_tab_with_id` で新規tab `<base>-<j+1>`（v2.2: 幹〔名前あり鋳型名/base〕+`-<b+1>` block接尾）へ移動し、command+args一致KDLでoverride-layout適用（run一致照合で決定論的配置）
   - 権限はpermissions.kdl seedで自動化（session起動後のseedも有効: `request_permission`が呼出毎にfileを再読込。L4 S-v2-1で実証）。probe pipeで状態変更前に応答性を確認し、不成立は中断
   - 要件2.6のoverflow（M>N時の追加tab layout反復・プロセス保存）を要件文言どおり実装。要件93（明示選択時以外のkill/再作成の禁止）は「破壊的再作成mode自体が存在しない」ため無条件で充足。v1の代替設計（`--overflow nest|tabs`）は廃止（v0.1.xでoptionごと削除済み）
2. **session一覧のJSON不在**: `list sessions --json`はセマンティクスをzelperが定義したJSONに載せ替える（sourceはテキストparse）
3. **resizeの正確な幾何保証は不可**: 達成可能な近似であることをhelp/JSONに明示する。収束しない目標では反復上限・振動検出で打ち切り、達成幾何とnotes（非収束の旨）を付して成功で返す（DD-9の近似契約。MR-5で確定）
4. **rename-sessionで旧名に戻せない**挙動（exit 0で無効果）が観測済み。zelperはsession renameをP0に含めない（要件もpane/tabのみ）
5. **サイレント成功**（存在しないIDへのclose等がexit 0）: zelperのpostcondition検証で検出し、エラーとして報告する

## 5. Phase 3への引継ぎ事項（完了状況）

- ~~最優先実験: 複数tab layoutのoverride-layout適用~~ → 実施済み。結果はcapabilities §4.1（CLI primitiveのみでは保存付きoverflow不可と確定したが、E6でcompanion pluginによる解決を実証しv2として実装）
- 未実施（v1設計で回避済みのため影響なし）: `paste`実挙動（v1はwrite-charsのみ使用）、resize刻み幅（DD-9の幾何検証で吸収）、stacked panes実挙動（remap対象はtiled terminal paneのみ）
- DD-1〜DD-12の章構成によるdetailed-design.md作成 → 完了
