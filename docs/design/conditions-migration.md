# テスト構造移行設計（conditions.toml全面導入）

作成日: 2026-09-13
位置づけ: zelperのテスト構造を test-structure skill（`~/.claude/skills/test-structure/SKILL.md`）準拠のテスト設計条件書（`tests/design/`のconditions.toml）方式へ移行するための適用設計。構造規則（schema・stable id・検証仕様・作成順序）の正本はskill側にあり、本書はzelper repoへの適用内容（file構成・id採番・tag仕様・verify script・段階適用・文書再構成）を定める。本書の作成時点ではtest code・toml・scriptは一切作成しない（移行実施は§9の段階計画による）。

確定済み方針（ユーザー決定・再議論対象外）: conditions.toml全面導入・条件書が正本・133 test（lib内3件含む）へのcovers tag付与・verify script整備・設計書起点の正規route（skill §5の順序）。`--bootstrap`は付け漏れ検出の対照表生成にのみ使い、出力は`tmp/`使い捨てとする。

## 1. 目的と範囲

### 1.1 移行のbefore/after

before（現状）:

- 検証条件の正本は `docs/testing/test-plan.md` §2（領域別テスト仕様。remap v2 matrix R1〜R50の定義を本文で保持）
- testと条件の対応は暗黙: test fn名へのR番号/C番号埋め込み（64件）と、一部file header commentの節参照（4件）のみ
- test-plan §2と`tests/README.md`に検証内容の記述が重複し、test追加のたび両方の長文更新が発生する構造

after（移行後）:

- 検証条件の正本は `tests/design/`の条件書15file（§2）。各条件はstable id・出典（source）・given/expectを持つ
- 全133 test fn（tests/配下130 + `src/app/list.rs`のlib内3件）が `// [covers:<id>]` tagで条件に紐付く（§4）
- `scripts/design/verify-conditions.py`（§5）がid対応・網羅・schema・source_lines自動同期・excluded構造・付け漏れ（`--complete`時）を機械検証する
- test-plan.md §2は条件書参照の対応表へ縮小、`tests/README.md`は索引化（§6）。R番号は条件書source欄と`tests/design/README.md`対応表で保持（§3・§7）

### 1.2 対象外

- L4統合test harness（podman・実zellij）の恒久化・`tests/integration/`への昇格（現行どおりrepo管理外。要件・実施記録はtest-plan L4節へ集約）
- 新規testの追加・既存testのロジック変更・fn名・doc commentの変更（既存133 testへのtag行挿入のみ）
- `tests/fake_backend/fake.rs`等のtest支援資産の構造変更
- update-tests skill運用そのものの変更（§10で移行後の接続のみ定義）

## 2. 条件書file構成（15file）

### 2.1 構成規則

- 配置: `tests/design/<stem>.toml`。stem = stable idのarea名（§3.1）と一致する
- 切り方: verb/機能別。remap系はtest-plan §2.7の副区分に従い5分割（(a) planner / (b) generator / (c) seed / (d)(e)(f)(j) 実行sequence / (g)(h)(i) CLI側）
- 条件書fileとtest fileは1:1ではない。test file（`[[test]]` target）は配布単位、条件書は機能単位であり、1 test fileが複数条件書に跨る。各testの帰属はtagのidのareaで定まる
- 条件書1件の`[meta].source_file`は代表する実装file 1件とし、関連する他の実装fileは`note`欄へ記載する（schemaはsource_fileを1件とする定義のため）

### 2.2 一覧表（最終確定構成）

本表は移行完了時点（2026-09-13・133条件）の確定値。移行後の未test項目の条件化（TASK-61〜71等）による条件追加・excluded解消は`tests/design/README.md`（条件書一覧・進捗記録）を正本として管理し、本表の数値は更新しない。

| # | file（stem） | feature | source_file（[meta]） | 対応test target（[[test]] name / lib） | 出典 | R番号範囲 | 条件数 |
|---|---|---|---|---|---|---|---|
| 1 | cli-grammar.toml | cli-grammar | src/cli.rs | cli_list_read_send（部分6件） | test-plan §2.1・§2.9（zellij不在はexcluded） | なし | 6 |
| 2 | selector.toml | selector | src/selector.rs | selector（8件） | test-plan §2.2 | なし | 8 |
| 3 | backend-parser.toml | backend-parser | src/zellij/parser.rs | parser（6件） | test-plan §2.3 | なし | 6 |
| 4 | json-contract.toml | json-contract | src/output/json.rs | error_map（4件）+ cli_list_read_send（部分2件） | test-plan §2.10・§2.9（部分） | なし | 6 |
| 5 | read-send.toml | read-send | src/app/read.rs | cli_list_read_send（部分5件） | test-plan §2.4 | なし | 5 |
| 6 | list-display.toml | list-display | src/app/list.rs | cli_list_read_send（部分10件）+ lib src/app/list.rs（3件） | test-plan §2.1（list系）+ TASK-44〜47（design-review §4.9〜4.11） | なし | 13 |
| 7 | rename.toml | rename | src/app/rename.rs | fake_rename_add_remove（部分3件） | test-plan §2.5・§2.9（部分） | なし | 3 |
| 8 | add-remove.toml | add-remove | src/app/remove.rs | fake_rename_add_remove（部分7件） | requirements §2.7/2.8・DD-11（test-plan §2に節なし） | なし | 7 |
| 9 | resize.toml | resize | src/app/resize.rs | fake_resize（7件） | test-plan §2.6・DD-9 | なし | 7 |
| 10 | layout-planner.toml | layout-planner | src/app/remap.rs | layout_planner（15件） | test-plan §2.7 (a)・DD-10.5/10.6 | R1〜R7・R9〜R12・R20 | 15 |
| 11 | layout-generator.toml | layout-generator | src/layout/generator.rs | layout_generator（8件） | test-plan §2.7 (b)・DD-10.8 | R9・R13〜R19 | 8 |
| 12 | companion-seed.toml | companion-seed | src/companion.rs | companion_seed（14件） | test-plan §2.7 (c)・DD-10.4 | R21〜R28 | 14 |
| 13 | remap-sequence.toml | remap-sequence | src/app/remap.rs | fake_remap（20件） | test-plan §2.7 R8・(d)(e)(f)(j)・(i)実行側・DD-10 | R8・R29〜R42・R47・R50 | 20 |
| 14 | remap-cli.toml | remap-cli | src/app/remap.rs | cli_remap（8件） | test-plan §2.7 (g)(h)(i)出力側・DD-12 | R43〜R46・R48〜R50 | 8 |
| 15 | docs-verb.toml | docs-verb | src/app/docs.rs | cli_docs（7件） | TASK-24/25・design-review §4.4 | なし | 7 |

