# zelper Test Plan (Phase 5)

作成日: 2026-08-21
前提: detailed-design.md DD-1〜DD-12（design-review DR-1〜DR-10反映済み）
原則: 安定した純粋ロジック（selector・planner・parser）については実装前にfailするテストを書く（development-plan Phase 5）

## 1. テスト階層と実行環境

| 階層 | 対象 | 実行環境 | zellij依存 |
|---|---|---|---|
| L1 unit | selector / planner / KDL parse・生成 / JSON serialize | `cargo test`（in-module + tests/unit） | なし |
| L2 fake-backend | ZellijBackend traitのfakeによる多段操作・部分失敗 | `cargo test`（tests/fake_backend） | なし（fake） |
| L3 CLI契約 | 引数parse・help・exit status・stdout構造・fake shim呼び出し列 | `cargo test`（tests/cli、`assert_cmd`） | fake zellij shim（tests/fixtures/fake-zellij。argvを記録しfixtureを返す実行可能script） |
| L4 統合 | 実zellij 0.44.3での実挙動・preservation実証 | podman sandbox（debian:12-slim + ホストバイナリ ro mount、--network=none、config.kdl配置でSetup Wizard抑止、`script -qec` PTY駆動） | あり |

L4の実行形: Phase 1と同一のpodman構成（docs/research/zellij-capabilities.md §3）。テストランナーはコンテナ内で (1) session起動 (2) zelperバイナリ実行（/workにmount） (3) heartbeat / list-panes --json で検証 (4) trapでsession全kill。**実行環境のホストでzellijは実行しない**。

## 2. 領域別テスト仕様

### 2.1 CLI parser tests（L1/L3）

- 各verbの正常系: DD-1.6の全exampleがparseされ意図するtyped requestになる
- 非例の全パターン（DD-1.6非例）: exit 2 + usage error
- 排他規則（DD-1.5）の全組み合わせ: remap 3source併用、send text/keys併用、--tab/--session-scope併用 等
- PANESPEC正規形: `3`→`terminal_3`正規化、`plugin_1`受理、不正文字はexit 2
- session解決: --session > env > 単一session > error（候補列出）の優先順
- `completion bash|zsh|fish`が空でない補完scriptを出力

### 2.2 target resolver tests（L1）

fixture: PaneState列（tab混在・floating混在・同名title・command部分一致）を用意。

- positional ID解決: 存在確認・0件（NoTarget）・存在しないID
- filter option: --name完全一致・--command部分一致・--cwd一致・--all・--tab（ID/一意名）の各集合計算
- positional + filter の和集合
- 単一対象要求（rename）で複数ヒット: AmbiguousTarget + 候補列出
- 順序の決定性: (tab_position, pane_y, pane_x) 昇順が固定されること（同値でないfixtureでの順序固定）

### 2.3 backend parser fixtures（L1/L2）

- `list-panes -a --json`実出力fixture（Phase 1のout/02-panes-*.jsonを恒久化: tests/fixtures/zellij/）→ PaneState変換の全field
- `list-tabs -a --json` fixture → TabState
- `list-sessions -n` テキストfixture → SessionInfo（色なし形式）
- `new-pane` stdout（`terminal_7\n`）/ `new-tab` stdout（`3\n`）のID parse
- zellijのerror出力（`Failed to load layout: ...`等、Phase 1失敗記録から）→ error class変換
- version文字列（`zellij 0.44.3` / `0.44.0` / `0.43.1` / 不正）→ 判定（0.44.1以上/未満/parse不能）

### 2.4 read / send multi-target（L2/L3）

- read: 対象順のdump-screen呼び出し列・`--tail`/`--full`の加工・per-pane失敗でexit 6・全失敗でexit 5
- send: text→write-chars呼び出し・`--enter`で追加のwrite 13・`--keys`→send-keys argv分割・部分失敗でも残対象継続
- fake shim: 呼び出しargv列の記録と期待比較（L3）

### 2.5 rename（L2）

- pane/tab rename呼び出しとpostcondition検証（list-panes再取得でtitle反映）
- 検証失敗（title不変）→ VerificationFailed exit 7

### 2.6 resize planning / convergence（L2）

- grow/shrink: STEPS回のresize呼び出し・no-op 2連続で打ち切り+warning
- equalize: 3 pane横並びfixtureで目標均等幅計算が正しい
- 収束ループ: fakeが途中でno-opを返す系列→20 iteration上限での打ち切り・報告
- 不可能目標（振動する幾何）→ 打ち切り+notes付きで**成功**（DD-9の近似契約。達成幾何とnotesを出力。MR-5で確定）
- **無限ループ禁止の保証**: 反復上限とno-op検出の両方が機能するケースを1テストで固定

