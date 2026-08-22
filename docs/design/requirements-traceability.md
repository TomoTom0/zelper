# Requirements Traceability (Phase 2)

作成日: 2026-08-21（Phase 8で最終監査により更新）
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
| 既存terminal paneのprocess保持再配置 | direct | `override-layout --apply-only-to-active-tab --retain-existing-terminal-panes`。slot数一致時のプロセス保存を実証済み（pid継続）。超過paneは最終slot入れ子収容 |
| layout name指定 | direct | bare name（拡張子なし）がlayout_dir解決される |
| path / inline指定の明示的代替 | direct | positional=path、`--layout-string`=inline（**改行区切りKDL必須**） |
| overflow: 追加tabへのlayout反復 | **not possible（プロセス保存付きでは）** | 実験確定（capabilities §4.1）: override-layoutは既存paneを第1tabにしか割り当てず、追加tabは常に新規pane。はみ出しはkillか第1tab内押し込みの二択。代替設計は§4.1に従う |
| pane順序の決定性 | approximate | slot割当順はpane ID昇順ではない（観測済み）。適用後`list-panes`検証で実対応を確定する設計で対応 |
| plugin paneの明示扱い | orchestration | `is_selectable`/`plugin_url`で判別しslot勘定から除外。retain flagはfloating/pluginで効かない点を明示 |
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
| 最小サポートバージョン | 判定済み | **0.44.1**（`--layout-string`導入）。`override-layout`/`list-panes --json`/`--pane-id`全系は0.44.0〜。検証済みは0.44.3 |
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
| DD-10 | `remap`アルゴリズム（overflow含む） | preservation・overflow・順序・plugin扱い |
| DD-11 | `add`/`remove`操作設計 | 追加/削除・空tab cleanup |
| DD-12 | safety model（dry-run・確認・部分失敗・error分類/exit status） | dry-run・clear error handling |

Acceptance criteria（§9の14項）の最終監査は §6に実施した。

## 6. Phase 8 最終監査（P0要件・AC × 設計・実装・テスト）

分類: implemented / implemented with documented limitation（文書化された制限付き）/ not implemented。
テスト階層: L1 unit / L2 fake-backend / L3 CLI契約（`cargo test`63件）/ L4 実機統合（tmp/phase7、S1〜S10全PASS）。

| 要件（requirements.md） | 設計 | 実装 | 自動テスト | L4 | 分類 |
|---|---|---|---|---|---|
| state discovery・ID/metadata（§2.1） | DD-2/3/5 | app/list.rs, zellij/parser.rs | L1 parser・selector / L3 list | S1 | implemented |
| layout discovery/resolution（§2.9） | DD-3.4/10.2 | layout/mod.rs | L1 KDL slot count / L3 排他 | S7/S8/S10 | implemented |
| read 1/多（§2.2） | DD-6 | app/read.rs | L3 | S2 | implemented |
| send 1/多 broadcast（§2.3） | DD-7 | app/send.rs | L3 | S3 | implemented |
| rename pane/tab（§2.4） | DD-8 | app/rename.rs | L2 | S4 | implemented |
| add/remove pane/tab（§2.7/2.8） | DD-11 | app/add.rs, app/remove.rs | L2 | S5 | implemented（空tab実削除は代替検証のみ: zellij 0.44.3で空tab作成不能。MR-14） |
| 高位resize（§2.5） | DD-9 | app/resize.rs | L2 | S6 | implemented with documented limitation（近似・完全幾何保証なし） |
| remap保存（§2.6） | DD-10 | app/remap.rs, layout/mod.rs | L2 R1〜R10・dry-run | S7（heartbeat pid継続+pane ID同一で実証） | implemented |
| remap overflow（§2.6） | DD-10.3 | 同上 | L2 R3/R3t/R4/R5・R7 | S9（2 tab構成・再作成command一致） | implemented with documented limitation（保存付き反復はzellij的に不可能→`--overflow nest|tabs`明示制。§4-1に文書化） |
| JSON出力（§3.6） | DD-4 | output/json.rs | L1 error_map / L3 json contract | S1〜S9 | implemented |
| shell completion（P0） | DD-1.7 | main.rs | L3 completion | - | implemented |
| clear error handling（P0） | DD-4/12 | error.rs | L1対応表 / L3 exit code | S10 | implemented |
| dry-run（§3.7） | DD-12 | remap/remove | L2 / L3 | S8・S5 | implemented with documented limitation（`resize equalize`/`add`はP1: MR-7） |
| AC-1〜6・9〜11・13・14 | 上記対応行と同一 | - | - | S1〜S6, S10 | implemented（AC-14はhelp文言test無し、実装と--help出力で確認） |

**AC-7（kill/recreateなしのremap）**: L4 S7/S9で実証（保存対象paneのheartbeat pidと連番が途切れなく継続、pane ID同一、geometry変化）。
**AC-8（overflow決定性）**: S9で実証（既定exit 7+案内→`--overflow tabs`で2 tab・決定的割当）。要件文言からの変更有り（§4-1）。
**not implemented**: なし（P0範囲）。P1（bulk rename・`--no-fill`・equalize/add dry-run・watch等）はrequirements P1リストのまま未実装。

## 4. 制約と設計への指示（impossible要件の代替）

1. **cross-tab pane移動は不可**（CLIにprimitives不存在、実機確認済み）。かつ**複数tab layout適用でも既存paneは第1tabにしか配分されない**（実験E1〜E5で確定）。要件2.6のoverflow（M>N時の追加tab layout反復）はプロセス保存付きでは実現不能。zelperの設計（detailed-design DD-10）:
   - M ≤ N: 単tab適用で全pane保存（実証済み）— 要件の主パスは完全充足
   - M > N: 黙示のkill/再作成は行わない（要件の禁止事項）。明示的選択制のoverflow戦略（`--overflow`）を導入する:
     - `nest`: retain付き単tab適用。全pane保存・layout形状は保証されない（近似）
     - `tabs`: 追加tabでlayoutを反復し、overflow分のpaneをcommand再起動で再構成（当該paneのみ破壊的・明示指定必須）。要件2.6のoverflow語彙はこのmodeが担う
   - この違反は「要件が想定したzellij機能（保存付きcross-tab配分）が存在しない」ことに起因し、要件自身の例外規定（破壊的再作成は明示選択時のみ）に沿った最も近い設計である
2. **session一覧のJSON不在**: `list sessions --json`はセマンティクスをzelperが定義したJSONに載せ替える（sourceはテキストparse）
3. **resizeの正確な幾何保証は不可**: 達成可能な近似であることをhelp/JSONに明示する。収束しない目標では反復上限・振動検出で打ち切り、達成幾何とnotes（非収束の旨）を付して成功で返す（DD-9の近似契約。MR-5で確定）
4. **rename-sessionで旧名に戻せない**挙動（exit 0で無効果）が観測済み。zelperはsession renameをP0に含めない（要件もpane/tabのみ）
5. **サイレント成功**（存在しないIDへのclose等がexit 0）: zelperのpostcondition検証で検出し、エラーとして報告する

## 5. Phase 3への引継ぎ事項（完了状況）

- ~~最優先実験: 複数tab layoutのoverride-layout適用~~ → 実施済み。結果はcapabilities §4.1（保存付きoverflowは不可と確定）
- 未実施（v1設計で回避済みのため影響なし）: `paste`実挙動（v1はwrite-charsのみ使用）、resize刻み幅（DD-9の幾何検証で吸収）、stacked panes実挙動（remap対象はtiled terminal paneのみ）
- DD-1〜DD-12の章構成によるdetailed-design.md作成 → 完了