計: 15file・133条件（= 133 test fn。§3.4の1:1原則）

source_fileの代表とnote欄記載の対応:

- json-contract: source_file = src/output/json.rs、note = 「src/error.rs（ErrorClass・exit code対応）も検証対象」
- read-send: source_file = src/app/read.rs、note = 「src/app/send.rs も検証対象」
- add-remove: source_file = src/app/remove.rs、note = 「src/app/add.rs も検証対象」
- remap-cli: source_file = src/app/remap.rs、note = 「廃止flag・version gateは src/cli.rs・src/zellij/process.rs も検証対象」

### 2.3 test file側の帰属内訳（1 fileが複数条件書に跨る分）

- `tests/cli/list_read_send.rs` 23件の振分: cli-grammar 6件（排他規則3・PANESPEC 1・completion 1・session解決1）/ read-send 5件（read 3・send 2）/ list-display 10件 / json-contract 2件（list panes JSON契約・error envelope）
- `tests/fake_backend/rename_add_remove.rs` 10件の振分: rename 3件 / add-remove 7件
- lib内test 3件（src/app/list.rs）: list-displayへ帰属。src/のままtag付け（§4.3）
- test-plan §2.1の「各verb正常系parse」条件は各verbの条件書へ分散帰属する（当該verb testが検証する。ただしL2 fake backend testはapp層関数の直呼びでありCLI引数parseを経由しないため、parse検証はL3 testを持つverb（list/read/send/docs）が担い、L2担当verb（rename/add/remove/resize）のparseは未testとしてcli-grammar.tomlの[[excluded]]へ記録する〔CMR4-4〕）。cli-grammarは文法の負荷側（非例・排他・PANESPEC正規形・session解決順序・completion）を受け持つ
- R9はplanner側（k=1維持）とgenerator側（layout宣言commandのbare正規化）の2条件、R12は3条件（N算出・multi-tab先頭tab形状・実機KDL形式のparse頑健性）、R21/R22は各2条件（判定純粋関数と実file書込）、R50はremap-cli（操作列種別表示）とremap-sequence（floating計画一致）の2条件に分割される

### 2.4 条件書記述の原点

- 各条件のdescription/given/expect_*は出典（test-plan §2該当節・detailed-design DD-x・design-review該当節・requirements）の記述から執筆する。test fnや実装の写しにしない（skill §1。本移行は信頼できる出典が存在するためfallbackではない）
- expect/descriptionはtestが実際にassertする内容を超えて出典要求を主張しない。超える場合は検証限界を明記する（第1段レビューCMR3-3/3-4/3-6で確定。第2〜3段の様式規則）
- 各fileの作成はskill §5「作成」の順序を踏む: 出典の全文読み → 要求される条件の系統的列挙 → 全項目の対応決定（条件化 / excluded記録） → 執筆 → tag付け → test実行 → verify → meta記録。列挙と対応決定を終えないまま「条件を書いた」としない
- source欄の書式: `docs/testing/test-plan.md §2.7 (a) R3 + docs/design/detailed-design.md DD-10.6` のように、docs pathと節（+ R番号/DD番号/tm task）を指定する

## 3. stable id採番規則

### 3.1 形式（skill §2準拠）

- `<area>.<kebab-topic>`。小文字ASCII英数字と`-`（単語区切り）。区切りの`.`は1つ。`_`は使わない
- areaは条件書fileのstem（§2.2の15種）と一致する。verify (c)が一致を機械検証する
- 付与後不変。条件の統合・分割時は新idを発行し、旧idを参照するtagを張り替え、`tests/design/README.md`対応表も更新する
- 位置依存の連番id（tNN等）は使わない。R番号はidの一部として保持する（§3.2）が、これは出典test-plan §2.7の要件番号であり連番idではない

### 3.2 R番号埋め込み規則

- 出典条件がtest-plan §2.7のR番号（R1〜R50）を持つ場合、topicを `r<N>-<内容kebab>` とする。例: `layout-planner.r3-four-panes-into-three-slots-k2`
- 数字は0埋めしない（r3・r12・r46）
- 同一R番号から複数条件が生じる場合（§2.3: R9/R12/R21/R22/R50）、`r<N>-`より後の内容kebabで全局一意にする。R番号は`tests/design/README.md`対応表で複数idに紐付く

### 3.3 R番号を持たない条件の命名

- 条件の内容をkebabで記述する。test fn名をkebab化したものでもよいが、基準はfn名ではなく条件の内容
- C番号（design-reviewの指摘ID）はidに使わない。C番号はdesign-review.mdを通した連番（C1〜C24）であるが、レビュー指摘logの追番であり要件のstable idではない（番号自体は条件内容を示さない）。出典はsource欄で節付きで特定する（例: `docs/design/design-review.md §4.7 C3（TASK-39レビュー）`。節参照によりどのレビューのどの指摘かまで特定できる）
- C番号由来の条件であっても実質がR番号条件の拡張である場合は、source欄に両方を書く（例: quoting warning拡張は `design-review §4.7 C6` を主出典とし、`test-plan §2.7 (a) R20` を関連に付ける。idは内容kebab）

### 3.4 条件とtestの対応（1:1原則）