### 2.7 remap planning matrix（L1/L2・最重要）

plannerを純粋関数（現状PaneState列+LayoutRef+option → RemapPlan）として検証:

| case | 入力 | 期待plan |
|---|---|---|
| R1 | 1 pane/3-slot | fill mode・slot割当・空2 slotは既定shell |
| R2 | 3/3 | fill mode・全保存 |
| R3 | 4/3 optionなし | error（nest/tabs案内文を含む） |
| R3n | 4/3 nest | 単tab適用・全保存・形状不保証の明示 |
| R3t | 4/3 tabs | instance 2・pane3のみrecreated・新tab KDLにcommand |
| R4 | 6/3 tabs | instance 2・pane0-2 preserved/3-5 recreated |
| R5 | 7/3 tabs | instance 3・pane6がtab 3 slot 1・残slot shell |
| R6 | 複数tab + --session-scope | tab毎の独立plan・cross-tab移動なし |
| R7 | 6/3 tabs・instance 2のnew-tab失敗（fake） | 実行済み/失敗/未実行の報告・rollbackなし |
| R8 | floating pane存在 | preflight error・`--embed-floating`でtoggle呼び出し後に対象化 |
| R9 | M<N・layoutにcommand付きslot | 空slotのcommandが生成KDLに保持される |
| R10 | visual order | (tab_position, pane_y, pane_x)順の割当 |

- KDL生成: 生成文字列が改行区切り形式・bar plugin明示（DD-3.3/10.7）であることのsnapshot test
- KDL parse: slot数カウント（bare/command/edit/plugin混在layout fixture）・plugin除外（config子node持ち・bare両方。plugin配下はすべてplugin configurationのため再帰しない: MR-32回帰）

### 2.8 integration fixtures / sessions（L4）

- 共通harness: `tests/integration/`にpodman実行helper（隔離env生成・config.kdl配置・session起動/破棄）
- harness要件: 十分な表示サイズ（200x60）を保証（Phase 1副次発見: 狭いとnew-paneが静かに失敗する）
- 主要シナリオ: list/read/send/rename/add/remove/resize/remap各1以上・実行後list-panes --jsonによるpostcondition assert

### 2.9 failure injection（L2/L4）

- zellij不在（PATHに存在しない）→ ZellijUnavailable exit 4
- 古いversion（fakeが`0.43.1`）→ UnsupportedVersion exit 4
- list-panesがerror → 対象解決前の失敗伝播
- close-paneがexit 0で無操作（実zellij挙動）→ postcondition検証によるVerificationFailed検出
- remap途中失敗（R7）
- 部分失敗のJSON: results[]のok/target構造

### 2.10 JSON contract tests（L3）

- 各成功コマンド: `schema_version:1`・`ok:true`・stable fields存在
- 各error class: exit code対応（DD-4.3表の全行）・error.class値
- **stable fieldsの将来互換**: snapshot testで無変更を強制

### 2.11 compatibility tests（L4任意）

- 0.44.3（実機）を基本。0.44.1実機は入手でき次第（P1）。

## 3. preservation実証（acceptance criteria 7・Phase 7要件）

「再作成ではなく同一processが生存」を**pane identity**で証明する:

1. 対象pane内でheartbeatプロセス（pid・連番を定期的にlogへ書く）を起動
2. zelper remap実行
3. 検証: (a) heartbeat logのpidと連番が途切れなく継続、(b) list-panesのpane ID同一
3', tabs mode のrecreated pane: 旧ID消失・新IDのpane_command一致を確認（保存対象外であることの明示）

この手法はPhase 1で確立済み（hb.sh）。R2/fill・nest・tabs第1instanceで実施。

## 4. tests/構成（tests/README.mdで管理。MR-8で実態に合わせ更新）

```text
tests/
  README.md                      # 本構成の索引
  unit/                          # L1純粋ロジック（selector / parser / error_map）
  fake_backend/                  # L2（fake.rs = FakeBackend本体 + 各領域テスト）
  cli/                           # L3（assert_cmd。fake zellij shimはテスト実行時に生成）
  fixtures/
    zellij/                      # 実出力JSON/text fixture
```

L4統合テストは`tests/`外の`tmp/phase7/`（エフェメラルharness。podman構成は§1記載）で実行し、結果を`tmp/phase7/integration-report.md`に記録する。恒久化する場合は`tests/integration/`へ昇格する。

## 5. 実装前テスト（fail-first対象）

Phase 6a開始時に最初に書くテスト（実装前にfailすることを確認）:

1. selector解決の2.2全ケース
2. remap plannerのR1〜R10
3. backend parserの2.3全fixtures
4. JSON contractのexit/class対応表全行