- 移行時は既存133 test fnと1:1になるよう条件を定義する（1条件 = 1 tag）。既存testは出典からfail-firstで執筆された実績があり、検証断面が出典条件の単位とほぼ一致するため
- 1 test fnが複数の出典要件を同時に検証する場合は、1条件に統合しsource欄へ全要件の出典を列挙する（test fnの分割は行わない。fn名・test構成は不変のため）
- 1条件に複数testが紐づく形は、移行後の更新運用で条件の統合を行うときにのみ許容する（§10.2）

### 3.5 area一覧（15）

cli-grammar / selector / backend-parser / json-contract / read-send / list-display / rename / add-remove / resize / layout-planner / layout-generator / companion-seed / remap-sequence / remap-cli / docs-verb

### 3.6 実例（実際のtest fnからの変換・全area網羅）

| test fn（所在file） | 条件id（確定） | source欄（出典） |
|---|---|---|
| positional_id_resolution_and_no_target（unit/selector.rs） | selector.positional-id-resolution-and-no-target | test-plan §2.2・DD-2 |
| visual_order_deterministic（unit/selector.rs） | selector.visual-order-deterministic | test-plan §2.2（順序の決定性） |
| empty_tab_definition（unit/selector.rs） | selector.empty-tab-definition | requirements §2.8（空tabの定義）・DD-11 |
| version_strings（unit/parser.rs） | backend-parser.version-strings | test-plan §2.3・DD-3.1/3.5 |
| sessions_parse_flags_current_and_exited（unit/parser.rs） | backend-parser.sessions-parse-flags-current-and-exited | test-plan §2.3・design-review §4.9（TASK-44） |
| class_exit_mapping_full_table（unit/error_map.rs） | json-contract.class-exit-mapping-full-table | test-plan §2.10・DD-4.3対応表 |
| json_error_envelope_shape（unit/error_map.rs） | json-contract.error-envelope-shape | test-plan §2.10・DD-4 |
| completion_generates_script（cli/list_read_send.rs） | cli-grammar.completion-generates-script | test-plan §2.1・DD-1.7 |
| invalid_pane_spec_is_usage_error（cli/list_read_send.rs） | cli-grammar.invalid-pane-spec-is-usage-error | test-plan §2.1（PANESPEC正規形）・DD-1 |
| read_nonexistent_pane_is_no_target_exit3（cli/list_read_send.rs） | read-send.read-nonexistent-pane-is-no-target-exit3 | test-plan §2.4・DD-6 |
| send_enter_appends_cr_and_keys_use_send_keys（cli/list_read_send.rs） | read-send.send-enter-appends-cr-and-keys-use-send-keys | test-plan §2.4・DD-7 |
| list_sessions_panes_summary_elides_beyond_four_tabs（cli/list_read_send.rs） | list-display.sessions-summary-elides-beyond-four-tabs | TASK-46・design-review §4.10 |
| list_panes_compact_shows_tab_and_short_cwd（cli/list_read_send.rs） | list-display.panes-compact-shows-tab-and-short-cwd | TASK-47・design-review §4.11 |
| human_age_granularity_boundaries（lib src/app/list.rs） | list-display.human-age-granularity-boundaries | detailed-design DD-4.1 + design-review §4.10 C14(d) |
| short_path_boundaries（lib src/app/list.rs） | list-display.home-substitution-boundaries | design-review §4.11 C21 |
| rename_pane_silent_noop_detected_by_postcondition（fake_backend/rename_add_remove.rs） | rename.pane-silent-noop-detected-by-postcondition | test-plan §2.5・§2.9（サイレント成功の検出）・requirements §4-5 |
| remove_empty_with_unknown_tab_name_is_error_not_silent_widen（fake_backend/rename_add_remove.rs） | add-remove.remove-empty-unknown-tab-name-is-error-not-silent-widen | requirements §2.8・DD-11 |
| equalize_oscillation_terminates_no_infinite_loop（fake_backend/resize.rs） | resize.equalize-oscillation-terminates-no-infinite-loop | test-plan §2.6・DD-9（近似契約・MR-5） |
| r1_one_pane_into_three_slots（unit/layout_planner.rs） | layout-planner.r1-one-pane-into-three-slots-k1-empty-two | test-plan §2.7 (a) R1・DD-10.6 |
| r3_four_into_three_k2_no_error（unit/layout_planner.rs） | layout-planner.r3-four-panes-into-three-slots-k2 | test-plan §2.7 (a) R3・DD-10.6 |
| r12_multi_tab_layout_counts_first_tab_shape_only（unit/layout_planner.rs） | layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only | test-plan §2.7 (a) R12・DD-10.2 |
| c6_quoting_warning_covers_single_quotes_newlines_control_chars（unit/layout_planner.rs） | layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars | design-review §4.7 C6（関連: test-plan §2.7 (a) R20） |
| r14_single_word_shell_command_is_still_injected（unit/layout_generator.rs） | layout-generator.r14-single-word-shell-command-still-injected | test-plan §2.7 (b) R14・DD-10.8 |
| r24_absent_node_is_pure_text_append_with_newline_guard（fake_backend/companion_seed.rs） | companion-seed.r24-absent-node-pure-text-append-with-newline-guard | test-plan §2.7 (c) R24・design-review §4.6 R1 |
| c2_stale_lock_file_is_tolerated_and_removed（fake_backend/companion_seed.rs） | companion-seed.stale-lock-file-tolerated-and-removed | design-review §4.7 C2 |
| r29_probe_only_when_move_needed（fake_backend/remap.rs） | remap-sequence.r29-probe-only-when-move-needed | test-plan §2.7 (d) R29・DD-10.4 |
| r41_verification_failure_reports_diff_and_progress（fake_backend/remap.rs） | remap-sequence.r41-verification-failure-reports-diff-and-progress | test-plan §2.7 (f) R41・DD-10.9 |
| exited_held_panes_are_excluded_from_source_and_survive（fake_backend/remap.rs） | remap-sequence.exited-held-panes-excluded-from-source-and-survive | DD-10.5（source定義: selectable tiled terminal） |
| r43_removed_flags_are_usage_error_exit2（cli/remap.rs） | remap-cli.r43-removed-flags-are-usage-error-exit2 | test-plan §2.7 (g) R43・DD-12・design-review §4.8 |
| r46_below_minimum_and_future_series_exit4（cli/remap.rs） | remap-cli.r46-below-minimum-and-future-series-exit4 | test-plan §2.7 (h) R46・DD-3.1/3.5 |
| docs_without_subcommand_is_usage_error（cli/docs.rs） | docs-verb.without-subcommand-is-usage-error | TASK-24/25・design-review §4.4 |

### 3.7 採番手順（移行実施時）

1. 出典条件にR番号があれば§3.2、なければ§3.3で命名する
2. 全条件書を通したid一意性とarea=stem一致をverify (c)が検証する。衝突時は内容kebabを具体化して再採番する（確定前のみ。確定後は不変）

## 4. covers tag仕様

### 4.1 構文（単一id）

test fn直上（`#[test]`属性行の直前）に `//` 行commentで1行で記述する。

```rust
// [covers:layout-planner.r3-four-panes-into-three-slots-k2]
#[test]
fn r3_four_into_three_k2_no_error() {
```

- tag行は `//`（doc commentでない）に限る。`/// [covers:...]` はdoc commentでありtagとして数えない。scannerの受理正規表現は `^\s*//\s*\[covers:([a-z0-9.-]+)\]\s*$` であり、`///` は `//` 直後に `/` が続くため受理されない
- 行末に他の文章を書かない（`// [covers:id] 説明` は不可）。説明は条件書のdescriptionへ書く

### 4.2 構文（複数id）

1 test fnが複数条件をcoverする場合は、1行1idで行を並べる。カンマ区切り・1行複数idは使わない（scannerの簡素化とdiffの単位をtest毎に保つため）。

```rust
// [covers:json-contract.error-envelope-shape]
// [covers:json-contract.class-exit-mapping-full-table]
#[test]
fn json_error_envelope_shape() {
```

移行時は§3.4の1:1原則により全tagが単一id行となる（1 test fnが複数出典要件を検証する場合は§3.4のとおり1条件へ統合し、source欄へ出典を列挙する）。複数id並記は移行後の更新で条件の分割等により1 testが複数条件をcoverする形になったときに現れる。

### 4.3 記述位置・既存doc commentとの共存

- 位置: `#[test]`属性行（複数attributeがある場合は属性block）の直前。既存の `///` doc commentがある場合は、doc commentと属性の間に置く
- Rustでは行comment（`//`）はtoken列に影響しないため、doc commentのitem付与を壊さない。fn名・既存doc comment（`/// P1-2(d): ...` 等の出典説明）は一切変更しない
- 例（src/app/list.rs）:

```rust
/// P1-2(d): human_ageの粒度境界（private fnのためlib内testで検証）
// [covers:list-display.human-age-granularity-boundaries]
#[test]
fn human_age_granularity_boundaries() {
```

- lib内test 3件（src/app/list.rs）: src/のままtag付けする。`#[cfg(test)] mod` 内のtagは通常の行commentでありcompileに影響しない。file移動はしない
- helper fn（`#[test]`を持たないfn）にtagを付けない

### 4.4 対象別の扱い・scanner走査対象（glob）

- 走査対象: `tests/**/*.rs`・`src/**/*.rs`・`plugin/src/**/*.rs`（companion plugin実装。現在test fnなし。将来pluginにtestを追加した場合も付け漏れ検査の守備に含める）。fixtures（`tests/fixtures/zellij/*.json`）は対象外
- `tests/fake_backend/fake.rs`: test支援資産でtest fnなし。tag対象外。`tests/design/README.md`の範囲明示に対象外として記載する（§7）
- tag行からtest fnへの紐付け: scannerはtag行を検出した後、同じfile内で以降最初に現れる `fn <name>` 宣言（attribute行を読み飛ばす）を対象test fnとみなす。`source_lines`にはtag行の位置（repo相対 `path:line`）を記録する

## 5. verify script仕様

### 5.1 概要・起動方法

- path: `scripts/design/verify-conditions.py`（skill §4の規定path）
- 実装: python・stdlibのみ（tomllib・re・pathlib・argparse・sys）。toml書込みはsource_lines行のtext置換による（§5.3）。外部依存・pip依存なし。script冒頭で `sys.version_info >= (3, 11)` を検査し、未満ならtomllib利用不可の旨を出力してfailする（tomllibはPython 3.11+必須）
- 起動: `uv run --no-project scripts/design/verify-conditions.py`（直接python実行しないproject rule準拠）
- mise task登録（mise.toml）。第0段の登録時点では `--strict-source` のみとし、第4段完了時に `--complete` 付きへ切り替える（検査(f)は全133 testのtag付け完了前に有効化すると必ずfailするため。§5.2 (f)）:

```toml
[tasks.verify-conditions]
description = "Verify tests/design conditions.toml (tag/coverage/schema/source_lines sync)"
run = "uv run --no-project scripts/design/verify-conditions.py --strict-source"
```

- `--strict-source` はskillが「全条件の移行完了後にon」とするが、本移行は全条件の出典が事前に判明しているため第1段からonで常用する
- 第4段完了後のmise taskは `run = "uv run --no-project scripts/design/verify-conditions.py --strict-source --complete"` へ更新する（§9.6）
- cargo testとの関係: 本scriptは条件書×tagの静的検証であり、test実行の代替ではない。test全件PASSは各条件書`[meta].test_command`（cargo test系コマンド）の実行で確認する。更新時の完結判定は「cargo test 全PASS」かつ「verify-conditions pass」の両方とする
- `meta.test_command`の記録例: selector.tomlは `cargo test --test selector`、list-display.tomlは `cargo test --test cli_list_read_send && cargo test --lib`、第4段の全縛め時は各fileの実行記録を確定済みcommandとする

### 5.2 検査項目

skill §4 (a)〜(e)に、repo拡張の(f)を加える。(f)は `--complete` flagで有効化される検査であり（§5.5）、移行中（第0〜3段）の通常実行には含まれない:

- (a) dangling tag: 全tagのidがいずれかの条件書の`[[condition]].id`と一致すること
- (b) 網羅: 全条件idが少なくとも1つのtagでcoverされていること
- (c) schema: `[meta]`必須fieldの存在（feature = file stemと一致・source_file・verified / verified_at / verified_by・test_command・source_hash）。`[[condition]]`のid形式（§3.1の正規表現・`.`は1つ）・全局id一意・area = stem一致・target_function / description / givenの非空・expect_return / expect_return_shape / expect_throw / expect_no_throwの1つ以上・conditionのverified = false時はunverifiable_reason非空必須・source_linesの形式（`path:line` のカンマ区切り）。`--strict-source`時はsource欄の空欄をfail
- (d) source_lines自動同期: tag走査結果（正本）への書き戻し（§5.3）。手書きしたsource_linesは上書きされる
- (e) excluded構造: `[[excluded]]`各entryのitem / source / reason非空・decided_atのYYYY-MM-DD形式・decided_byの値域（claude / human / codex）。空配列は許容。書かれたentryのみ検証し、記載漏れの検出は内容照合レビュー（§9）の役割
- (f) tag付け漏れ（repo拡張・`--complete`時のみ有効）: `#[test]`を持つ全fn（tests/・src/・plugin/src/）に直前tag行が存在すること。test追加時にtagを書かないまま残るのをfailで防ぐ。dangling・網羅とは逆方向の検査（test側から見た付け漏れ）であり、第4段完了後（mise taskへ`--complete`組み込み後）は常時作動する。移行中の付け漏れ検出は対照表突合で代替する（§5.5 `--list-untagged`・§9各段の完了判定）

### 5.3 source_lines同期機構

1. glob走査で (file, tag行, id, 対象fn) を列挙する
2. idごとに `["tests/unit/layout_planner.rs:142", ...]` の参照列を構築する（昇順）
3. 各tomlをtextで読み、`[[condition]]`block（id行から次の `[[condition]]` / `[[excluded]]` まで）内の `source_lines` 行と比較する
4. 差分があれば置換後のtext全体をまず構築する（当該行のみ置換。複数tagは `", "` 区切りの文字列1行。条件にsource_lines行がない新規条件はid行の直後に挿入。toml全体の再直列化は行わない）
5. 置換後textをin-memoryでtomllib parseし、parse失敗時はfailとする（fileへの書込みは行わない）
6. parse検証を通ったfileのみ書き込む。同期はscriptが行う唯一の書込みであり、meta等の他fieldには触れない

### 5.4 出力・exit code

- pass: `OK: 15 files, 133 conditions, 133 tags, N excluded (M files synced)` 形式のサマリ1行を出力しexit 0（(d)の書換があっても検査結果がpassならexit 0。書換file数を表示）
- fail: `FAIL <検査> <toml or test file>: <id or fn>: <詳細>` を検出件数分出力しexit 1。検査別にgroupingして表示する
- usage error（不明option等）: exit 2

### 5.5 オプション

- `--strict-source`: source欄の空欄をfail（§5.1のとおり初手からmise taskに組み込む）
- `--complete`: 検査(f)（全`#[test]` fnのtag存在）を有効化する。全133 testのtag付けが完了する第4段までは未tagが正常状態のため無効運用とし、第4段完了時にmise taskへ組み込む（§5.1・§5.2 (f)）
- `--list-untagged`: tagを持たない`#[test]` fnのfile・fn名一覧を出力する（検査ではなく報告であり、exit codeに影響しない）。移行中の付け漏れ検出は、この出力とtmp/対照表（`--bootstrap`生成）の該当領域fn listのdiffで行う: 段の対象領域に差分がなければ当該段のtag付けは完了。対象外領域の未tagは未実施段の分として許容される
- `--bootstrap [--out <dir>]`: 既存testから条件書を抽出生成する移行限定mode（skill §4）。stable id付きfileはskip。本移行では第0段の対照表生成のみに使用し、出力先は `tmp/<date>_bootstrap_conditions/`（使い捨て・commitしない）。対照表の用途: (1) 生成entry数 = 133であることの確認 (2) test fn一覧と本設計§2.2/§2.3の振分・§3.6のid対応の突合（付け漏れ・書き漏れ検出）
- `--verified`: `--bootstrap`併用の検証記録記入。本移行では未使用（metaは正規routeで手書きする）

## 6. test-plan.md再構成

### 6.1 残す節

- §1 テスト階層と実行環境（L1〜L4定義・podman構成。不変）
- §2.8 integration / §2.11 compatibility / §3 preservation実証: 「L4統合test」節へ集約する（L4要件・harness構成・実施記録 S1〜S11・S-v2-1〜6の正本は引き続きtest-plan）。旧節番号（§2.8・§2.11・§3）は§6.2対応表の行としてlocaterを存続させ、条件書excludedのsource欄参照を有効に保つ
- §4 tests/構成（tests/README.md参照を維持。条件書への言及を追記）
- §5 実装前テスト（fail-first履歴。残す）

### 6.2 §2縮小後の形式

§2.1〜§2.7・§2.9・§2.10の本文（条件定義）を削除し、次の対応表へ置換する。節番号（§2.7 (a)等）は条件書source欄のlocaterとして存続するため、対応表の行として維持する:

```markdown
## 2. 領域別テスト仕様（正本: tests/design/の条件書）

領域別の検証条件の正本は tests/design/<stem>.toml（test-structure skill準拠）。
条件id・given/expect・出典は各条件書を参照。R番号（remap v2 matrix R1〜R50）と
条件idの対応は tests/design/README.md。L4統合testは本file「L4統合test」節。

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
| §2.7 (d)〜(f)(j) | remap実行sequence | remap-sequence.toml | fake_remap |
| §2.7 (g)(h)(i) | remap CLI契約 | remap-cli.toml | cli_remap |
| §2.9 | failure injection（L2/L3分） | 各機能の条件書へ分散 | - |
| §2.10 | JSON contract | json-contract.toml | error_map + cli_list_read_send |
| §2.8 | L4 integration harness | （条件書なし。L4節へ集約。remap-sequence.tomlの[[excluded]]が参照） | L4（repo管理外） |
| §2.11 | compatibility（L4任意） | （条件書なし。L4節へ集約。remap-cli.tomlの[[excluded]]が参照） | L4（repo管理外） |
| §3 | preservation実証（L4） | （条件書なし。L4節へ集約。remap-sequence.tomlの[[excluded]]が参照） | L4（repo管理外） |
```

- L4でしか検証しない要求（§2.8・§2.11・§3）はL4節へ集約し、該当条件書の`[[excluded]]`から参照行で指す（§8）
- test file冒頭のheader comment（`// L1: target resolver（test-plan §2.2）`等）は、参照先の節番号が対応表として存続するため更新を必須としない。新規test fileは条件書を参照するheaderとする

### 6.3 tests/README.md索引化

- 各fileの長文内容説明を削除し、「file + 対応条件書 + 1行」の索引へ縮小する
- 検証条件・経緯の詳細は条件書が正本である旨を冒頭に明記する
- L4節・実行節は残す。規則節に「テスト追加時は条件書の更新が先行（update-tests skill二段階委任）」を追記する

### 6.4 構造対比（before/after）

| 対象 | before | after |
|---|---|---|
| test-plan §2.1〜§2.10 | 条件定義本文（R1〜R50表を含む） | 条件書参照の対応表 |
| test-plan §2.8・§2.11・§3 | L4要件が§2内と§3に分散 | 「L4統合test」節へ集約 |
| tests/README.md | file毎の内容長文説明 | 索引（file→条件書） |
| R番号の定義箇所 | test-plan §2.7本文 | 条件書source欄 + tests/design/README.md対応表 |
| 条件×test対応 | fn名埋め込み・header comment（暗黙） | covers tag（明示・機械検証） |

## 7. tests/design/README.md仕様

役割: `tests/design/`配下の索引・移行進捗・R番号対応表の管理（test-structure skill §7.3）。詳細は各条件書が持つため、索引のみとし肥大化させない。

構成:

1. 冒頭: 条件書が検証条件の正本である旨・test-structure skill準拠の旨・`mise run verify-conditions`の実行方法
2. 条件書一覧表: §2.2の表と同じ項目（stem・source_file・対応test target・条件数）にverified列を加えた実績値
3. R番号→条件id対応表: R1〜R50の各行に属するid（複数可。§2.3の分割対応）。旧test-plan §2.7本文をR番号で参照してきた読者の到達経路（§10.3）
4. 進捗記録: 段階適用（§9）の各段の完了日・対象file・残file。第4段完了後に「移行完了」を明記
5. 範囲明示: 対象外資産（`tests/fake_backend/fake.rs` = test支援資産・tag対象外、`tests/fixtures/` 、L4 harness = repo管理外）とlib内testの扱い（src/app/list.rsの3件はsrc/のままlist-display.tomlが管轄）・companion plugin実装（`plugin/src/`。現在test fnなし。test追加時はremap系の関連条件書へ条件化し、scanner走査対象に含まれる（§4.4））

## 8. excluded記録仕様

### 8.1 記録規則

- L4でしか検証しない要求: 該当機能の条件書へ参照行として記録する（実体の要件・記録はtest-plan L4節）
- 未testの要求（既存133 testでcoverされない出典要求）: 本移行では新規testを作らないため条件化できず、excludedに記録する。reasonに「未test」であることと今後の条件化条件を書き、移行実施時にtm起票する
- test支援資産の能力（fake backend失敗注入等）: 製品の検証条件ではないためexcludedに1行で対象外記録する
- 移行実施時の列挙（skill §5 step3）で「条件化しない」と決めた全項目にこの形式で記録する。機械検査は構造のみ（§5 (e)）であり、記載漏れの検出は内容照合レビュー（§9）で行う

### 8.2 記載行案（確定済み分の全文）

remap-sequence.toml:

```toml
[[excluded]]
item = "実zellijでのremapプロセス保存実証（heartbeat pid継続・pane ID同一・cross-tab移動・companion setup・probe込みのE2E）"
source = "docs/testing/test-plan.md §3・§2.8（S7・S-v2-1〜6として実施済み）"
reason = "実zellij依存のL4統合testでのみ検証可能。repo管理外podman harnessで実施済みでありL1〜L3の自動test対象外（test-structure skill §6の階層対応。要件はtest-plan L4節に集約）"
decided_at = "2026-09-13"
decided_by = "human"

[[excluded]]
item = "空slotの既定shell paneの実機での実在確認（R40のL4側強化）"
source = "docs/testing/test-plan.md §2.7 (f) R40"
reason = "L2ではpane数==Nとして代理検証（remap-sequence.r40-empty-slots-filled-with-default-shell）。shell paneの実在確認はL4でのみ可能（S-v2-1で実施済み）"
decided_at = "2026-09-13"
decided_by = "human"

[[excluded]]
item = "fake backendの失敗注入能力一式（pipe block・pipe効果不出現・permissions.kdl書換競合・不正layout）"
source = "docs/testing/test-plan.md §2.7 (j)"
reason = "test支援資産（tests/fake_backend/fake.rs）の能力であり製品の検証条件ではない。各注入の効果は個別条件（companion-seed.r28系・remap-sequence.r31系等）で検証済み"
decided_at = "2026-09-13"
decided_by = "human"
```

cli-grammar.toml:

```toml
[[excluded]]
item = "zellij不在（PATHに存在しない）でZellijUnavailable exit 4となる実行pathの検証"
source = "docs/testing/test-plan.md §2.9"
reason = "未test。exit code対応はjson-contract.class-exit-mapping-full-tableで検証済みだが、process起動失敗の実pathは未検証。新規test追加時に条件化する（移行実施時にtm起票）"
decided_at = "2026-09-13"
decided_by = "claude"
```

remap-cli.toml:

```toml
[[excluded]]
item = "未実証組合せ（zellij 0.44.4以降・新series）での実機確認"
source = "docs/testing/test-plan.md §2.11"
reason = "L4任意・tile pin更新とセットで実施（P1）。安全網（probe R30/R31・postcondition R37〜R41）はL2条件として条件化済み"
decided_at = "2026-09-13"
decided_by = "human"
```

### 8.3 実施時に対応決定が必要な残項目（既知の未test・要確認要求）

第2段の列挙（skill §5 step3）で「条件化 / excluded + tm起票」の対応を決定する既知項目:

- (a) readの`--tail`/`--full`加工（design-review §4.2 MR-30でtail testはP1と明示された経緯あり）
- (b) readのper-pane失敗exit 6・全失敗exit 5（test-plan §2.4）
- (c) send部分失敗時の残対象継続（test-plan §2.4）
- (d) `--cwd`一致filter（test-plan §2.2）
- (e) zellij error出力→error class変換（test-plan §2.3「Failed to load layout: ...等、Phase 1失敗記録から」）
- (f) list-panes errorの対象解決前伝播（test-plan §2.9）
- (g) §2.1排他規則の一部（add tab 3source併用・resize equalize×grow/shrink併用）
- (h) §2.1のPANESPEC正規形（`3`→`terminal_3`正規化）に既存testが対応するかの確認
- §2.9残項目の振分（close-pane exit 0無操作の検出はrename側で条件化済み、部分失敗JSONはjson-contract側で条件化等）

各項目は既存testでcoverされるなら該当機能の条件書へ条件化し、未testなら§8.1の未test規則でexcluded記録 + tm起票する。

## 9. 段階適用計画

### 9.1 段階一覧

| 段 | 内容 | 条件書 | 条件数 | tag付け |
|---|---|---|---|---|
| 第0段 | 基盤整備 | なし（script・mise・README骨子・対照表） | 0 | 0 |
| 第1段 | パイロット（小規模） | selector・backend-parser・json-contract | 20 | 20 |
| 第2段 | 中規模 | cli-grammar・read-send・list-display・rename・add-remove・resize・docs-verb | 48 | 48（lib内3件含む） |
| 第3段 | remap系 | layout-planner・layout-generator・companion-seed・remap-sequence・remap-cli | 65 | 65 |
| 第4段 | 文書縮小 + 全締め | なし（test-plan縮小・README完成・meta揃え） | 0 | 0 |

累計133条件・133tagで完了。

### 9.2 第0段（基盤整備）

作業:

- `scripts/design/verify-conditions.py`実装（§5仕様）
- mise.tomlへ`verify-conditions` task登録
- `tests/design/`作成、`README.md`骨子（§7の構成で未完成部分は進捗記録に「未実施」明記）
- `--bootstrap --out tmp/<date>_bootstrap_conditions/`で対照表生成

完了判定:

1. toml 0件状態で `mise run verify-conditions` がexit 0
2. `cargo test` 133件全PASS（tag未挿入のためtest側は不変であることの確認）
3. 対照表のfn数 = 133、本設計§2.2/§2.3の振分との突合で不一致なし
4. script仕様レビューの指摘へ対応済み

独立レビュー点位: script仕様の独立subagentレビュー（作成者と別agent。指摘は`docs/design/design-review.md`へ節を立ててID付き記録し、修正/P1化/却下のdispositionを残す。プロジェクトCLAUDE.mdのsubagent運用規則による）。

### 9.3 第1段（パイロット）

対象: selector.toml（8）・backend-parser.toml（6）・json-contract.toml（6）

手順（skill §5「作成」step1〜7）: 出典全文読み → 条件列挙 → 全項目の対応決定（条件化/excluded） → 条件書執筆（§3のid規則） → tag付け（§4仕様でtag行挿入のみ。fn名・doc comment不変） → cargo test実行 → verify実行 → meta記録

完了判定:

1. 3fileの内容照合レビュー完了（出典×条件の突合・excluded妥当性。存在確認・機械検証passで検証を終えない。skill §5検証）
2. verify (a)〜(e) pass（`mise run verify-conditions`。検査(f)は`--complete`時のみ有効で移行中は含まれない）+ `--list-untagged`出力とtmp/対照表の当該段対象領域fn listのdiffが空（付け漏れなし。§5.5）
3. `cargo test --test selector --test parser --test error_map` および json-contract分の `--test cli_list_read_send` 全PASS
4. 3fileのmeta検証記録（verified = true・verified_at・verified_by・test_command・source_hash = `git hash-object <source_file>`の値）

独立レビュー点位: 条件書3fileの独立subagentレビュー（内容照合）。パイロットで手順・id品質・tag様式・meta記録様式・1fn複数要件の統合様式（§3.4）を確定し、第2段以降の雛形とする。

### 9.4 第2段（中規模）

対象: cli-grammar（6）・read-send（5）・list-display（13）・rename（3）・add-remove（7）・resize（7）・docs-verb（7）

完了判定: 第1段と同様の4点に加え:

- `cargo test --lib` 3件PASS（src/app/list.rsのlib内tag付け確認）
- §8.3の残項目を含む出典列挙の対応決定が完了し、未test項目はexcluded記録 + tm起票済み

独立レビュー点位: 2批に分けて実施。cli系（cli-grammar・read-send・list-display・docs-verb）とbackend系（rename・add-remove・resize）。

### 9.5 第3段（remap系）

対象: layout-planner（15）・layout-generator（8）・companion-seed（14）・remap-sequence（20）・remap-cli（8）

完了判定: 第1段と同様の4点に加え:

- `tests/design/README.md`のR番号→id対応表がR1〜R50で空欄なく完成（§2.3の分割対応どおり）

独立レビュー点位: remap 5file一括の独立レビュー。test-plan §2.7旧本文のR表と対応表・各条件のid/source/given/expectの全行照合を含む（R番号対応の正確性が最大の検証対象）。

### 9.6 第4段（文書縮小 + 全締め）

作業:

- test-plan.md §2縮小・L4節集約（§6.1/6.2）
- L4節集約に伴うexcluded source欄locater更新: §2.8/§2.11/§3を参照するexcluded行が、§6.2対応表の旧節locater経由で引き続き有効であることを確認する（参照切替が必要なら当該条件書を更新）
- tests/README.md索引化（§6.3）
- tests/design/README.md完成（進捗100%・対応表最終化）
- 全15fileのmeta最終確認（verified = true・source_hashが現行実装と一致。不一致fileは再検証）
- mise.tomlのverify-conditions taskを`--complete`付きへ切り替え（§5.1）
- `docs/README.md`索引に`tests/design/`・verify scriptを記載（update-docs skillによる突合）

完了判定:

1. `cargo test` 133件全PASS
2. `mise run verify-conditions` pass（--strict-source込み。`--complete`組み込み後は検査(f)を含む全検査pass・`--list-untagged`出力が空）
3. test-plan縮小後も、既存のR番号参照（detailed-design・design-review・requirements-traceabilityの記載）からREADME対応表経由でid・条件へ到達できることの確認
4. 移行全体の独立レビュー完了（条件書・test-plan・README・verifyの一貫性・正本移譲の妥当性）

独立レビュー点位: 移行全体の最終レビュー。完了をもって移行taskをtm finishする。

### 9.7 二重正本期間の管理

- 第1〜3段の間、test-plan §2（旧正本）と条件書（新正本）が領域単位で並存する
- 移行期間中に要件変更が発生した場合: 旧§2該当節と、移行済みであれば担当条件書の両方へ反映する（update-tests skillの二段階委任で実施）。未移行領域は旧§2のみでよい
- 第4段の縮小で正本を条件書へ移譲し、二重状態を終了する。期間中の条件書変更にもverify passを必須とする

### 9.8 各段共通の完了条件

- 内容照合: 出典が要求する条件の漏れなし・条件がすべて出典に基づくことの突合を、機械検証passとは別に行う（機械検証passは前置必要条件）
- 全test PASS・verify pass・meta検証記録の3点が揃って段完了

## 10. 移行後の運用接続

### 10.1 update-tests二段階委任（常態運用への接続）

- 移行後はupdate-tests skillのworkflowがそのまま作動する。orchestratorは自分で条件書・test codeを書かず、前段subagent（条件書の追加・更新: test-structure skill §5）→ 後段subagent（test codeの追加・更新とtag付け・実行）へ委任し各段を検収する
- 前段検収は内容照合（id形式・sourceの出典が設計文書を指すこと・変更に対する条件の不足なし・除外の妥当性）。tomlの存在や機械検証passで検収しない
- 後段検収は条件×test対応・`mise run verify-conditions` pass・test全PASS

### 10.2 新規test追加時の手順

1. 条件書へ条件を追加する（idは§3規則で採番・sourceに必ず出典をつける）
2. test codeへtag付きで追加する
3. `mise run verify-conditions`を実行する（第4段完了後のmise taskは`--complete`付きのため検査(f)が有効で、tagのないtest fnはfailする。tag付け忘れはここで止まる）
4. 対象test実行・meta更新・`tests/README.md`索引更新

### 10.3 R番号で来た既存参照からの到達経路

- `docs/design/detailed-design.md`・`docs/design/design-review.md`・`docs/design/requirements-traceability.md`・review記録にはR番号（R1〜R50）での言及が残る。これらの文書は更新しない（本移行の対象外）
- 到達経路: R番号 → `tests/design/README.md`のR番号→id対応表 → 条件書の当該条件（source欄に旧節locaterあり） → covers tagでtest fn
- 逆引き（条件側から）はsource欄のR番号、またはidの `r<N>-` 接頭でgrep可能

## 11. リスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| tag付け漏れ（test追加時にtagを書かない） | 網羅検査をすり抜け条件とtestが遊離する | 検査(f)（`--complete`・第4段完了後にmise taskへ組み込み）が全`#[test]` fnのtag存在をfail検出。移行中は`--list-untagged`による対照表突合（§5.5）で代替。各段の完了判定にverify passを必須化 |
| 条件そのものの書き漏れ（test fnを見落とし条件化しない） | (a)(b)は通るがR番号・出典網羅が欠ける | 第0段のbootstrap対照表（fn数 = 133）との突合、READMEのR番号対応表の全行照合（第3段完了判定）、内容照合レビュー |
| id collision・area誤り | verify (c) failで移行が中断する | 全局id一意性とarea = stem一致を(c)で機械検証。衝突時は内容kebabの具体化で解消 |
| source_lines大量diff（第3段は65件一括） | review負荷 | (d)が自動同期のため手書き負担なし。段階適用によりdiff範囲を第3段に限定。書換は行置換のみでdiffが読める |
| verify scriptのtoml書換破損 | 条件書が壊れる | 書込み前に置換後text全体をin-memoryでparse検証し、検証pass時のみ書込む（§5.3）。repo fileへの直接書込みのためalcom snapshotも保険になる |
| verify遅延 | 更新loopの停滞 | 対象は15 toml + 数十rs fileのtext走査でms order。cargo testと独立実行のため懸念なし |
| 移行期間の要件変更（二重正本） | 旧§2と条件書の不整合 | §9.7の運用規則（両方へ反映） |
| lib内testの存在忘却 | src/側が無検証のまま残る | scannerがsrc/**を走査・list-display.tomlがlib 3件を管轄・READMEの範囲明示 |
| excluded記載漏れ・形骸化 | 除外が無検証で残る | 内容照合レビューでexcluded一覧との照合を必須項目化（§9.8） |

## 12. 関連資産

- 構造規則の正本: `~/.claude/skills/test-structure/SKILL.md`（schema・stable id・検証仕様・作成順序）・`~/.claude/skills/update-tests/SKILL.md`（二段階委任運用）
- 移行対象の実態: `docs/testing/test-plan.md`・`tests/README.md`・`tests/{unit,cli,fake_backend,fixtures}/`・`src/app/list.rs`・`Cargo.toml`（[[test]] 12宣言）
- 本移行の実施記録・各段の独立レビュー指摘: `docs/design/design-review.md`へ節を追加して記録する
