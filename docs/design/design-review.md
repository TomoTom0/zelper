# Design Review (Phase 4)

作成日: 2026-08-21
対象: docs/design/detailed-design.md（DD-1〜DD-12）をdevelopment-plan.md Phase 4の10観点でレビューした結果
運用: 指摘にはID（DR-n）を付け、disposition（設計に反映済み / 理由付き承認 / P1への先送り）を明示する

## 1. レビュー観点別の結論

### 要件の完全性

requirements.mdのP0全項目がDD-1〜DD-12のどこにあるかはrequirements-traceability.md §3のマップで追跡可能。acceptance criteria 14項のうち13項は設計で充足経路が確定している。残る1項（AC-8: overflow時のlayout反復）は`--overflow tabs`という明示modeで充足する（実験で「保存付き反復は不可能」と確定したための要件解釈変更。ユーザー承認済み・traceability §4.1に記録）。

### CLI構造の一貫性 / semantic wrapper原則

8 verbはいずれも「解決→計画→実行→検証」の多段構成で、`zellij action`の1対1短縮ではない。レビューで`resize`のgrammar不統一を発見し修正（DR-1）。

### first-argument verb原則

`completion`のみ例外（DD-1.1に文書化済み）。それ以外はverb開始。

### positional-primary / option-alternative原則

PANESPEC/TABSPEC/LAYOUTNAME/TEXTはpositional。代替解決（name/command/cwd/path/inline）はすべてoption。polymorphic positionalは不存在。`send`の`--`区切りでtargets/textの曖昧性を構文レベルで解消。

### multi-targetの一貫性

read/send/remove/addは対象集合を扱い、反復順序（visual order）・per-target結果（`results[]`）・exit status（部分失敗=6）を共通規則化。renameの複数対象はv1から外す（DR-5）。

### remap preservation

保存可能な全ケース（fill mode・nest・tabs modeの第1 instance）で保存を実証済みの経路のみ使用。破壊が入るのは`--overflow tabs`のoverflow pane（明示flag）と`--embed-floating`なしのfloating pane（preflight errorで停止）のみ。黙示kill経路は設計上存在しない。

### failure/rollbackの現実性

atomicityを主張しない。部分失敗は「実行済み/失敗/未実行」の報告とdry-run再実行・snapshot復旧の案内で対応。zellijにrollback primitivesが存在しない以上、これが最も嘘のない設計（要件8の指示と整合）。

### testability

ZellijBackend traitでfake差し替え可能。CLI契約テストはfake zellij shim、統合テストはpodman隔離基盤（Phase 1で実証済み）。remap計画は純粋関数としてfake backendで全worked examplesを検証可能。

### 互換性仮定

最小0.44.1はCHANGELOG由来の**仮定**で、実機検証は0.44.3のみ（DR-6）。

### その他の発見

DR-2〜DR-8は下表のとおり。

## 2. 指摘事項とdisposition

| ID | 指摘 | disposition |
|---|---|---|
| DR-1 | `resize PANESPEC grow ...`（nounなし）と`rename pane/tab`（nounあり）のgrammar不統一 | **設計に反映済み**: `resize pane PANESPEC ...`に修正（DD-1.2/1.6） |
| DR-2 | `remap`の`--tab`と`--session-scope`の排他が明記されていなかった | **設計に反映済み**: DD-1.5に追加 |
| DR-3 | error.classとexit statusの対応表が存在せず、Layout系classのcodeが未定義だった | **設計に反映済み**: DD-4.3に対応表を追加 |
| DR-4 | layout_dir判定にconfig.kdlの読み取りが必要なことがDD-3.4に明記されていなかった | **設計に反映済み**: config.kdlの`layout_dir` node読み取りを明記 |
| DR-5 | `rename`が要件2.4「support multiple targets where useful」をv1で満たさない | **理由付き承認**: basic-design 4.4も「単純semanticを証明してから追加」としており、bulk renameは機械的反復で後付け容易。P1に記録（実装後の拡張） |
| DR-6 | 最小0.44.1サポートが実機検証されていない（検証は0.44.3のみ） | **理由付き承認**: 互換性宣言は「target: 0.44.1以上、検証済み: 0.44.3」と明記する。0.44.1実機検証はPhase 7の任意項目とする |
| DR-7 | fill mode（M<N）で空slotに既定shellが起動し、paneが増えることが直感に反しうる | **理由付き承認**: zellijのnative挙動（slotは埋まる）。READMEとdry-run出力で明示する。`--no-fill`（余りslotのtruncate）はlayout KDLの改変が必要でv1対象外。P1に記録 |
| DR-8 | `resize pane STEPS`の単位が「resize操作1回」であり行/列数でない（zellijが刻み幅を非公開） | **理由付き承認**: helpに「1 step = 1 resize operation」と明記。equalizeは幾何検証付きのため実用上の問題は限定的 |
| DR-9 | `read --tail N`と`--full`の組み合わせ semantics が未定義だった | **設計に反映済み**: DD-6に「--tailは取得済み内容（viewportまたは--full時scrollback）の末尾N行に適用」と定義 |
| DR-10 | `remove tab --empty`の「空」定義（floating pane の扱い）が曖昧だった | **設計に反映済み**: DD-11に「selectable な pane（tiled+floating両方）が0個」と明記 |

## 3. レビュー後の設計の状態

- DD-1〜DD-12はDR-1〜DR-4・DR-9・DR-10の修正を反映済み
- DR-5・DR-7はP1（requirements.md P1リストと整合。実装後に再評価）
- DR-6・DR-8は文書化された既知制限として受け入れ
- 設計上の要件違反・黙示の弱体化は不存在。実装フェーズ（Phase 6）に移行可能

## 4. 事後レビュー（Phase 6実装・Phase 7統合後、subagent独立レビューによる）

実装完了後、作成者と別のsubagentによる独立レビュー（設計文書整合・実装コード）と実機統合テストを実施し、以下を確定した（MR-n）。上位DRとは独立に付番する。

| ID | 指摘 | disposition |
|---|---|---|
| MR-1 | `remap --dry-run --embed-floating`がdry-run判定前にfloating paneをtiled化し非破壊契約（DD-12・要件3.7）に違反。加えてtoggle後の状態再取得がなくfloating paneがsourceに含まれない二重bug | **修正済み**: dry-run時はtoggleせず計画にのみ含める。実行時はtoggle後に再取得。回帰テスト追加（fake_remap.rs） |
| MR-2 | L3テストが環境変数`ZELLIJ_SESSION_NAME`を隔離せず、zellij session内で実行すると失敗する | **修正済み**: test helperでenv_remove。tests/READMEに実行条件を記載 |
| MR-3 | `remove`全失敗時のexit codeがDD-4.3（=5）と不整合（=7で実装） | **修正済み**: read/sendと同じ5に統一 |
| MR-4 | DD-3.4/DR-4がconfig.kdlの`layout_dir`読み取りを謳うが実装は`ZELLIJ_LAYOUT_DIR`>既定dirのみ | **文書を実装に統一**: config.kdl読み取りはv1対象外。READMEの既知の制限に明記 |
| MR-5 | equalize非収束時の契約がtraceability（error）とDD-9・実装（note付き成功）で不整合 | **note付き成功に統一**: 近似である旨を出力。traceability/test-planを更新 |
| MR-6 | DD-4.2に成功時envelopeの`data` keyが未記載 | **DD-4.2に追記**（stable fieldsに`data`を追加） |
| MR-7 | DD-12が`resize equalize`/`add`のdry-runを謳うが未実装・DD-1.2にも不存在 | **P1に確定**: 要件3.7はSHOULD。DD-12をv1対象（remap/remove）に訂正 |
| MR-8 | DD-3.2 traitが実装と乖離（list_layout_dir不在・追加メソッド・filter引数） | **DD-3.2を実装に統一** |
| MR-9 | DD-1.2 `remove tab`構文・`list`の`--tab`説明が実装と乖離 | **DD-1.2を更新** |
| MR-10 | DD-10.2に旧稿残存（floating扱いがDD-10.1と矛盾・重複行） | **削除しDD-10.1に一本化** |
| MR-11 | READMEが「検証済み: 実機統合テスト」と過大表明（当時remap未検証）・geometry検証/snapshot添付の過大記載 | **修正済み**: 検証範囲を具体的に記載。実装に合う文面に訂正 |
| MR-12 | 表記ゆれ「zeller」（30箇所以上） | **一括修正** |
| MR-13 | 統合テストS9: 生成KDLの値がbare形式（`command=sleep`）でzellij 0.44.3 parserに拒否 | **修正済み**: 生成時の強制quote（KdlEntryFormat.value_repr）。capabilitiesのinline layout行に実測追記。回帰テスト追加 |
| MR-14 | 空tabはzellij 0.44.3で作成不能（new-tabが必ずpaneを作る） | **既知の検証限界として記載**: `remove tab --empty`の実削除は代替検証のみ（tests/README） |
| MR-15 | traceabilityの見出し番号・「最優先実験」残存・percentage分類の過大 | **修正済み**（見出し振り直し・実験完了反映・v1未提供に分類訂正） |

MR-1〜MR-3・MR-13は実装修正を伴い、修正後に全テスト（63件）と統合S7〜S9の再実行で全PASSを確認。

### 4.2 実装コードレビュー（第2ラウンド）指摘とdisposition

レビュー時点で既に§4.1で修正済みだったもの: floating dry-run非破壊（MR-1相当）・toggle後再取得（同）・remove全失敗exit 5（MR-3相当）・test hermetic性（MR-2相当）。

| ID | 指摘 | disposition |
|---|---|---|
| MR-16 | kdl crate（KDL v2）はzellij layout常用のbare `true`/`false`（`borderless=true`等）をparse拒否し、実layoutが`LayoutInvalid`になる | **修正済み**: parse前の字句正規化（文字列外bare boolの強制quote）。回帰テスト追加 |
| MR-17 | `--json`指定時の失敗にerror envelopeが出ない（DD-4.2不履行）・部分失敗のresultsが失われる | **修正済み**: mainのerror分岐 + `ZelperError::with_data`（error envelopeの`data`にresultsを同梱） |
| MR-18 | slot数Nをlayout全tabの合計で算出するが、`--apply-only-to-active-tab`は先頭tabのみ適用するためoverflow判定が狂う | **修正済み**: N=先頭tab（base_subtree）のslot数。回帰テスト追加 |
| MR-19 | tabs mode再作成検証がsession全体のcommand部分一致で同command paneと交差matchする | **修正済み**: instance作成tab内での検証（tiled pane数 + command一致）に限定 |
| MR-20 | `remove tab --empty`のTABSPEC解決失敗を黙って捨て、削除対象が「全空tab」へ暗黙拡大する | **修正済み**: `resolve_tab`による解決（不能ならerror）。回帰テスト追加 |
| MR-21 | equalizeのpositional不在ID黙除去・異tab混在無検証・`expect("target")`のpanic経路 | **修正済み**: NoTarget/Preflight error化・Result化 |
| MR-22 | `remap --tab X --dry-run`が`go-to-tab`で表示を切替する（dry-run非破壊違反） | **修正済み**: dry-run時はtab切替せず、対象tabは解決済みIDで処理 |
| MR-23 | dry-runに実行予定操作列が出ない（DD-10.4(d)）。remap/addの途中失敗に部分適用状態が載らない（DD-10.5/11） | **修正済み**: `plan_operations`（dry-run表示）・`partial()`/`add_partial()`（実行済み情報をerrorに同梱） |
| MR-24 | tabs modeのtab名がKDL依存で`--path`/`--inline`時に`remap-2`になる。DD-10.3 step6の`rename-tab-by-id`未使用 | **修正済み**: 作成後に`rename-tab-by-id`で確定 |
| MR-25 | shell pane再作成時の`--cwd`が生成KDLで失われる（injectがcommand空でreturn） | **P1**: shell paneのcwd保持。影響限定的（shellの開始dirは既定で作成tabのcwdに従う） |
| MR-26 | fill/nest modeがlayoutをそのまま渡すためbarを持たないlayoutでbarが消える（DD-3.3のbar明示生成から逸脱） | **仕様確定**: barは「対象layoutの定義に従う」（layoutがbarを含めば維持される）。bar自動注入はP1。capabilities §4の実測（layout明示なしではbarは消える）どおりの挙動で、黙示の破壊はない |
| MR-27 | layout_dirのconfig.kdl読み取り未実装 | **MR-4と同一**: v1対象外・`ZELLIJ_LAYOUT_DIR`運用をREADME既知の制限に記載済み |
| MR-28 | `resize equalize`/`add`に`--dry-run`が無い | **MR-7と同一**: P1 |
| MR-29 | timeout 30秒（DD-3.1の10秒から逸脱）・`version_supported`等dead code・未来major判定なし | **修正済み**: 10秒化・削除・`check_capability`に未来major拒否を追加 |
| MR-30 | `send`のtext結合コメント不正確・`--yes`+`--dry-run`併用の注意表示なし・`--tail`のtest不在 | **修正済み**: コメント訂正・注意行追加・tail testはP1（apply_tail経路は実装済み・test追加は軽微） |

**検証**: 修正後 L1〜L3テスト71件合格・clippy警告0・統合S5〜S10再実行でFAIL 0件（実行記録はrepo管理外の検証作業dirに残置）。

### 4.3 PR#1外部レビュー対応の独立レビュー（subagentによる）

PR#1（初期取り込み）への外部レビュー指摘5件（plugin leaf slot indexing・再作成command検証・floating変更タイミング・shellのみpaneのcwd・検証失敗JSON envelope）への対応diffに対し、作成者と別のsubagentが独立レビューを実施。**P1/P2相当の指摘なし（承認可）**。検証内容: diffとworking treeの完全一致・テスト77件合格・clippy/fmtクリーン・回帰検出力の実証（各src修正を一時revertして対応testがFAILすること・fake強化単体では従来挙動を壊さないことを確認後に復元）。

| ID | 指摘 | disposition |
|---|---|---|
| MR-31 | 子なし`tab`/`layout` node（braceなし）でcount（walk_slotsはskip）とinject（leaf扱いでslot消費）の規則が非対称。`layout { tab \n pane \n pane }`形式で生成KDLのcommandが1つずれる（scratch実行で実証。PR#1 fix 1と同族の既存乖離） | **修正済み**: inject_walkのdecisionに「子なしlayout/tabはskip」を追加しwalk_slotsと対称化。回帰テスト追加（childless_tab_node_does_not_consume_slot_index） |
| MR-32 | plugin nodeのconfig子node（zjstatus形式の`format_left`等）がhas_nested判定でnest扱いになり、config node自体がslotに数えられ・inject対象になる（scratch実行で実証。count/injectは対称のためindexずれ無し。PANE_CONTENT_NODESはplugin/argsのみの既存構造） | **修正済み**（TASK-23）: zellij parser実装（zellij-utils/src/kdl/kdl_layout_parser.rs）を調査し規則確定 — plugin配下の子nodeはすべてplugin configurationとして文字列化されslotを形成しない・layout直下のbare pluginは実zellijで無視される・tab直下のbare pluginはInvalid tab property error。walk_slots/inject_walkともplugin nodeをconfig子nodeの有無にかかわらずleaf扱いとし再帰抑制。加えて実機検証（S11）で、生成KDLはbase subtreeをtab配下に置くためlayout直下bare pluginが残るとparse errorになると判明し、inject_walkでlayout/tab直下のbare plugin nodeを生成KDLから除去（pane run block内は保持）。回帰テスト追加（plugin_config_children_do_not_consume_slot_index・fail-first実証済み）・L4実機検証S11でbare/wrapper両形式12/12 PASS |
| MR-33 | FakeBackendのnew_tab emulateがzelper生成KDLをzelper自身のextract規則で解釈する循環構造。injectのslot意味論が実zellijと乖離していてもfakeは同じ解釈を再現する。layout parse失敗時に黙って1 bare paneを生成（実zellijならerror） | **修正不要**: 既知のテスト限界として記録。slot indexずれの回帰検出は生成KDLの再parse（extract）で独立担保しており、実zellijとの意味論一致はL4実機検証で補完する運用を維持 |
| MR-34 | `--embed-floating`時のsource sortがtoggle前（floating geometry）基準。tabs modeでembed後の実際の並びと異なる順序でoverflow対象が選ばれうる | **修正不要**: DD-10.2記載のとおりdry-run/実行の計画統一のための意図的選択。fill modeはzellij側が割当するため影響は報告のみ。実運用で問題報告があれば再検討 |
| MR-35 | session-scope × `--json`はtab毎にenvelopeが出る（成功時も。部分失敗時は成功tab分+最終error envelope） | **修正不要**: tab毎responseという仕様意図と解釈。単一document契約を全verbで厳密化する場合はDD-4.2の明記が必要（必要なら別起票） |

**検証**: 修正後 L1〜L3テスト77件合格・clippy警告0・fmt差分なし。

**MR-32修正（TASK-23）の独立レビュー（codex read-only・2026-08-23）**: slot再帰抑制・生成KDLからのbare plugin除去に対し、作成者と別model（codex）で独立レビュー。**P1/P2/P3指摘なし（承認可）**。count/inject/extractのslot index対称性・bare plugin除去の限定性（pane run block・`floating_panes`・templateとの組合せで過剰除去・除去漏れなし）・zellij 0.44.3 parserソースとの静的照合・テストのfail-first検証力を確認。L1〜L3テスト78件合格・clippy警告0・fmt差分なし。

### 4.4 agent配布物（TASK-24）の独立レビュー（subagentによる）

`zelper docs` verbと配布物（SKILL.md・snippet・docs/agent/README.md）に対し、作成者と別のsubagentが独立レビューを実施。P1なし、P2: 2件、P3: 8件。

| ID | 指摘 | disposition |
|---|---|---|
| MR-36 | DD-1.3が「v1はTABSPEC=IDのみ」と記載する一方、実装（`resolve_tab`）は`rename tab`/`remove tab`のpositional含む全TABSPECで一意なtab名を受容し、正本間で矛盾 | **修正済み**: 実装・README・SKILL.mdが一致しているためDD-1.3を実装に合わせ更新（TABSPEC = ID or 一意な名前の共通解決） |
| MR-37 | docs/agent/README.mdがversion管理外の`tmp/task24/`を参照（他cloneでlink切れ・形式比較の出典が追跡不能） | **修正済み**: `docs/research/research_agent-docs-formats.md`へ移設しdocs/README.md索引に登録 |
| MR-38 | 軽微表記（P3群）: snippetのexit status略記・SKILLの「parse error」表記・「候補列出つき」・単一pane削除例の`--yes`誤学習リスク・list sessions JSON制限の記載漏れ・README:66等の要約省略・snippet verb一覧のcompletion省約説明・適用手順見出しとcommandの不一致 | **修正済み**: exit status表記統一・usage error表記・例修正（`remove pane 12`）・`list sessions`制限追記・「主要verb一覧」説明修正・CLAUDE.md用command併記。dry-run細部（README:64-66）はskillの分量制約上readme参照でカバー |

**検証**: 修正後 L1〜L3テスト78件合格・clippy警告0・fmt差分なし。`zelper docs readme|skill|snippet`の出力が正本と一致することをCLI契約テスト（tests/cli/docs.rs）で検証。

**TASK-25追記（2026-08-24）**: docs verbを `docs readme | llm usage|skill|snippet` の2段構造化（llmのみ第2階層。ユーザー指示）。LLM向けusage参照（`docs/agent/llm/usage.md`: 全verb文法・排他規則・JSON envelope全field・error class 11種とexit対応・安全gate・誤用対）を追加。**検証訂正**: TASK-24時点でtests/cli/docs.rsに`[[test]]`宣言が無くdocs verbテストが実行されていなかった（78件に未含入。検証報告が実態と不合だった）。宣言追加により85件合格で実検証完了。教訓: tests/直下以外の新規test fileはCargo.tomlの`[[test]]`宣言要。

**TASK-26追記（2026-08-24）**: docs配布物正本を `docs/usage/` カテゴリへ再構成（README.md・llm.md・skill/SKILL.md・snippet.md。docs/の文書種カテゴリ体系 design/research/testing に参入、ユーザー指示）。docs/agent/カテゴリは廃止。include先・索引・適用手順・テストパス更新、85テスト合格。TASK-25/26変更に対する独立レビューは本項とは別に実施（結果は下記追記予定）。

**TASK-25/26の独立レビュー（2026-08-24）**: docs verb 2段化・docs/usage/再構成・llm.md新規執筆に対し、作成者と別のsubagentが独立レビュー（実装突合・実binary検証・85テスト確認込み）。P1: 2件、P2: 5件、P3: 4件。全て対応済み。

| ID | 指摘 | disposition |
|---|---|---|
| MR-39 | llm.md事実誤認2件: (a) destructive gateを「exit 2」と記載（実態はPreflight exit 7。llm.md自身の対応表とも矛盾）(b) remap --session-scopeがresults[]を出すと記載（実装はtab毎に独立envelopeを出しresults[]を生成しない） | **修正済み**: (a) Preflight exit 7に訂正 (b) results[]対象をread/send/removeに限定し、session-scopeはtab毎envelope・失敗時最初のerrorで中断と記載 |
| MR-40 | llm.md過大一般化2件: (a) 「1つでも失敗あれば全体exit 6」（実態: 一部失敗=6・全失敗=5）(b) candidatesを「対象解決失敗時のみ」と記載（実態: session曖昧時のsession名・remap floating検出時も付く。省略条件は空か否か） | **修正済み**: 両記述を実装どおり訂正 |
| MR-41 | docs/usage/README.md: cp例が`~/.claude/skills/skill/`を作りfrontmatter name:zelperと不整合。旧ファイル名（usage.md/snippet_zelper-guide.md）残存2件。DD-1.1のdocs文法が旧1段構造のまま | **修正済み**: cp先をzelperに修正、旧名を現行名（llm.md/snippet.md）に更新、DD-1.1文法を`docs readme | llm usage|skill|snippet`に更新 |
| MR-42 | 軽微4件: add行の[--json]欠落（llm.md他verbと不整合）・remove tab --empty時のTAB省略可の未記載・SKILL.md verb一覧にdocs行なし・--allの意味限定（selectable terminalのみ）未記載 | **修正済み**: 全てllm.md/SKILL.mdに反映 |

**検証**: 修正後85テスト合格・clippy警告0・fmt clean。レビュアーによる実binary検証（docs verb 4出力が正本とbyte一致・排他規則違反のexit 2実機確認）込み。

### 4.5 TASK-31設計レビュー（第三者配布整備）

レビュー日: 2026-08-26。対象: TASK-31実装設計書（repo管理外の作業dirに置かれた正本）。作成者と別のsubagentによる独立レビュー。指摘9件（P2: 3件・P3: 6件）、すべて修正採用（design-review.md本文の§2.8文言調整で当ファイル:117も対象になるため、当項目の追記自体が§2.8の実装に含まれる）。IDはT31R-n。

| ID | severity | 指摘 | disposition |
|---|---|---|---|
| T31R-1 | P2 | tmp/参照の文言調整対象に`src/layout/mod.rs:327`（doc comment内のrepo管理外実行記録へのtmp/参照）が漏れていた。§1修正file一覧・§2.8表からも不存在で、src内のdangling参照だけが調整されないまま残る | **修正**: §1一覧・§2.8表へ追加し、§0の再確定記述にsrc側1箇所を含めた |
| T31R-2 | P2 | §5のpublic化コマンドが現行gh CLIの必須flag（`--accept-visibility-change-consequences`）を欠き、手順実行時にconfirmで失敗する | **修正**: flag付きのコマンドへ更新 |
| T31R-3 | P3 | §5のtag打ち直し手順にrelease残存時の削除がなく、`gh release create`成功後に後続stepが失敗した場合はtag打ち直し再pushが既存releaseとの重複で失敗する | **修正**: 先行して`gh release delete vX.Y.Z --yes`を実行する旨を追記 |
| T31R-4 | P3 | mise ubi backendのasset選択機構の記述が不正確。archiveが1つのみの場合はplatform判定なしで無条件選択されるため、macOS/arm64でも導入可能（誤導入）となるが、その留意が§2.4にない。またubi backendはmiseでdeprecated warning表示中 | **修正**: §2.4 mise節にLinux x86_64向けである旨の留意1行を追加。§2.6に選択機構（拡張子filter、archive 1つなら無条件選択、複数ならplatform判定）の正確な記述を追記 |
| T31R-5 | P3 | §2.6のworkflow YAMLが`actions/checkout@v4`（旧major） | **修正**: `@v5`へ更新 |
| T31R-6 | P3 | §5前段にTASK-31 PRのhead branchがmerge後remoteに残存しているかの確認手順がなく、残ったままpublic化すると公開branchが増える | **修正**: 確認手順（残っていれば削除）を§5前段へ追記 |
| T31R-7 | P2 | §4 verify表に§2.7/2.8文言調整箇所の設計・実file間の一致確認（docs/src内`tmp/`・`/home/tomo`言及のgrep残存確認）がなく、文言調整の実装漏れをverifyで検出できない | **修正**: verify表へ追記（規約・規則としての言及とrepo管理外である旨の明示は残存してよい旨を明記） |
| T31R-8 | P3 | §0の「`.claude/settings.local.json` は untracked 存在」が実態と不正確。本machineではuser global gitignore（`~/.config/git/ignore`）によりignored状態であり、repo .gitignore追加の目的は他clone・CI環境での保護 | **修正**: 実態どおりの記述へ差し替え（§6リスク欄の同旨記述も整合） |
| T31R-9 | P3 | §0のtmp/参照計数が列挙ベースで不正確。実測はdocs側8箇所（うちdesign-review.md:142はMR-37指摘文の歴史的引用のため変更不要）・research側6箇所・src側1箇所。また「規約内のtmp/言及（CLAUDE.md:15・.gitignore:2）は調整対象外」の明示が無い | **修正**: grep実測値に正確化し、対象外明示を追加 |

**検証**: 指摘9件すべて設計書へ反映済み。docs/src/testsのgrep実測と計数の一致を確認（レビュー時点の`/home/tomo`言及はzellij-capabilities.md:52の1箇所のみ。実装で一般化済み）。実装は反映後の設計書を正本として実施。

**段階8コードレビュー（codex・2026-08-26）**: TASK-31実装（LICENSE-MIT/LICENSE-APACHE・Cargo.toml metadata・README install章・.gitignore・docs/src文言調整・release workflow）に対し、codex（read-only）でコードレビュー。指摘5件（必須1件・推奨4件）、全件disposition=修正。IDはCR-n。

| ID | severity | 指摘 | disposition |
|---|---|---|---|
| CR-1 | 必須 | Release archiveがLICENSEを同梱せず、Apache-2.0 §4(a)のObject form再配布時のlicense写し提供義務に非対応 | **修正**: package stepをstaging dir方式へ変更し、archiveに `zelper` / `LICENSE-MIT` / `LICENSE-APACHE` の3fileを同梱 |
| CR-2 | 推奨 | `actions/checkout@v5` がtag参照（tag移動・supply chain改変余地）で、かつcredential保持がdefault有効 | **修正**: v5系最新release v5.1.0のcommit SHA固定（`fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09`）+ `persist-credentials: false` |
| CR-3 | 推奨 | tagとCargo.toml versionの不一致がworkflowで機械検出されない。`gh release create` がtag実在を検証しない | **修正**: build前にtag/Cargo.toml version一致検証stepを追加（`cargo metadata` との比較）。`gh release create` に `--verify-tag` を追加 |
| CR-4 | 推奨 | LICENSE-MITの文言は正本一致だが折り返し位置（改行）が手整形されておりverbatimでない | **修正**: rust-lang/rust の `LICENSE-MIT` を再取得し、差分が著作権行 `Copyright (c) 2026 TomoTom0` の1行のみになるよう改行・trailing newline含めverbatimで再配置 |
| CR-5 | 推奨 | README install章のdownload手順がReleases page訪問と手動downloadを前提としcopy-paste完結しない（asset名にversionを埋める案内だと陳腐化も） | **修正**: `releases/latest/download/` URL（常に最新版を指す）による実行可能例へ変更。同一dirへ2file downloadする旨・archiveへのLICENSE同梱（CR-1反映）を本文に明記 |

**検証**: 修正後 `cargo fmt --check` 差分なし・`cargo clippy --all-targets` 警告0・`cargo test` 85テスト合格・`actionlint .github/workflows/release.yml` pass・LICENSE-MITとrust-lang/rust正本のdiffが著作権行のみ・新しいpackage手順（staging dir・3file同梱）でのtar・sha256sums.txt再生成により `sha256sum -c` pass・tar内容3file確認。設計書（§2.1・§2.4・§2.5節以降のworkflow YAML・§4・§5・§7）も修正内容へ追従更新。

### 4.6 TASK-37設計レビュー（remap再設計 v2・companion plugin構成）

レビュー日: 2026-09-03。対象: DD-10 v2全面改訂とDD-1/DD-3/DD-12・module構成の更新（TASK-37設計作業分。`docs/design/detailed-design.md`）。作成者と別model（codex・read-only sandbox）による独立レビュー。指摘9件（すべてP1）・dispositionは修正8件・却下1件。IDはR-n。

明示的に指定したレビュー観点のうち「移動sequenceの不変条件」（group 0 inbound先行によるanchor自動close防止・pane id基準の新規tab特定によるtab id再利用race防御・長寿命tab id参照の排除）は**問題なし**判定（指摘化されず）。

| ID | severity | 該当節 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| R1 | P1 | DD-10.4 | permissions.kdl追記時に既存fileの末尾改行が無い場合の区切り保証がない（前行への連結でKDL破壊） | **修正** | 追記前に改行終端を保証する規則を明記 |
| R2 | P1 | DD-10.4 | parse→更新→書戻しの間にzellij他processが書換えると、last-writer-winsで他pluginの権限を失わせうる（「他pluginの権限を壊さない」目的と両立しない） | **修正** | 書戻し直前の再読込・内容比較による変更検知 + bounded retry + atomic rename（temp+rename）を仕様化。retry打ち切り後も競合する場合は書込まず中断 |
| R3 | P1 | DD-10.4/10.7 | 起動済みserverがseedを反映しない場合の検知が最初のbreak pipeまで遅れ、CLI失敗後にserver側で遅延実行されてpaneが動く恐れ | **修正** | 状態変更前のnon-mutating probe pipe（権限を要するがuser状態を変えない操作。E6で権限付与時に動作確認済みのplugin自身のpane title書換え）をsequence先頭に追加。probe不成立は一切の状態変更前に中断。中断時の報告にpipe遅延実行の注意を追記 |
| R4 | P1 | DD-10.3 | build.rs plugin buildの仕様不足（workspace再帰build防止・Cargo.lock尊重・cargo解決・rerun条件・artifact path/名前・escape hatch指定時の取扱） | **修正** | `-p zelper-companion-plugin` 明示・`--locked`・`CARGO` env変数によるcargo解決・`rerun-if-changed=plugin/`と`rerun-if-env-changed=ZELPER_PLUGIN_WASM`・artifact target pathと名前・escape hatch指定時はOUT_DIRへcopyしてからinclude_bytes!する統一方式を明記 |
| R5 | P1 | DD-10.3/DD-3.1/3.5 | tile `=0.44.3` exact pinと「最小0.44.3・future major以外は許容」のversion規則が不整合 | **修正** | compatibility policyとして「実行時要件は >=0.44.3。実証済み組合せはzellij 0.44.3 + tile 0.44.3のみ。0.44.4以降等の未実証組合せはprobe + postcondition検証により安全に失敗する設計。新seriesでの互換性確認のたびに再実証」をDD-3.5とDD-10.3に明記 |
| R6 | P1 | DD-1.2/1.5・DD-10.2 #5/#11 | deprecated no-opのwarning 1行の出力先が未規定（stdout出力だと`--json`の単一envelope契約を破壊） | **修正** | warningはstderr固定（--json時も）。clap側のhelp非表示化と旧値 `nest` / `tabs` 受理を固定するtest要件をDD-12に追加 |
| R7 | P1 | DD-10.6/10.8/10.9/10.13 | 要件「決定論的かつ文書化されたpane順序」と、同一command run・複数shell paneのslot順非保証の記述が矛盾 | **修正** | 決定論性の保証範囲を「group（tab）所属と、run一意なpaneのslot対応」までと明示し、同一run内のslot順は保証外（zellij run一致照合の限界）である旨を要件例外としてDD-10に明記 |
| R8 | P1 | traceability・README・usage配布物・test-plan | 旧仕様の残留（`--overflow tabs`記述・version 0.44.1前提・S9再作成検証の設計依拠等） | **却下（設計外）** | TASK-38（test-plan・条件書）・TASK-40（README・usage配布物・traceability・skill）で対応予定。本レビューの対象はdetailed-design.mdの設計妥当性に限定 |
| R9 | P1 | 現行実装との乖離 | 実装が旧動作のまま（`MIN_SUPPORTED=(0,44,1)`・`--overflow tabs`のkill/recreate実装残存）で、設計文書だけが先行して乖離 | **修正** | DD-10 v2冒頭に「本v2は未実装の設計である。実装はTASK-39。実装完了までtraceabilityのimplemented表記は旧実装に対応付けたまま」である旨を明記。実装・文書の切替はTASK-39/40で行う |

**検証**: R1〜R7・R9の修正を`docs/design/detailed-design.md`へ反映済み（文書変更のみ。src/testsは対象外。実装はTASK-38/39）。

### 4.7 TASK-39 コードレビュー（remap v2実装）

レビュー日: 2026-09-05。対象: TASK-39実装（companion plugin・remap v2本体・build.rs・permissions seed・CLI。レビューlog: repo管理外 `tmp/260905_review_code_remap_task39.log`）。作成者と別model（codex・read-only sandbox）による独立レビュー。P0（kill/recreateの要件違反）なし。指摘7件（P1×3・P2×4）+回帰テスト不足。IDはC-n。対応はTASK-41。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| C1 | P1 | src/app/remap.rs | `--embed-floating`実行時のtoggle対象が`p.is_floating`のみで、probe自動launchのnon-selectableなcompanion plugin paneまでtiled化する。preflightはselectable terminal限定（DD-10.5/10.3違反） | **修正** | toggle対象をpreflightで特定したselectable floating terminal paneのID集合のみに変更（live再列挙を廃止しID基準に）。DD-10.5に「tiled化対象はpreflightで特定したselectable floating terminalのみ」を明記。r8テストの「floating paneが一つも残らない」誤期待を修正（plugin paneはfloatingのまま残る）+plugin paneをtoggleしない回帰assert追加 |
| C2 | P1 | src/companion.rs | permissions.kdlの「再読込一致→rename」間のTOCTOU残存。zellij起因の書換は外部からlock不可能で、atomic renameはlast-writer-winsを防がない | **緩和+文書化** | (a) zelper自身の並行実行に対するadvisory lock（flock・permissions.kdl同dirの`.zelper-seed.lock`。Dropでunlock+削除）で読込→書込区間を直列化、(b) DD-10.4に残リスク（windowは再読込→renameの最小区間に縮小済み・zellijはrequest_permission呼出毎に再読込するため喪失時は再requestで回復、黙示の恒久喪失ではない）を明記。直列化の回帰テスト（BがAのlock解放までblockすることの所要時間固定）+残留lock fileの追試を追加 |
| C3 | P1 | src/app/remap.rs | polling deadlineが`cond()`（list_panes等）の所要時間を考慮しない。deadline超過後の条件成立を成功扱いしうる・sleepが残時間を超過する | **修正** | `poll`をbackend呼出ごとにdeadline再検査する形に修正（超過後のcond真は成立扱いにしない）・sleepは残時間でcap。fakeに`slow_list_panes_after_pipe`注入を追加し「10.3s遅延したlist_panesがprobe title成立を返す系列では中断する」回帰テストを追加（fail-first確認済み） |
| C4 | P2 | src/companion.rs | temp fileの`write`自体の失敗時に部分書込tempが残留する（rename失敗時のみcleanupしていた） | **修正** | wasm extract・permissions seed両方のwrite失敗経路でbest-effortの`remove_file`を追加。temp pathをdirectoryで塞いでwrite失敗を発生させ、新規fileを残留しないことのテストを2件追加 |
| C5 | P2 | src/app/remap.rs | 新規tab名postconditionが「同名tabの存在」のみで、target_j自身のIDと名前の対応を検証していない | **修正** | `tabs_after`から`id == target`のtabを取得して名前比較する形式に変更（欠損・不一致をmissingへ）。fakeに`mute_rename_tab`（成功扱いだが名を変えない）注入を追加し、既存同名tabがあっても誤通過しない回帰テストを追加 |
| C6 | P2 | src/app/remap.rs / tests/unit/layout_planner.rs | quoting warning判定が`"`と`\`限定で、`'`・改行・制御文字を含むpane_commandがwarningなしでargv破壊しうる | **修正** | warning条件を`"`・`'`・`\`・改行・制御文字に拡張（`needs_quoting_warning`）。`bash -lc 'echo a b'`・改行込み・ESC込み・通常引数の実例テストを追加 |
| C7 | P2 | src/main.rs | `--session-scope`と`--overflow`同時指定でdeprecated warningが2行出る | **修正** | 指定された廃止optionを列挙した1行に統合。両指定時もstderr 1行・stdout単一envelopeのテストを追加 |
| 補足 | -（観点2） | - | held/exited paneの専用回帰テスト不足 | **仕様確認の上テスト追加** | detailed-design該当節（DD-10.5）を確認した結果、旧来の定義は「selectable+tiled terminal」でexit hold扱いが未規定だったため、「exited/is_heldのpaneは実行runを持たずrun一致照合対象にならないためsource外とし元tabに残す（paneは生存）」と仕様確定してDD-10.5へ明記し、`is_remap_source`へ反映。除外されること・kill/移動されないことの回帰テストを追加 |

**検証**: 修正後 `cargo fmt --check` 差分なし・`cargo clippy --workspace --all-targets` 警告0・`cargo test` 119テスト合格（110+追加9）。回帰テストの検出力実証: C2（flock無効化でc2テストFAIL）・C3（旧poll logicでc3テストFAIL）を一時revertで確認後に復元。

### 4.8 TASK-43 方針変更の記録（廃止optionの即時削除）

TASK-37レビュー時点（§4.6 R6・DD-10.2 #11）では廃止option（`--session-scope` / `--overflow nest|tabs`）を「当面hidden no-opで受理しv0.2で削除」としていたが、公開直後のv0.1.xであること・旧`--overflow tabs`が破壊的再構成を期待する指定であることから、no-op受理は旧scriptの意図を無言で別意味に解釈する危険が移行期間中も残るため、即時削除に方針変更した（TASK-43。2026-09-05実施）。両optionの指定はusage error（exit 2）。§4.6 R6・§4.7 C7の指摘内容（warning出力先・warning統合）はoption削除により問題自体が消滅。DD-1.2/1.5・DD-10.2 #5/#11・DD-12・test-plan §2.7 (g)・README・usage配布物は削除済み仕様へ更新済み。

### 4.9 TASK-44/45 コードレビュー（EXITED session解決・list panes CWD列）

レビュー日: 2026-09-11。対象: TASK-44/45実装（parse_sessionsのEXITED除外・list sessions/layoutsのsession解決回避・list panes human出力CWD列追加・shim FAKE_SESSIONS差し替え・test 5件追加・detailed-design/README更新）。作成者と別subagentによる独立レビュー。EXITED判定の中核はzellij v0.44.3 `zellij-utils/src/sessions.rs` の`print_sessions`実出力（suffixは空/`(current)`/`(EXITED - attach to resurrect)`の3値）と突合して**問題なし**判定。指摘5件（P1×1・P2×4）。IDはC-nの連番（C8〜C12）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| C8 | P1 | tests/ | `list layouts` の回帰テスト不在。修正で挙動が変わったlayouts経路（旧: session解決で失敗しうる → 新: 無条件で動く）が無ガード | **修正** | `list_layouts_works_without_session_resolution` 追加（live session 2つ + EXITED混在環境で`ZELLIJ_LAYOUT_DIR`のfile列挙がexit 0で動くことを検証。複数実行中は修正前にresolve_sessionがAmbiguousTargetで失敗する条件であり回帰検出力を持つ） |
| C9 | P2 | src/main.rs | Sessions/Layouts分岐がcheck_capability（DD-3.5起動時version gate）を回避し、未対応versionでのみ`list sessions`が動く非一貫挙動 | **修正** | Sessions分岐にcheck_capabilityを残す（zellijを呼ぶため）。Layoutsはzellijを呼ばないためgate対象外のまま。DD-3.5にgate適用範囲（zellijを呼ぶ出力系含む・layoutsは対象外）を明記 |
| C10 | P2 | detailed-design.md | 新設計文言「session不在でも表示できる」が過大表明。session完全不在時はzellij list-sessions自体がexit 1で失敗するため`zelper list sessions`も失敗する | **修正（文言）** | 「EXITED混在・複数実行中でも表示できる。sessionが全くない場合のlist sessionsはzellij list-sessions自体が失敗するためzelperも失敗する。layoutsはzellijを呼ばず無条件」に修正（旧挙動からのregressionなし） |
| C11 | P2 | src/zellij/parser.rs | `words.collect::<Vec<_>>().join(" ")`は毎行Vec+Stringを確保する。`words.any(|w| w.contains("EXITED"))`で意味同等かつ確保なし | **修正** | `words.any` 形式に簡素化（"EXITED"は空白を跨げないため意味同等） |
| C12 | P2 | README.md / FR-357 | EXITED除外によりdead session一覧をzelperで見る手段がなくなる（生`zellij list-sessions`が必要） | **将来提案として記録** | resurrection自体はrequirements対象外（§Out of scope）でありREADME既知の制限にも記載済みのため本変更は許容。`list sessions --all`系flag（dead session含む表示）を将来の拡張候補として本記録に残す。需要が生じた際に起票 |

**検証**: 修正後 `cargo test` 全件合格（125テスト）・`cargo clippy --all-targets` 警告0・`cargo fmt --check` 差分なし。C8の検出力実証: Layouts分岐を一時的に旧実装（mk_backend経由）へrevertすると `list_layouts_works_without_session_resolution` がFAIL、復元でPASSすることを確認済み。

### 4.10 TASK-46 コードレビュー（list系metadata拡張）

レビュー日: 2026-09-12。対象: TASK-46実装（SessionRef拡張 created/current/exited・parse_sessions全行parse化・resolve_session/list sessions側filter移動・list_tabs_for追加・sessions NAME/CREATED/CURRENT/PANES列・tabs PANES列・layouts SIZE/MODIFIED列・JSON契約拡張・usage配布物更新）。作成者と別subagentによる独立レビュー。parse_sessionsとzellij v0.44.3 `print_sessions` 実出力の突合・exited filter移動の消費者洗出し・run_action_asのargv構成（shim `$3` 判定への影響なし）は**問題なし**判定。指摘7件（P1×2・P2×5）。IDはC-n連番（C13〜C19）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| C13 | P1 | detailed-design.md | §3.4「EXITED行は除外し実行中のみを返す」がparse→呼び出し側への責務移動後も旧契約のまま。DD-5「読み取り1回」もsessionsは1+N回になり正本内で矛盾 | **修正** | §3.4を「EXITED行もflag付きで返し絞り込みは呼び出し側」に、DD-5に「sessionsはlist-sessions 1回+実行中session毎にlist-tabs発行・失敗時は該当行を`-`に落とす」を明記 |
| C14 | P1 | tests/ | 要件本体のテスト不足: (a)5tab超省略形式 (b)floating併記 `2+1f` (c)list-tabs失敗fallback (d)human_age境界 (e)JSON current:false | **修正** | (a)(b) `list_sessions_panes_summary_elides_beyond_four_tabs`（6tab fixture生成・FAKE_FIXTURES差し替え。PANES `7+7+6+2+...`・tabs列 `7+2f`）(c)(e) `list_sessions_fallback_when_tabs_fetch_fails`（shimにFAKE_FAIL_LIST_TABS失敗注入。human `-`・JSON null・current:false）(d) lib内 `#[cfg(test)]`（human_age粒度境界・未来時刻0s・summarize_panesのfloating除外と省略） |
| C15 | P2 | src/app/list.rs | PANES連結内の`2+1f`併記はtab境界が曖昧（`2+2+1f`の読みが2通り）。humanはfloating込み・JSONはtiledのみの非対称 | **修正** | session概要のPANES連結からfloatingを除外（tiledのみ。JSON panes_per_tabと対称）。floating併記はlist tabsのPANES列（単tabで曖昧性なし）に限定 |
| C16 | P2 | src/app/list.rs | layoutsのstat失敗・mtime不明時に(size 0, epoch 0)へ落ち「20500d 12h ago」等の誤表示 | **修正** | entriesをOption<u64>化。size/modified取得失敗時は `-`（JSONはnull）表示 |
| C17 | P2 | docs/usage/llm.md | layouts JSONも `["a","b"]`→`{name,size,modified_epoch}` 配列へのbreaking changeなのにsessionsのみ注記（運用不整合） | **修正** | llm.mdにlayouts JSON形状の行を追記 |
| C18 | P2 | src/app/list.rs | clippy `redundant_closure` 警告1件（`map(\|t\| pane_count_label(t))`） | **修正** | summarize_panesのtiled化に伴い閉包自体を解消（tiled countのto_stringへ） |
| C19 | P2 | src/app/list.rs | human_ageのdoc comment「zellijのCreated表記に合わせる」が不正確（zellijはhumantime `3days 5h`形式で書式が異なる） | **修正** | 「zellijのCreated表記と同系の相対表記」に修正 |

**検証**: 修正後 `cargo test` 全件合格（129テスト）・`cargo clippy --all-targets` 警告0・`cargo fmt --check` 差分なし。C14の(a)(c)はユーザー要求（7+7+6形式・省略）と設計明記のfallback挙動の受け入れ要件本体。

### 4.11 TASK-47 コードレビュー（list --compact縮小表示）

レビュー日: 2026-09-12。対象: TASK-47実装（`--compact`/`-c` 追加・panesはTAB+短縮cwdのみ・他resourceは名前のみ・`--json`併用時はJSON優先）。作成者と別subagentによる独立レビュー。short_pathの境界case手検証（HOME未設定/空・HOME=`/`・prefix類似dir・byte境界）・`--tab`組合せ・優先規則の実装/docs一致・sessions compact時のlist_tabs_for不呼出は**問題なし**判定。指摘5件（P1×1・P2×4）。IDはC-n連番（C20〜C24）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| C20 | P1 | src/app/list.rs | short_pathの境界case（prefix類似dirで置換されない・`p == home`・HOME未設定）に対する回帰test不在。`starts_with(home)` への簡略化がtestで検出できない | **修正** | `short_path_with(p, home)` 純粋関数へ分離しlib内test追加（肯定/prefix類似 `/home/tomox`・`/home/tomo2/x`/HOME外/空HOME/末尾スラッシュ正規化） |
| C21 | P2 | src/app/list.rs | HOME末尾スラッシュ（`/home/tomo/`）時に `p == home`・`starts_with("//")` とも不一致し短縮が無音で無効 | **修正** | `trim_end_matches('/')` による正規化を `short_path_with` 本体へ組み込み（C21のtest修正過程でtrim位置の誤りを検出し本体側へ移動） |
| C22 | P2 | src/app/list.rs | compact panesに非selectable plugin pane（tab bar等）が `Tab #1<TAB>-` として混入し、識別子を持たない紛らわしい行になる | **修正** | compact時は `p.is_selectable` でfilter（full human・JSONは従来どおり全pane表示） |
| C23 | P2 | docs/usage/snippet.md | `--json` 併用時のJSON優先規則がllm.md/SKILL.md/DD-4.1のみでsnippet.mdに未記載（配布物の粒度不整合） | **修正** | snippet.mdにcompact説明行（JSON優先・plugin除外）を追記 |
| C24 | P2 | tests/cli/list_read_send.rs | `list layouts --compact` 分岐がtest未カバー（docsは名前のみと記載） | **修正** | `list_layouts_compact_shows_names_only` 追加 |

**検証**: 修正後 `cargo test` 全件合格（133テスト）・`cargo clippy --all-targets` 警告0・`cargo fmt --check` 差分なし。C20のtest追加過程でC21相当の実装誤り（trimをenv版のみに置いた）をfail-firstで捕捉・本体へ修正した。

### 4.12 条件書移行設計レビュー

レビュー日: 2026-09-13。対象: docs/design/conditions-migration.md（テスト構造移行設計: conditions.toml全面導入・15条件書133条件・133 testへのcovers tag付与・verify script・5段階適用計画）。作成者と別subagentによる独立レビュー。指摘11件（P1×1・P2×3・P3×7）。IDはCMR-1〜CMR-11（§2のPhase 4 review DR-nと区別するため、当fileのreview毎接頭辞慣行に合わせた接頭辞。CMR = Conditions-Migration Review）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR-1 | P1 | 設計書§5.2 (f)・§9.2〜9.5 | 検査(f)（全`#[test]` fnのtag存在fail検出）が段階適用と矛盾: 第0段時点で全133 testが未tagのため(f)が必ずfailし、各段の完了判定（verify pass）が成立しない | **修正** | (f)を`--complete` flagでのみ有効化される検査とし、mise taskは第4段完了時に`--complete`付きへ切り替える設計へ変更。移行中の付け漏れ検出は`--list-untagged`出力とtmp/対照表の該当領域fn listのdiffで代替（§5.1/5.2/5.5・§9各段の完了判定へ反映） |
| CMR-2 | P2 | 設計書§3.3 | C番号不使用の根拠が事実誤り（C番号は節毎の番号空間ではなくdesign-review.md全体の連番C1〜C24） | **修正** | 根拠を「C番号はレビュー指摘logの追番であり要件のstable idではない（番号自体は条件内容を示さない）」へ書換え。結論（C番号不使用・source欄での節付き特定）は維持 |
| CMR-3 | P2 | 設計書§8.2・§6.1/6.2 | excludedのsource欄が参照するtest-plan §2.8/§2.11/§3が第4段のL4節集約で消滅し、参照が切れる | **修正** | §6.2対応表へ§2.8・§2.11・§3の行を追加して旧節番号をlocaterとして存続させ、§9.6へ「L4節集約時のexcluded source欄locater更新」作業を明記 |
| CMR-4 | P2 | 設計書§8.3 | 既知の未test要求が列挙されておらず、第2段列挙の入力が不明確 | **修正** | (a) readの--tail/--full加工（§4.2 MR-30でtail testはP1と明示）(b) §2.4 per-pane失敗exit 6・全失敗exit 5 (c) send部分失敗時の残対象継続 (d) §2.2 --cwd一致filter (e) §2.3 zellij error出力→error class変換 (f) §2.9 list-panes error伝播 (g) §2.1排他の一部（add tab 3source併用・resize equalize×grow/shrink併用）を§8.3へ列挙し第2段列挙の入力とした |
| CMR-5 | P3 | 設計書§3.4・§4.2 | 1 test fnが複数の出典要件を検証する場合の扱いが未規定 | **修正** | 「1条件に統合しsource欄へ全要件の出典を列挙する（test fnは分割しない）」と§3.4へ規定。§4.2の複数id並記は移行後の条件分割時に限ると明確化し、第1段パイロットの雛形確定項目へ追加 |
| CMR-6 | P3 | 設計書§2.2 | R番号範囲表記の過大包含（layout-planner行のR1〜R12にR8が、remap-cli行のR43〜R50にR47が含まれる。いずれもremap-sequence所属） | **修正** | layout-planner行を「R1〜R7・R9〜R12・R20」へ、remap-cli行を「R43〜R46・R48〜R50」へ明示列挙に置換 |
| CMR-7 | P3 | 設計書§5.1 | tomllibはPython 3.11+必須であることに言及なし | **修正** | script冒頭で`sys.version_info >= (3, 11)`を検査し未満ならfailする規定を§5.1へ追加 |
| CMR-8 | P3 | 設計書§4.4・§7 | scanner走査対象にcompanion plugin実装（plugin/src/）が含まれず、将来の付け漏れ検査の守備外になる | **修正** | 走査対象globへ`plugin/src/**/*.rs`を追加し、§7の範囲明示へpluginの扱い（test追加時はremap系条件書へ条件化）を追記 |
| CMR-9 | P3 | 設計書§5.3・§11 | source_lines同期が「書込み後にparse検証・失敗時破棄」の順で、破棄処理を要する | **修正** | 「置換後text構築 → in-memory parse検証 → 検証pass後に書込み」の順へ変更し、§11の破棄処理記述を簡素化 |
| CMR-10 | P3 | 設計書§5.2 (c) | schema検査にconditionのverified=false時のunverifiable_reason必須検査がない | **修正** | (c)へ「verified = false時はunverifiable_reason非空必須」を追加 |
| CMR-11 | P3 | docs/README.md | 設計書docs/design/conditions-migration.mdがdocs/README.mdのdesign/索引に未登録 | **修正** | orchestratorがdocs/README.mdへ登録（本レビュー対応とは別工程のため設計書側の変更なし） |

**検証**: 修正後、CMR-1〜CMR-10をdocs/design/conditions-migration.mdの該当節（§1.1/§2.2/§3.3/§3.4/§4.2/§4.4/§5.1〜5.5/§6.1/§6.2/§7/§8.3/§9.3/§9.6/§10.2/§11）へ反映済み。条件数整合（15file・133条件 = 133 test fn）とR番号対応（R1〜R50の過不足なし: planner R1〜R7/R9〜R12/R20・generator R9/R13〜R19・seed R21〜R28・sequence R8/R29〜R42/R47/R50・cli R43〜R46/R48〜R50）を確認。

### 4.13 TASK-54 第0段基盤整備レビュー（verify script）

レビュー日: 2026-09-13。対象: scripts/design/verify-conditions.py（TASK-54第0段で実装したverify script: 検査(a)〜(f)・source_lines自動同期・`--bootstrap`）。作成者と別subagentによる独立レビュー。レビュー方法: 設計書docs/design/conditions-migration.md §4/§5・test-structure skill §3/§4との仕様照合、repo実test file（tests/unit/selector.rs・src/app/list.rs等）とscanner仕様解釈の突合、read-only実行検証（repo本体のtoml 0件状態での起動、tmp/配下sandboxへのscript copy+fixtureによる同期・skip・fail検出の実証。repo file・tests/design/への書込みなし）。指摘11件（P1×0・P2×2・P3×9）。IDはCMR2-1〜CMR2-11（§4.12のCMR-nの第2陣として接頭辞CMR2 = Conditions-Migration Review 2nd）。修正8件・却下3件。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR2-1 | P2 | build_tag_refs | source_lines参照列のsortが文字列比較のため、同一file内の行番号が数値昇順にならない（sandbox実測: `foo.rs:142` が `foo.rs:3` より先に並ぶ）。§5.3(2)の「昇順」は位置順の意 | **修正** | sort keyを `(path, int(line))` 形式へ変更。tmp/偽repoで桁違いケース（`rs:9, rs:141`）の数値昇順を確認 |
| CMR2-2 | P2 | --bootstrap（render_bootstrap / run_bootstrap） | skill §4の「手動コメント保持・meta直前の手動コメントは保持」が未実装。tNN fileへの再生成で既存の手動コメントが消える（stable id付きfileのskipは実装済み） | **修正** | 本移行ではtmp/使い捨て1回限りの使用（設計書§5.5）で無害なため実装せず、script docstringへ非対応の明記（stable id付きfile・読み取れないfileはskip、tNN file再生成時は手動コメントを保持しない）で显在化 |
| CMR2-3 | P3 | scan_rust_file / build_tag_refs | `#[test]`を持たないhelper fnへのtagもcoverとして数えられる（tag→fn紐付けがis_testを参照しない）。§4.3「helper fnにtagを付けない」の誤配置が移行中は対照表頼みになる | **却下** | §4.3は執筆規則であり設計書§5.2の検査(a)〜(f)の範囲外。移行は§3.4の1:1原則によりtest fn直前にのみtagを付けるため発火しない。warn/fail化が必要になった場合は設計書の検査列挙への追加を経る |
| CMR2-4 | P3 | FN_RE / TEST_ATTR_RE | FN_REが`pub(crate) fn`等のvisibility修飾子、fn名と`(`/`<`の間の改行を受理せず、該当fnがentry化されない（tagは以降の別fnへ誤紐付け・検査(f)から無言で漏れる）。TEST_ATTR_REも`#[test] // comment`を落とす。現repoに該当patternなし（将来のtest追加時に無音化） | **修正（一部）** | FN_REへ `pub(?:\([a-z]+\))?` 受理を追加し、`[<(]` 要求を除去してfn署名途中改行を受理。TEST_ATTR_REは未拡張（現repoに該当なし・属性はrustfmt運用で単独行。該当書式採用時に要対応）。tmp/偽repoで `pub(crate)`・multiline署名の検出と同期を確認 |
| CMR2-5 | P3 | 同期の書込み | source_lines同期の書込みが非atomicで、書込み中断時にfile破損の余地 | **修正** | tmp file書込み + `os.replace` によるatomic書込みへ変更 |
| CMR2-6 | P3 | sync_text / sync_block | block終端・id行の検出が厳格すぎる: (1) 他のtable header (2) 行末comment付き`[[condition]]` header (3) single-quoteの `id = 'x'` の場合に、当該blockの同期がschema passのままsilent skipする | **修正（一部）** | header検出を前方一致（startswith）化して(2)へ対応、id行regexをsingle-quote受理へ拡張して(3)へ対応。(1)は新種table追加時にblock終端列挙の追記が必要（本移行の15条件書構成では発生しない）。tmp/偽repoで(2)(3)の同期を確認 |
| CMR2-7 | P3 | scan error出力 | scan errorの通知が `FAIL scan <file>: <詳細>` で、§5.4のfail行形式（`FAIL <検査> <file>: <id or fn>: <詳細>`）とsegment構成が異なる | **修正** | `FAIL scan <file>: -: <詳細>` 形式へ統一 |
| CMR2-8 | P3 | --out | `--out` がCWD相対で解決され、repo外からの起動で出力先が意図とずれる | **修正** | REPO_ROOT相対へ変更（絶対path指定はそのまま受理） |
| CMR2-9 | P3 | main / file_has_stable_id | `--bootstrap` 指定時に `--strict-source` / `--complete` / `--list-untagged` を無言無視する。`file_has_stable_id` がparse error fileを「stable無し」扱いし、手書き中の壊れたtomlを上書きし得る | **修正** | 検査optionとの併用をusage error（exit 2）化。parse error fileはstable扱い（skip）へ変更して上書きを防止 |
| CMR2-10 | P3 | check_design / check_excluded | schema検査がmeta fieldの存在のみで、`verified = true` 時のverified_at/verified_by/test_command/source_hash空欄を検査しない。decided_atが正規表現のみで `2026-13-99` 等が通る | **却下** | 設計書§5.2(c)(e)の検査列挙（meta必須fieldは「存在」・decided_atは「YYYY-MM-DD形式」）の範囲で実装済み。記録field空欄検査・日付妥当性（strptime）検査の追加は設計書の検査列挙改訂を経て行う |
| CMR2-11 | P3 | render_bootstrap | bootstrap生成tomlのsource_linesがtag行でなく `#[test]` 行をseedし、§4.4の定義（tag行の位置を記録）と一時的に不一致 | **却下** | 対照表の用途（§5.5: 生成entry数の確認・test fn一覧と§2.2/§2.3振分の突合）にfn位置seedが必要。tag付け後のverify実行で(d)同期がtag行位置へ自動置換するため、不一致は一時的 |

**検証**: 修正後、実repoでtoml 0件状態の `mise run verify-conditions` がexit 0（`OK: 0 files, 0 conditions, 0 tags, 0 excluded`）、`--list-untagged` 133件検出かつexit 0維持、`--complete` で133件fail・exit 1、`--bootstrap --out tmp/` 再生成が13file 133entry、`--bootstrap`×`--strict-source` 併用がexit 2を確認。tmp/偽repo（tmp/20260913_negative_task54）で正常系pass・同期の挿入・置換・数値sort（`rs:9, rs:141`）・冪等（再実行で0 files synced）・dangling tag/coverage fail時の無書込み・`pub(crate)` とfn署名途中改行の検出・行末comment付きheaderとsingle-quote idの同期を確認。FN_RE緩和によるentry数変化なし（133件維持）。

### 4.14 TASK-55 第1段パイロットレビュー（条件書selector・backend-parser・json-contract）

レビュー日: 2026-09-13。対象: TASK-55第1段で作成した条件書3file（tests/design/selector.toml・backend-parser.toml・json-contract.toml・20条件+excluded 3件）・covers tag 20件・meta検証記録。作成者と別subagentによる三者照合レビュー（出典（test-plan §2.2/§2.3/§2.10・detailed-design DD-1/2/3/4/10/11・design-review §4.2/§4.9）と条件の内容照合、条件書×test fn実assert×対照表の突合、verify・cargo test実行結果の確認）。指摘7件（P2×1・P3×6）。IDはCMR3-1〜CMR3-7（§4.12 CMR-n・§4.13 CMR2-nに続く第3陣。CMR3 = Conditions-Migration Review 3rd）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR3-1 | P2 | tests/design/selector.toml | filter option指定の結果0件→NoTarget（no pane matched。positional不在IDとは別code path）が条件化・excluded記録のいずれにもなく、§2.2「0件（NoTarget）」要求の対応決定漏れ | **修正** | [[excluded]]へ未test記録を追加（TASK-61のtest追加時に--cwd正例と併せ0件負例を条件化）。positional条件のdescriptionの「0件」をpositional不在ID文脈と明示し、source欄の節修飾も「存在確認・存在しないID」へ絞り、0件（NoTarget）をexcluded側のsourceへ分離 |
| CMR3-2 | P3 | tests/design/selector.toml（selector.visual-order-deterministic） | given「y/x座標とtab_positionが互いに同値でない」がfixture実態と不一致（terminal_3とfloating terminal_4が同一visual_key (1,0,0) を共有。等値pair間の順序は未検証） | **修正** | givenを実際のfixtureへ修正: visual_key一覧（(0,30,0)・(0,0,0)・(0,0,100)・(1,0,0)）を明示し、等値pair共有と等値pair間順序が未検証である旨を記載 |
| CMR3-3 | P3 | tests/design/selector.toml（selector.filter-name-exact-and-command-partial） | --name完全一致は正例のみのassertで、完全一致→部分一致への回帰（titleが部分文字列として他paneにhitする変質）を検出できない | **修正** | 検証限界をexpectへ明記（agent-bはagent-aを部分文字列に含まないため件数が変わらず検出不能である旨。文言はtest実態どおり） |
| CMR3-4 | P3 | tests/design/selector.toml（selector.tab-resolution-by-id-or-unique-name） | DD-1.3の曖昧時候補列出をtestがassertしていない（ErrorClassのみでcandidates中身は未assert） | **修正** | 検証限界をdescriptionへ注記（pane側resolve_singleでの候補assertはsingle-target条件が検証する旨と併記） |
| CMR3-5 | P3 | tests/design/selector.toml（selector.empty-tab-definition） | source欄の「requirements §2.8（空tabの定義）」が不正確。§2.8は空tab削除の要求を述べるもので、空tabの定義本体はDD-11 | **修正** | 「§2.8（空tab削除の要求）」へ修正し、DD-11側に「空tabの定義」であることを明記 |
| CMR3-6 | P3 | tests/design/backend-parser.toml（backend-parser.sessions-text-basic-form） | expect「各name・createdが行の値どおりで、current=false・exited=false」が検証実態を超えている（testはs[1]のcreated・flagsをassertしていない） | **修正** | expectを検証実態へ正確化: s[0]はname/created/current/exited、s[1]はnameまで（created・flagsは未assert） |
| CMR3-7 | P3 | tests/design/json-contract.toml（json-contract.cli-error-envelope-on-failure） | source欄のMR-17所在節が「§4」で不正確（MR-17は§4.2 実装コードレビュー第2ラウンド内） | **修正** | 「§4.2 MR-17」へ修正 |

**検証**: 修正後 `mise run verify-conditions`（--strict-source込み）pass（`OK: 3 files, 20 conditions, 20 tags, 4 excluded (0 files synced)`・excluded 3→4件・source_lines同期差分なし）。test codeは不変のためcargo testは未実施（第1段実装時に133件全PASS・clippy警告0・fmt差分なしを確認済み）。レビュー観点7（検証限界の様式化）の推奨を受け、docs/design/conditions-migration.md §2.4へ「expect/descriptionはtestが実際にassertする内容を超えて出典要求を主張しない。超える場合は検証限界を明記する」を第2〜3段様式規則として追加。

**TASK-61完了（2026-09-13）**: CMR3-1 dispositionで約定した未test項目の条件化を実施。selector.tomlのexcluded 2件を条件化（selector.filter-cwd-exact-match: --cwd完全一致正例+前方一致"/tm"負例、selector.filter-zero-matches-no-target: filter 0件→NoTargetをpositional不在IDのmessageと区別してassert）し、CMR3-3の--name完全一致検証限界を--name agent（部分文字列）負例の追加で解除（filter-name条件のexpectを書換え・検証限界記載を削除、positional条件descriptionのexcluded参照を条件id参照へ解消）。検証: `cargo test` 135件全PASS（selector 10件）・`mise run verify-conditions` pass（`OK: 15 files, 135 conditions, 135 tags, 31 excluded (0 files synced)`）・clippy警告0・fmt差分なし。

### 4.15 TASK-56 第2段中規模領域レビュー（条件書7file・covers tag 48件）

レビュー日: 2026-09-13。対象: TASK-56第2段で作成した条件書7file（tests/design/cli-grammar.toml・read-send.toml・list-display.toml・rename.toml・add-remove.toml・resize.toml・docs-verb.toml・48条件+excluded 14件）・covers tag 48件・meta検証記録・tests/design/README.md更新。作成者と別subagentによる三者照合レビュー（出典（test-plan §2.1/2.4/2.5/2.6/2.9・detailed-design DD-1/4〜9/11・design-review §4.2/§4.4/§4.9〜4.11・requirements §2.4/2.7/2.8）と条件の内容照合、条件書×test fn実assert×対照表の突合、verify・cargo test実行結果の確認）。指摘11件（P2×2・P3×9）。IDはCMR4-1〜CMR4-11（§4.12 CMR-n・§4.13 CMR2-n・§4.14 CMR3-nに続く第4陣。CMR4 = Conditions-Migration Review 4th）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR4-1 | P2 | tests/design/list-display.toml [meta] | source_hashがcargo fmtによる`#[test]`行indent修正前の値（e1e543da…）で、検証時点（fmt後）の実装と不一致 | **修正** | 実測値（git hash-object src/app/list.rs = a4bfc373…）へ更新 |
| CMR4-2 | P2 | tests/design/cli-grammar.toml | session解決の残り段（--session NAME直接指定・ZELLIJ_SESSION_NAME環境変数・複数実行中session時のerror+候補列出）が条件化・excluded記録のいずれにもなく、§2.1優先順要求の対応決定漏れ | **修正** | [[excluded]]へ未test記録を追加しTASK-65へ集約（goal書き換え）。session-auto-resolve条件の検証限界文言は単一実行中の正例のみである旨を維持 |
| CMR4-3 | P3 | tests/design/add-remove.toml（remove-empty-unknown-tab-name） | given「tab0=空でない・tab1=空でない」がfixture実態と不一致（FakeBackend::new(vec![], vec![tab(0,0), tab(1,3)])はtab0=空・tab1=空でない） | **修正** | 「tab0=空・tab1=空でない」へ修正 |
| CMR4-4 | P3 | tests/design/cli-grammar.toml + 設計書§2.3 | DD-1.6のverb例のうちL2 test担当分（rename/resize/add/remove）のCLI parseが対応決定漏れ。L2 testはapp層関数の直呼びでparseを経由せず、設計§2.3「当該verb testが検証する」前提に穴 | **修正** | [[excluded]]へ未test記録を追加しTASK-65へ集約。設計書§2.3へ「L2 testはCLI引数parseを経由しないためparse検証はL3 testを持つverbが担い、L2担当verbのparseは未testとしてcli-grammar.tomlの[[excluded]]へ記録する」注記を追加 |
| CMR4-5 | P3 | tests/design/cli-grammar.toml excluded（DD-1.6非例の残り） | reason「remap positional path渡しは3source排他と同一経路、重複optionはclap既定動作」が実装と不一致の見込み（排他はconflicts_with_allで併用時発火のため、単独positional path渡しはparseを通過しlayout名解決のLayoutNotFound系 exit 7になる見込み。--json重複の実際の挙動は未確認） | **修正** | reasonを実装に即した文へ書き換え。TASK-65 bodyへ「test化時に§2.1期待値（exit 2）と実装の突合が必要」を追記 |
| CMR4-6 | P3 | tests/design/rename.toml（pane-silent-noop） | source「requirements §4-5」が実在しない節参照（§4はScope prioritization・§5はExplicit non-goalsで内容非対応） | **修正** | 内容を確認のうえ「requirements §2.4（rename a pane/tab のfirst-class wrapper操作要求）」へ修正 |
| CMR4-7 | P3 | tests/design/resize.toml（noop-stops-early） | 検証限界にresize呼び出し回数（打ち切り自体の発生）が未assertである旨の記載漏れ | **修正** | 検証限界へ「resize呼び出し回数（打ち切り自体）も未assert」を追記 |
| CMR4-8 | P3 | tests/design/add-remove.toml（remove-pane-safety-gate） | given「2対象のdry-run」がtest実態と不一致（dry-runは--yes併用の単一対象指定で、DD-1.5の--dry-run×--yes併用経路） | **修正** | given/expectをtest実態（単一対象・--yes併用dry-run（DD-1.5併用経路）・--yesのみ）へ修正し、pane数遷移（3→2→2→1）を明記 |
| CMR4-9 | P3 | 各条件書 | 出典走査の残項目5点の対応決定漏れ: (1)§4.10 C16 layouts stat失敗/mtime不明の-表示・JSON null (2)DD-5失敗行（session不存在はOperationFailedで包む） (3)DD-1.5併用系正例（--tab×--all併合・--dry-run×--yes注意出力〔MR-30〕） (4)DD-7複数行textはwrite-chars単呼 (5)§2.1 completionのzsh/fish種別 | **修正** | excluded 5行を追加（(1)(2)はlist-display.toml・(3)(5)はcli-grammar.toml・(4)はread-send.toml）し、TASK-64へ(1)(2)・TASK-65へ(3)(5)・TASK-63へ(4)を集約（各goal書き換え） |
| CMR4-10 | P3 | tests/design/read-send.toml（human-output-pane-headers） | descriptionに簡体字「哪个」が混入（「どの」の誤記） | **修正** | 「どの」へ修正 |
| CMR4-11 | P3 | docs/design/conditions-migration.md §3.6 | 実例のsource欄「development-plan P1-2(d)」がdevelopment-plan.mdに実在しない参照（lib内test doc comment由来のローカル番号。作成者実装報告で指摘済み） | **修正** | 実際に採用した出典（detailed-design DD-4.1 + design-review §4.10 C14(d)）へ修正（id list-display.human-age-granularity-boundariesは不変） |

**検証**: 修正後 `mise run verify-conditions`（--strict-source込み）pass（`OK: 10 files, 68 conditions, 68 tags, 25 excluded (0 files synced)`・excluded 18→25件・source_lines同期差分なし）。test codeは不変のためcargo testは未実施（第2段実装時に133件全PASS・clippy警告0・fmt差分なしを確認済み）。

### 4.16 TASK-57 第3段remap系レビュー（条件書5file・covers tag 65件）

レビュー日: 2026-09-13。対象: TASK-57第3段で作成した条件書5file（tests/design/layout-planner.toml・layout-generator.toml・companion-seed.toml・remap-sequence.toml・remap-cli.toml・65条件+excluded 7件）・covers tag 65件・meta検証記録・tests/design/README.md更新（R1〜R50対応表の完成）。作成者と別subagentによる三者照合レビュー（出典（test-plan §2.7 (a)〜(j)・detailed-design DD-3/10/12・design-review §4.2/§4.6〜4.8）と条件の内容照合、条件書×test fn実assert×対照表の突合、verify・cargo test実行結果の確認）。指摘4件（すべてP3）。IDはCMR5-1〜CMR5-4（§4.12 CMR-n・§4.13 CMR2-n・§4.14 CMR3-n・§4.15 CMR4-nに続く第5陣。CMR5 = Conditions-Migration Review 5th）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR5-1 | P3 | tests/design/layout-planner.toml | R20の出典locater計3箇所（header・r20条件source・quoting-warning条件source）が「§2.7 (a) R20」だが、R20の定義はtest-plan §2.7 (b) 生成KDL節（planner側への振分自体は設計§2.2どおりだが節locaterが不正確） | **修正** | 3箇所とも「§2.7 (b) R20」へ修正（headerは「(a) R1〜R12・(b) R20」表記へ） |
| CMR5-2 | P3 | tests/design/remap-sequence.toml（r31-probe-timeout） | sourceの「design-review §4.12 worked example #8」が不正確な参照（worked example #8の定義箇所はdetailed-design DD-10.12であり§4.12は条件書移行設計レビュー節） | **修正** | 「detailed-design.md DD-10.12 worked example #8」へ切り分け修正（§4.6 R3参照は維持） |
| CMR5-3 | P3 | 3条件 | 検証限界の記載漏れ: (1) companion-seed r22-ensure条件: description「内容・mtime含め書き換えない」に対しtestは内容同一のみassert（mtime未assert） (2) remap-sequence r50-dry-run条件: dry-run側はrun Okとtoggle不発生のみで計画内容（M=4/k=2・floating込みsource）は未assert (3) remap-cli r48条件: source一覧は件数のみassert（visual order順序・現在tab付き表示は未assert） | **修正** | 3条件のdescriptionへ検証限界を追記 |
| CMR5-4 | P3 | tests/design/remap-sequence.toml | DD-10.11失敗分類のうち「companion setup失敗: 状態変更前（tiled化を除く）に即中断」が未testかつexcluded未記録（component単位の失敗はcompanion-seed側〔r25/r28/C4系〕で検証済みだがremap::run経由の実行sequenceが対応決定漏れ） | **修正** | [[excluded]]へ未test記録を追加しTASK-70へ併載（goalをlayout適用phase失敗とcompanion setup失敗の両方へ書き換え・body追記） |

**検証**: 修正後 `mise run verify-conditions`（--strict-source込み）pass（`OK: 15 files, 133 conditions, 133 tags, 33 excluded (0 files synced)`・excluded 32→33件・source_lines同期差分なし）。test codeは不変のためcargo testは未実施（第3段実装時に133件全PASS・clippy警告0・fmt差分なしを確認済み）。

### 4.17 TASK-58 第4段・移行全体最終包括レビュー（test-plan縮小・README索引化・全締め）

レビュー日: 2026-09-13。対象: TASK-58第4段の変更一式（docs/testing/test-plan.md §2対応表化・§3「L4統合test」節集約・§4/§5更新、tests/README.md索引化、tests/design/README.md最終化、mise.toml `--complete`切替、docs/README.md更新）および移行全体（第0〜4段）の正本移譲の妥当性。作成者と別subagentによる最終包括レビュー。検証方法: (1) 新旧test-plan全文比較（縮小前後で条件定義・L4要件・harness構成・実施記録S1〜S11・S-v2-1〜6の欠落なし・旧節番号locaterの存続確認） (2) 三方突合（test-plan §2対応表×tests/design/README.md条件書一覧・R番号→条件id対応表×各条件書source欄） (3) 全file突合（excluded 33件のsource欄locater・test file header commentの節参照が対応表経由で解決可能か。detailed-design・design-review・requirements-traceabilityのR番号参照の到達性）。指摘3件（すべてP3）。IDはCMR6-1〜CMR6-3（§4.12 CMR-n〜§4.16 CMR5-nに続く第6陣。CMR6 = Conditions-Migration Review 6th）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CMR6-1 | P3 | docs/testing/test-plan.md §5 | 「§2.7冒頭のとおり経過措置として第2段で置換する」がdangling reference（§2.7冒頭は対応表化で消滅）かつ時制が古い（置換完了済み） | **修正** | 「旧§2.7冒頭の経過措置どおりv2実装（TASK-39）で置換完了（旧仕様テストは削除済み）」の完結形へ修正 |
| CMR6-2 | P3 | tests/design/rename.toml | header commentの出典「requirements §4-5」が実在しない節参照（CMR4-6でsource欄は実在節「requirements §2.4」へ修正済みだがheaderに残存） | **修正** | header当該箇所を「requirements §2.4」へ修正（source欄と一致） |
| CMR6-3 | P3 | docs/testing/test-plan.md §2対応表 | remap-sequence行が「§2.7 (d)〜(f)(j)」のみで、R47・R50の実行側（L2: remap-sequence.toml）条件の所在が対応表から読めない（(i)はremap-cli行の表記のみ） | **修正** | 当該行を「§2.7 (d)〜(f)(j)・(i)実行側（R47・R50のL2側）」へ拡張（設計書§2.2の13行目出典表記と同一） |

**検証**: 修正後 `mise run verify-conditions`（--strict-source --complete込み・検査(f)含む）pass（`OK: 15 files, 133 conditions, 133 tags, 33 excluded`・条件数・tag数・excluded件数とも不変。条件書の変更はrename.tomlのheader commentのみで条件本体・source欄は不変）。test codeは不変のためcargo testは未実施（第4段実施時に133件全PASS・clippy警告0・fmt差分なしを確認済み: tmp/20260913_test_cargo_stage4_task58.log）。

**総評**: 移行全体として設計書§9.6の完了判定を満たす（判定1: cargo test 133件全PASS・判定2: verify-conditions pass〔--complete込み・--list-untagged空〕・判定3: 既存R番号参照からtests/design/README.md対応表経由でid・条件へ到達可能・判定4: 本最終包括レビューにて完了）。検証条件の正本はtest-plan §2から条件書15file（133条件・133tag・excluded 33件）へ完全移譲され、二重正本期間は終了。test-plan §2対応表・L4統合test節集約・README索引化・`--complete`常時化のいずれも旧正本の内容を欠落なく保持し、旧節番号locater経由の参照はすべて解決可能。conditions-migration移行（TASK-54〜58）は完了。

### 4.18 TASK-61完了レビュー（selector未test項目の条件化）

レビュー日: 2026-09-13。対象: TASK-61の変更一式（tests/unit/selector.rsのtest 8→10件・tests/design/selector.tomlのexcluded 2件条件化とfilter-name条件の検証限界解除・tests/design/README.md・tests/README.mdの件数更新・§4.14 TASK-61完了記録）。作成者と別subagentによる三者照合レビュー（条件書×test fn実assert×src/selector.rs実装、出典〔test-plan §2.2・DD-1.4〕との照合、検証log確認・残存参照grep・fmt再実行）。指摘2件（ともにP3）。IDはTR61-1〜TR61-2（§4.17 CMR6-nに続く。TR61 = TASK-61 Review）。全件**修正**で対応。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR61-1 | P3 | tests/design/selector.toml（selector.filter-zero-matches-no-target） | expectがmessage文言「'no pane matched the given targets'」を全文引用するが、testのassertは`contains("no pane matched")`の部分一致まで（§2.4様式規則上の軽微な主張超過。文言後半の変化は検出されない） | **修正** | expectを「messageが'no pane matched'を含む」へ正確化（test側を全文assertへ強化せずcontainsで足りると判断。code path区別の目的はcontainsで担保済み） |
| TR61-2 | P3 | docs/design/conditions-migration.md §2.2 | 一覧表（最終確定構成）のselector行が旧値8件のまま。TASK-62〜71のexcluded条件化が進むたび設計書表と実態の乖離が蓄積する運用になる | **修正** | §2.2へ「本表は移行完了時点の確定値。移行後の条件追加・excluded解消はtests/design/README.mdを正本として管理し本表は更新しない」の注記を追加（数値の二重管理を排除） |

**検証**: 修正後 `mise run verify-conditions` pass（`OK: 15 files, 135 conditions, 135 tags, 31 excluded (0 files synced)`）・`cargo test` 135件全PASS・clippy警告0・fmt差分なし。観点3（回帰検出力）は指摘なし: "/tm"は"/tmp"の真の前方一致・"agent"はagent-a/agent-b双方の部分文字列であり、完全一致比較が前方一致/部分一致へ変質すればtestがfailすることを確認。

**総評**: 機能的・事実的な問題なし。指摘は様式・運用留意点のみ。

### 4.19 TASK-59設計レビュー（zellij error出力→error class変換の条件化）

レビュー日: 2026-09-13。対象: TASK-59の条件書設計文案（tests/design/backend-process.toml新規作成案・backend-parser.tomlのexcluded削除とnote追記案・tests/design/README.md・tests/README.md更新案）。方法: 作成者と別subagentによる独立レビュー（実態突合せ: src/zellij/process.rs・src/zellij/mod.rsの実装、tests/design/*.toml既存条件・id実在、README各記載行の突合）。検証結果概要: 事実主張はほぼ一致。指摘5件（P2×2・P3×3）。IDはTR59-1〜TR59-5（§4.18 TR61-nに続く。TR59 = TASK-59 Review）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR59-1 | P2 | 設計文案excluded（timeout） | timeout超過（→OperationFailed）を除外した理由が事実誤認: private field timeoutへ短い値を直接設定したin-module testからsleepするfake scriptで決定的に引起できる | **修正** | timeoutを5条件目backend-process.timeout-operation-failedとして条件化（excludedから削除）。検証手段（timeout fieldへの短時間直接設定×sleepするfake script）をdescriptionへ明記 |
| TR59-2 | P2 | README更新案 | lib内test「3件」の言及がtests/design/README.mdに4箇所（:27・:49・:123・:138）あり、更新対象行が曖昧（:27はlist-display行の内訳10+3=13・:123は第2段歴史記録であり書換えると歴史記録を壊す） | **修正** | 更新対象を:49・:138へ限定し、:27・:123は不変として明示 |
| TR59-3 | P3 | 設計文案 | src/zellij/process.rsのerror分岐の列挙漏れ: :161-168 current_tabの"unexpected current-tab-info output"分岐・:249/:299 new_tab/override_layout内validate_exclusive失敗分岐が条件化・excluded記録のいずれにもない | **修正** | error return起点で全分岐を走査のうえ対応決定: (1) current_tab非単一tab出力は条件化（backend-process.current-tab-unexpected-output-operation-failed。fake script応答を2要素tab JSON配列へ固定し決定的に検証） (2) validate_exclusive失敗分岐はexcludedへ記録（covered by add-remove.add-tab-layout-sources-conflict。実在確認: tests/design/add-remove.toml:23。error生成はLayoutSpec::validate_exclusiveの単一実装でprocess.rs側は`?`伝播のみ） (3) parse系error伝播（parse_sessions/parse_tabs/parse_panes/parse_created_pane/parse_created_tabの各`?`）はexcludedへ記録（error生成はsrc/zellij/parser.rs側・backend-parser.toml管轄）。追加条件は1件で原則2件以内 |
| TR59-4 | P3 | 設計文案条件4（version-unparseable） | 本条件がparse_versionのNone変換に依存していることが条件書上から読めず、backend-parser側条件との依存関係が不明 | **対応** | 条件4のdescriptionへ「parse_version("not-a-version\n")→None（backend-parser.version-stringsがparse関数側を検証）に依存する」を1文追記 |
| TR59-5 | P3 | 設計文案 | 条件のsource欄が未指定 | **却下** | 設計全文案にはsource欄が記載済み。レビュー委任へ渡された要約版でsource欄が省略されていたことに起因する誤検出 |

**検証**: cargo test --lib 9 passed（新規6+既存3・回帰なし）/ cargo test全件 141 passed・0 failed / mise run verify-conditions exit 0（OK: 16 files, 141 conditions, 141 tags, 37 excluded） / cargo clippy --all-targets 0 warning / cargo fmt --check clean / source_hash aa1ba000f2c98b037968168f83a2785163e34ea0

**総評**: 指摘5件（P2×2・P3×3）は修正3（TR59-1 timeout条件化・TR59-2 対象行限定・TR59-3 列挙追加とcurrent-tab条件化）・対応1（TR59-4 依存明記）・却下1（TR59-5）で解消。条件6件・excluded 7件として確定し全verify pass。

#### code review（codex・read-only・2026-09-13〜14）

既存の設計レビュー結果および実装・条件書・進捗記録をread-onlyで突合し、TASK-59仕上げ時点のcode reviewを実施した。指摘5件（P1×1・P2×3・P3×1）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CR59-1 | P2 | tests/design/backend-process.toml 条件3 description | root環境では実行権限のない0644 fileのexecが許可され、testが失敗する懸念 | **修正** | 条件3 descriptionへLinuxの根拠を明記。kernelはCAP_DAC_OVERRIDEでもS_IXUGO必須のため、0644はrootもEACCESとなることを追記 |
| CR59-2 | P2 | tests/design/README.md :114/:128 | 進捗記録の旧数値（15file・133条件等）が現行と混同されうる | **修正** | :114/:128へ、記載値が移行完了時点の値である旨を明記 |
| CR59-3 | P2 | tests/design/backend-process.toml header/note・tests/design/README.md | toml header/note・READMEに作業途中記述（後段委任等）が残存 | **修正** | header・note・READMEを完了状態へ更新 |
| CR59-4 | P3 | src/zellij/process.rs test module内 setup_fake_zellij | helper temp pathはtag+pid固定・cleanupなしであり、tempfile::tempdir()の方がより堅牢 | **却下** | 既存L3 shim（tests/cli/remap.rs setup_fake_zellij）と同一方式を意図的に踏襲。cleanupはtemp dirのOS管理に委ねる運用 |
| CR59-5 | P1 | src/zellij/process.rs test module内 setup_fake_zellij・fake zellij test | 検証再実行時にtest 6件が約1/3のrunで散発失敗（failするtestは毎回変化）。class assertへmessage出力を追加して捕捉した実態はETXTBSY (os error 26): fake scriptのexecve時点で対象fileがwrite open状態になる競合。strace下37回非再現・/proc/self/fd採取でも恒常的write fd漏れなし | **修正** | setup_fake_zellijをstaging fileへwrite/chmod後にcloseしatomic renameで公開し、FAKE_ZELLIJ_LOCKによるtest直列化を実施。src実装本体は不変。cargo test --lib 50回loop 0失敗を確認。L3側helperの同一潜在競合は別taskとして起票済み |

**検証（最終）**: `cargo test --lib` 50回loop 0失敗（stability log: `tmp/20260913_test_task59_stability.log`） / `cargo test`全件 pass / `mise run verify-conditions` exit 0（16 files・141 conditions・141 tags・37 excluded） / clippy 0 warning / fmt clean。

**総評**: 設計レビュー5件＋code review 5件の計10指摘は、修正8・却下2で解消し、条件6件・excluded 7件・test 6件として確定した。

### 4.20 TASK-73設計レビュー（L3 fake zellij shimへのETXTBSY対策適用）

レビュー日: 2026-09-13。対象: TASK-73の設計（tests/cli/list_read_send.rs・tests/cli/remap.rsのsetup_fake_zellijへstaging+atomic rename公開・module直下FAKE_ZELLIJ_LOCK静的Mutex・全31 test fn冒頭guard取得。src/zellij/process.rs test module構成の踏襲）。方法: 作成者と別subagentによる独立レビュー（実態突合せ: 両test fileのhelper・test fn一覧とcall graph、src/zellij/process.rs:387-563の適用済みpattern、CR59-4/CR59-5記載、scripts/design/verify-conditions.pyのsource_lines自動sync機構、tests/README.md:14、Cargo.toml test target名、`cargo test -- --list`による総数確認）。検証結果概要: 設計の前提とされた事実はすべて一致（setup_fake_zellij直接callはzelper()内list_read_send.rs:53・zelper_with()内remap.rs:68の2箇所のみでtest fnからの直接callなし/ test fn 23+8=31/ 置換行番号 list_read_send.rs:46-47・remap.rs:51-52/ src側staging+renameはprocess.rs:400-405で設計snippetとcomment文言まで同一diff/ src側test 6 fn全て冒頭guard取得/ fake_backend・companion_seed.rsはexec対象fileをwriteせず対象外宣言どおり/ binary間pathはzelper-fake-・zelper-fake-remap-・zelper-fake-process- prefix+tag+pidで衝突なし/ 検証手順のtarget名cli_list_read_send・cli_remapはCargo.toml:49/57に実在/ 現test総数141件一致/ verify-conditionsは検査pass時にsource_linesを書き戻すため1回実行でsync差分が生成される）。技術的正当性も確認: rename公開によりshim pathのinodeはwrite fd close後にのみ公開され、再writeは常に新staged inodeへ向かうためwrite-open中inodeのexec/exec中inodeへのwrite-openの両経路を遮断する方向に働く。直列化の覆盖は同一binary内test並列のみで十分（cargo testはtest binaryを順次実行・binary間はpid付きpathで無関係）。指摘5件（P3×5・P1/P2なし）。IDはTR73-1〜TR73-5（§4.19 TR59-nに続く。TR73 = TASK-73 Review）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR73-1 | P3 | 設計前提（同一tagへの逐次再writeの列挙） | r46（tests/cli/remap.rs:172-183）のみ記載されているが、r43（remap.rs:103-128）もfor loop内でzelper("r43")を3回呼び同一tagへ逐次再writeする。いずれも同一thread直列（assert_cmdは子processの終了を待つ）のため固定staging名の安全性の結論は不変だが、前提集計として不完全 | **修正** | 設計前提へr43を1行追記（前提集計の完全化。実装内容・検証手順への影響なし） |
| TR73-2 | P3 | 設計変更内容(1) staticのdoc comment | 契約「test fn冒頭でのみ取得すること（wrapper・helper内では再取得しない）」に再取得禁止の理由がなく、理由（std::sync::Mutexは非reentrantで同一thread再取得は永久block）は設計本文(3)にのみある。将来の追記者が契約の意図を知らずguardをhelper側へ移動すると、failではなくtest hang（timeout）として発覚し診断が高価 | **修正** | doc commentへ「再取得は同一threadの再lockでdeadlockする」の1文追加を実装時に反映 |
| TR73-3 | P3 | 設計リスク対処（poisoning受容） | poisoning連鎖の受容はsrc同一挙動として妥当だが、src側6 fnに対しL3側は23/8 fnと規模が異なり、1件のpanicが同binary内残り全testのfailとして波及するノイズが大きい点の認識記録がない | **対応** | 受容判断は維持し、規模差（初回失敗特定時の連鎖ノイズ）の1文を設計記録へ追加 |
| TR73-4 | P3 | 設計推奨案の「src側test module構成の完全複製」表現 | src側static（process.rs:391）にはdoc commentがなく、L3側はdoc comment付きstaticを追加するため厳密には「複製+契約doc comment追加」である（改善付きで実害なし。主張の正確性のみの指摘） | **対応** | 表現を「同一構成+契約doc comment付き」へ修正 |
| TR73-5 | P3 | 設計リスク対処（guard付け忘れ担保） | guard付け忘れの検出がdoc comment契約+README 1行のみで機械検証がない。verify-conditions.pyは#[test] fn走査を行うため「zelper()/zelper_with()を呼ぶtest fnでのguard取得有無」checkへの拡張は構造的に可能 | **却下** | 現行は契約+READMEで担保し、散発失敗の再燃時に拡張を検討（本task範囲外とする設計判断を尊重） |

**検証**: 設計段階レビューのため実装検証なし（読み取り突合せのみ: `cargo test -- --list` 141件一致、grep突合せによる前提検証、src側pattern・verify script機構の直接確認）。

**総評**: 指摘5件はすべてP3（前提列挙・doc comment文言・記録・表現）で、P1/P2なし。ETXTBSY回避機構の技術的正当性・src側適用済み構成との一致・変更範囲の過不足（対象2 file+自動sync差分+README 1行で過不足なし）・検証手順（50回loop 0失敗の判断基準・失敗時triage・target名実在）はすべて確認済み。設計はこのまま実装に進めてよい。

#### code review（2026-09-13）

TASK-73実装（tests/cli/list_read_send.rs・tests/cli/remap.rs・tests/README.mdの3 file変更）に対するcode review。指摘1件（P3×1）。IDはTR73-6（TR73-1〜5に続く）。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR73-6 | P3 | 実装側doc comment（tests/cli/list_read_send.rs:8・tests/cli/remap.rs:9） | 「src/zellij/process.rs test moduleと同一構成」の表現がTR73-4（設計記録側は対応済み）と同じ厳密性問題: src側static（process.rs:391）にはdoc commentがなく、L3側は契約doc comment追加付きのため厳密には「同一構成+契約doc comment」である | **修正** | 両fileのcomment文言を「src/zellij/process.rs test moduleと同一構成+契約doc comment」へ変更済み（各1行・他は不変） |

その他のレビュー観点（31/31 test fn冒頭guard・wrapper/helper内でのlock取得なし・staging write→chmod→rename順序・src側diff（process.rs:400-405）とのcomment文言含む一致・既存test期待値・条件書の不変・tests/README.md実態一致・tests/内coverage完結）はいずれも指摘なし。

### 4.21 TASK-74設計レビュー（remap run一致照合修正・DD-10 v2.1）

レビュー日: 2026-09-15。対象: TASK-74設計（`docs/design/remap-v2-matching-fix-task74.md`・detailed-design.md DD-10 v2.1改訂〔10.1事実5精密化・10.7・10.8全面改訂・10.10・10.13〕。spike S成果物`tmp/task74/spike74/`・実験a/b/c前提）。方法: 作成者と別model（codex gpt-5.6-luna・read-only sandbox）による独立レビュー（レビューlog: repo管理外 `tmp/260915_review_design_task74.log`）。指摘7件（P1×2・P2×4・P3×1）・dispositionは全件修正・対応（却下なし）。IDはTR74-n。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR74-1 | P2 | 設計書§2.5 | PaneStateへ必須field追加で、tests/unit/selector.rs・tests/fake_backend/rename_add_remove.rs・tests/fake_backend/resize.rs等の既存PaneStateリテラルがcompile不能になるが§2.5に含まれていない | **修正** | `rg "PaneState \{"`で全literal箇所（tests/unit/layout_planner.rs:24・tests/unit/selector.rs:15,45,67・tests/fake_backend/remap.rs:28・rename_add_remove.rs:8・resize.rs:9・fake.rs:197,345,415,596・src/zellij/parser.rs:107）を§2.5へ列挙。PaneInfoのserde旧fixture互換（`terminal_command`欠損fixtureは`#[serde(default)]`でnull扱い・既存testは修正なしで継続）も明記 |
| TR74-2 | P1 | 設計書§2.2/§2.5/§4(ii) | templateの引渡し経路（doc → default_tab_template_subtree → plan/dry-run/execute → instance_kdls → generate_instance_kdl_v2）が不明確。現行signatureにtemplate引数が無く、src/app/remap.rsの呼出し更新が明示されていない | **修正** | §2.2へ関数signature単位の引渡し経路（`remap::run`で1回取得・`instance_kdls(plan, base_sub, template)`・`generate_instance_kdl_v2(base, runs, template)`・dry-run経路含む）と、template有/無それぞれの生成KDL例を追加 |
| TR74-3 | P1 | 設計書§1 F5/§2.1・DD-10.6(iii)/10.13 | 同一Run複数pane・run=None複数shell paneのslot順非保証は、requirements.md:100「pane ordering ... must be deterministic and documented」との未解決矛盾（TASK-37 R7の例外明記はDD-10側のみで要件側に未反映） | **修正** | requirements.md §2.6へR7先例形式で例外を明記（決定論保証範囲=tab/group所属・run一意paneのslot対応まで。同一run内・複数shell pane間のslot順はzellij run一致順依存で保証外。プロセス保存・pane数・所属tabは保証）。DD-10.6(iii)/10.13から要件例外への参照を追記し設計書§5対応表へ10.6行追加 |
| TR74-4 | P2 | 設計書§3 | r14/r15/r20等のdescription更新だけでは旧実装（pane_command依存）をfail-first検出できない。terminal_command=None+pane_command=Some（shell上で前景process実行）と terminal_command=Some+pane_command=None/別値 の直交ケースが必要 | **修正** | §3へ両field直交fixture（P-shell-proc / P-cmd-pane）の最小例と期待値・旧実装の挙動対比表を追加。planner（run assert）・parser（JSON key明記）・cli（shim fixture + dry-run KDL preview期待値）の3層配置を明記。generator層はSlotRun経由で既存条件が覆盖 |
| TR74-5 | P2 | 設計書§4(iii)(iv) | 副因C（空応答retry）のE2E検証が非決定的（「発生すれば記録・未発生ならfake test」）。(iii)の「観察項目」「合格必須からは除外しない」「spawnされない場合は記録」が相互矛盾 | **修正** | 副因Cの必須合格条件をfake backend test（remap-sequence.empty-list-panes-retry-in-polls・empty-response-retry-before-verify。N回空応答後成立の決定的注入・fail-first込み）へ昇格。§4を実機の必須合格条件・観察条件（j>=1 tab bar spawn有無・空応答発生時の非中断記録）・失敗条件に分離して再記述 |
| TR74-6 | P2 | 設計書§2.3/§5・DD-10.7 | §2.3がverify()冒頭・snapshot取得もlenient化する一方、DD-10.7本文はpolling前言と移動phaseのみで、初期snapshot・verifyのretry・list-tabs空応答の扱いが規範本文に未対応 | **修正** | DD-10.7へ「空応答のlenient取得」節を新設: 対象（polling closure〔probe・5-a/5-b〕・step 1 snapshot・10.9検証冒頭・`list_panes_lenient`/`list_tabs_lenient`）・deadline（10s不変・C3規則不変）・fatal条件（空応答のdeadline超過継続・非空parse失敗・非zero exit）を規範化し、§5対応表の10.7行を更新 |
| TR74-7 | P3 | 設計書§1 F4/§2.6 | F4のcwd比較はcwd=/tmpのみで、相対cwd・layout継承cwd・`zellij run --cwd`正規化差異は未検証のまま「cwd明示pane」一括扱い | **対応** | §1 F4に「検証済み=絶対path cwd明示のみ」の限定を追記。§2.6を検証状況で分解（絶対cwd明示=F4実証済み。相対・継承・正規化差異=未検証・同じ照合外れ経路と推定・追加実験は実施せず予算温存）し、preflight検出不可（terminal_commandからcwd観測不能・pane_cwdは代理にならない）を明記。§7の未判明事項を更新。DD-10.13・設計書§5対応表の10.1行にも反映 |

**検証**: 文書変更のみ（src/tests不変のためcargo等の機械検証対象なし）。修正後の整合確認: 設計書§2.2/§2.5/§2.6/§3/§4/§5/§7・detailed-design.md（DD-10.6(iii)/10.7/10.13）・requirements.md §2.6の相互参照一致。spike S成果物・実験予算（残り2回）は不変。

**総評**: 指摘7件は全件修正・対応で解消（却下なし）。P1の2件（TR74-2 template引渡し経路の実装指示欠落・TR74-3 要件側例外未反映）はいずれも設計書→実装・要件への接合漏れであり、文書修正で解消した。

### 4.22 TASK-74 コードレビュー（remap照合修正実装）

レビュー日: 2026-09-15。対象: TASK-74実装（remap照合key変更・template反映・poll lenient化・boolean出力修正。commit範囲86f0dbf..HEAD・src/tests 29 file→最終31 file程度。E2E経由でbar配置bug・boolean quote bugを発見修正済みの状態）。方法: 実装者（codex）と別model（glm・read-only）による独立レビュー。指摘8件（P1×1・P2×4・P3×3）・dispositionは全件修正・対応（却下・負債化なし）。IDはCR74-n。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CR74-1 | P1 | src/app/remap.rs execute step 6 | template系LayoutInvalid（children不在・leaf数不一致）検証が状態変更（probe・toggle・移動・rename）後にしか走らない。DD-10.7「全preflightを状態変更前に完了」・設計§2.2「preflight追加検証…事前中断」違反。partial_phase報告も`?`伝播で省略 | **修正** | instance_kdls計算をrun()の状態変更前（dry-run分岐と同形）へ切り上げ、executeへ渡す。step 6再計算を削除。回帰条件 remap-sequence.template-invalid-detected-before-mutation（状態変更操作0件・companion setup不発生込み）を新規条件化しfail-first確認後に修正 |
| CR74-2 | P2 | src/app/remap.rs probe poll | lenient呼び出しがgate用で実際のtitle判定はstrict list_panes再取得（tick毎2回spawn・gate直後strict空応答でfatalの窓・comment誤り） | **修正** | probe pollのtitle判定をlenient戻り値で直接行い1 tick 1回呼び出し化。fake strict側注入（busy window両掛かり模擬）に対してもpollはlenientのみで完結。C3 testの遅延注入をlenient経路へ追従 |
| CR74-3 | P2 | tests/fake_backend | 空応答注入カウンタがrun冒頭snapshot_lenientで消費され切り、probe/5-a/5-b poll・verify冒頭に空応答が届いておらず既存2条件の記載内容がtestで未実証（経路をstrictへ戻してもpass） | **修正** | fake注入機構をpipe以降arm（empty_panes_after_pipe）・override-layout以降arm（empty_*_after_override）へ再構成。旧empty_*_before廃止。既存2条件のdescription/givenを実証ある系列へ更新しfail-first確認後に対応 |
| CR74-4 | P2 | src/app/remap.rs timeout文面 | F8機構（screen繁忙の1s list応答timeout→空出力）注記がsnapshot_lenientのみでprobe/move両timeout文面にない（probe側は権限問題と誤診されうる） | **修正** | probe poll timeout・move系timeout（r36）文面へscreen繁忙注記を追加。poll-deadline系（C3）とr36のtest期待値へ文面assert追加 |
| CR74-5 | P2 | src/app/remap.rs・src/layout/generator.rs・tests comment | 廃止規則（pane_command基準注入）のstale doc/comment残存（V2Assignment.run・V2Plan.warnings・SlotRun・r20 test冒頭） | **修正** | すべてterminal_command基準の文言へ更新 |
| CR74-6 | P3 | src/layout/mod.rs normalize_zellij_kdl | 行単位字句処理のため複数行文字列（"""）内のbare true/falseを書き換えうる（旧実装からある既存制限。template配下plugin設定が流れるようになったため記録に値） | **対応** | 既知制限としてdocへ明記 |
| CR74-7 | P3 | src/layout/mod.rs default_tab_template_subtree | children blockを持たないnode（braceなし）がchildren()=Noneでtemplate無し扱いになり黙ってbar欠落（検証(b)はplugin paneを数えず検知不能） | **修正** | node在り・children block無しはSomeを返しむらのgenerator検証でLayoutInvalid発火。template-children-substitution条件へcase追加しfail-first確認後に対応 |
| CR74-8 | P3 | tests両README・meta note | fail-first期の記述（「期待値更新待ち」「現行実装はquote付きでfail-first」等）が現状と不一致 | **対応** | 当時記録である旨補正（layout-planner・backend-parserの同種記載含む） |

**検証**: 修正後 cargo test 149/149 green（追加・更新test含む。codex sandboxで見えたcli_list_read_send 4件failはhost非再現のsandbox artifact） / mise run verify-conditions exit 0（16 file・149条件・149tag・41excluded） / clippy 0 warning / fmt clean / podman E2E acceptance run5 全必須18条件PASS（tmp/task74/acceptance/・verdict.txt・run5正本。幾何はreferenceとIDENTICAL・bar top-level生存・27 pane/3 tab・anchor bar id保存）。

**総評**: P1×1（CR74-1 preflight順序違反）を含む指摘8件は全件修正・対応で解消（却下・負債化なし）。レビュアー総評「正常系の実装本体は設計・E2E実績と整合」どおり、CR74-1対応後にE2E再合格しTASK-74は完了可能となった。

### 4.23 TASK-75設計レビュー（remap multi-tab全体再現・DD-10 v2.2）

レビュー日: 2026-09-16。対象: TASK-75設計（`docs/design/remap-v3-multi-tab-task75.md`・detailed-design.md DD-10 v2.2改訂〔10.1事実6・10.2 #6/#8注記・10.5・10.6全面改訂・10.7・10.8・10.9・10.10・10.12 #9・10.13・10.14・DD-3.3〕・requirements.md §2.6 multi-tab要件。実験成果物`tmp/task75/`前提）。方法: 作成者と別model（codex gpt-5.6-luna・read-only sandbox）による独立レビュー（レビューlog: `tmp/20260916_review_design_task75.log`）。指摘10件（P1×1・P2×7・P3×2）・dispositionは全件**修正**（却下なし）。IDはTR75-n。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| TR75-1 | P1 | requirements.md §2.6・設計書§2.3/§2.8・DD-10.13 | requirementsは「tab名を`zellij --layout`起動時と同一に再現」「anchor以外に例外なし」と読める一方、設計/DD-10は名無しtab自動名（`Tab #N`）とpane name→titleを保証対象外としており、要件と詳細設計が矛盾する | **修正** | requirements.md §2.6へ設計/DD-10と同一の例外群を明記（(a) anchor名・位置 (b) 名無しtab自動名→zelper命名 (c) pane name→title保証外 (d) pane id）。DD-10.13 multi-tab対象外へ要件側明記済みの参照を追記 |
| TR75-2 | P2 | detailed-design.md:313 | v2.2改訂直後に「本v2は未実装・実装はTASK-39・切替はTASK-40」の旧記述が残存し、v2.1実装済みの現状と矛盾 | **修正** | 同箇所を「v2.1まで実装済み（E2E run5合格）・v2.2は未実装」の現状注記へ更新（旧v2時点の記録は完了済みである旨を明記） |
| TR75-3 | P2 | 設計書§2.7・DD-10.6・requirements.md:104 | 「T=1厳密一致」が式・命名・tab focusのみの一致か実行時挙動全体の一致か曖昧。T=1でもfocus-pane-idが新規実行されるため操作列全体の一致ではない | **修正** | 設計書§2.7を見出しから「T=1後方互換（割当・k・命名・tab focus復帰の式レベル一致）」へ改め、保証範囲=計画レベル4項目の一致・実行操作列全体の一致でないことを冒頭に明記。DD-10.6の当該bullet・requirements.md §2.6へも同旨の限定を追記 |
| TR75-4 | P2 | 設計書§3.4・DD-10.7 | 空groupのnew-tab由来tab IDをlayout適用phaseまで保持するdata flowが未定義。「長寿命tab id参照を持たない」原則（DD-2）と緊張 | **修正** | 設計書§3.4・DD-10.7へdata flowを追記: targets配列（targets[b*T+t]）の単一execute実行scope内保持は現行と同じ運用であり、DD-2原則の適用範囲を「実行を超えた保持の禁止」と明確化。step 6各group処理直前にlist-tabs（lenient）で存在確認し、不在なら再解決（割当あり=pane所属基準・空group=rename済み生成tab名基準。go-to-tab-nameは同名曖昧性のため補助）。new-tab由来idはparse直後のrenameで「取得直後に消費」 |
| TR75-5 | P2 | 設計書§3.4/§5・DD-10.7 | 空groupのみcase（M=0・source全てanchor内）のpreflight depthが不明。new-tab経路（tab ID parse・既定pane・override後N_t一致）が未検証のまま必須条件化されていない | **修正** | 設計書§3.4・DD-10.7へ「空groupのみ実行はcompanion・probe不使用・preflightは他caseと同一depth（layout解決・N_t=0・children置換・plan・生成KDL計算）を状態変更前に完了・new-tab失敗はOperationFailed」を追記。§5.1 remap-sequence条件へtab ID parse・既定paneのN_t正規化を含むfake必須条件として昇格、§5.2 (iii) E2E必須条件へ昇格 |
| TR75-6 | P2 | 設計書§5.2・DD-10.9 | E2Eのper-tab geometry条件が「anchor tabを除く各tabで検証」となり、anchor形状検証の除外と読める。anchor例外は名前・位置のみで形状・slot数の例外ではない | **修正** | 設計書§5.2 (ii) (iii)の幾何条件を「anchorを含む全生成tabでreference幾何と一致」へ修正。DD-10.9 (b)へ「anchor (0,0)を含む全生成tabが対象」を明記、DD-10.13 (4)にも形状・slot数は例外でない旨を追記 |
| TR75-7 | P2 | 設計書§5.2 | tab focus決定則のE2E必須条件が「文書順最初のtabにfocus」caseのみで、2番目以降のfocus tab・複数focus時の文書順最初採用・focus無指定時のanchor復帰が実機で固定されていない | **修正** | 設計書§5.2へ**(v) tab focus決定則matrix**を追加: 系列a（文書順2番目tabのfocus→最終go-to先が(0,t*!=0)）・系列b（複数focus=true→文書順最初採用+warning）・系列c（focus無指定→anchor復帰）の3系列を必須合格条件化（各系列ともactive tabをlist-tabsで検証） |
| TR75-8 | P2 | 設計書§4.3・DD-10.9 | dump比較除外項目が一部列挙のみで、§2.3/§2.8/DD-10.13の保証外項目との対応が体系的でない。dump比較実装時に誤差項目を合格判定へ混入しうる | **修正** | 設計書§4.3を対応表へ改訂: 比較対象を「tab毎slot数・幾何・bar存在・検証可能focus（is_focused・active）」に限定し、dump出現項目×除外根拠（保証外条文・R75事実）×代替検証経路の9行対応表化。DD-10.9へ「dump-layoutとの完全一致は保証・検証対象外・比較対象限定」の規定を追記 |
| TR75-9 | P3 | 設計書§3.8・src/app/remap.rs・src/layout/mod.rs・src/layout/generator.rs・tests/unit/* | 旧API（base_subtree・default_tab_template_subtree・template引数付きgenerate_instance_kdl_v2・単一instance V2Plan）削除時の全参照更新（remap.rs:66-125,203-265,577-595・mod.rs:264-304・generator.rs:26-49・tests/unit/*）と移行順が不足 | **修正** | 設計書§3.8へ旧API移行設計を追記: 旧API×現行参照箇所×移行先の対応表+実施順序5段（TabTemplate追加→本体切替え→旧API削除+残存0確認→test fixtureの正規形TabTemplate化→fake/cli期待値追従。各段でcargo check --all-targetsを先行） |
| TR75-10 | P3 | 設計書§3.5・src/zellij/mod.rs・process.rs | focus_paneのargv例がterminal_3限定で、PaneKindId系列分岐・focus対象がterminalであること・失敗時warning化の責務（backendかexecuteか）が不明確 | **修正** | 設計書§3.5へ責務分担を明記: backendはargv組み立てとResultを返す薄い実装のみ（系列分離はas_spec()が担い分岐なし。zellij仕様としてはplugin_Nも受理するがremapのfocus対象はterminal slotのみ）・Errのwarning化はexecute側の責務・fake.rsも同型の薄い実装+呼出記録 |

**検証**: 文書変更のみ（src/tests不変のためcargo等の機械検証対象なし）。修正後の整合確認: 設計書§2.7/§3.4/§3.5/§3.8/§4.3/§5.1/§5.2・detailed-design.md（DD-10.6/10.7/10.9/10.13・:313注記）・requirements.md §2.6（例外群・T=1後方互換・anchor例外の限定）の相互参照一致。実験成果物・予算（3回消化済み）は不変。E2E acceptanceは(v)追加により(i)〜(v)構成。

**総評**: 指摘10件は全件修正で解消（却下なし。P1×1は要件側例外未反映TR75-1）。レビュアー総評「中核設計（配分式・TabTemplate正規形・per-tab適用・anchor保持）は実装可能。P1解消とP2のTR75-4〜8反映後なら実装に進める」どおり、全指摘反映によりTASK-75設計は実装段階へ進める状態となった。

### 4.24 TASK-75 コードレビュー（remap multi-tab全体再現実装）

レビュー日: 2026-09-16。対象: TASK-75実装（正規形TabTemplate列・block×tab配分・focus位置ベース決定・空groupのnew-tab --layout-string経路・leftover_tabs報告・E2E要因A/B改修・vb warning改修込み。diff 61d0d84..HEAD・src/tests。E2E acceptance 80条件PASS・cargo test 171/171・verify-conditions 171条件済みの状態）。方法: 実装者と別model（codex gpt-5.6-luna・read-only sandbox）による独立レビュー（レビューlog: `tmp/20260916_review_code_task75.log`）。指摘3件（P2×1・P3×2）。IDはCR75-n。

| ID | severity | 該当 | 指摘 | disposition | 対応内容 |
|---|---|---|---|---|---|
| CR75-1 | P2 | src/app/remap.rs resolve_target | 再解決経路が`list_panes()`（strict・1回のみ）を呼ぶため、tab ID再解決時の一時的空stdoutで即時失敗する。DD-10.7/10.9の空応答設計（一時状態はpoll継続）と不整合（step 6本体はlenient使用なのに再解決だけstrict） | **修正** | resolve_target内の取得を`list_panes_lenient`のpoll（deadline規則は他lenient取得と同一・timeout文面にscreen繁忙注記）へ変更。tab-id-resolution-lenient-before-apply条件のtestへ「再解決位置のlist_panes空応答（精密arm empty_panes_nth_after_override）→retryで再解決成立（go-to 5直前のlist-panes-lenient連続2回）」系列を追加（strict 1回呼び出しならfailする期待値・弱め化なし） |
| CR75-2 | P3 | tests/design/remap-sequence.toml（--tab経路・layout適用phase失敗のexcluded） | `--tab TABSPEC`経路（source絞り込み・anchor切替・鋳型0単tab適用）とlayout適用phase失敗の部分状態報告が未testのまま。TASK-75のtargets再解決・multi-tab化との相互作用を固定できていない | **起票済み（TASK-69/70）** | 両経路とも条件書自身が未testと記録している既知項目であり、tmで管理済み（TASK-69: remap --tab TABSPEC絞り込みtest追加・TASK-70: layout適用phase失敗の報告test追加）。本レビューでは新しい対応を行わない |
| CR75-3 | P3 | src/app/remap.rs step 6（layout適用phase） | 各生成tabごとにlist-tabs poll＋list-panes pollを逐次実行するため、screen繁忙時に最大10sの待ち時間が生成tab数（T×k）に比例して累積する。phase内での取得結果共有・再利用方針が設計にない | **対応** | 設計書remap-v3-multi-tab-task75.mdへ§4.4（実行時間の性能特性）を新設: 現状構造（逐次poll・最悪待ち時間のT×k比例）・致命的でない理由（focus対象決定はbest-effortでpoll timeout時warning skip・検証(d)で検知）・将来の改善方向（phase内取得結果共有・deadlineのphase単位共有・存在確認の間引き。実用上問題になった時に検討）を明記。実装変更なし |

**検証**: CR75-1修正後 cargo test 171/171 green（tab-id-resolution-lenient-before-apply拡張込み） / mise run verify-conditions exit 0（171条件・171 tags・46 excluded） / clippy 0 warning / fmt clean。CR75-1はfake/L2で実証される変更（実機経路の挙動はlenient化で変わらずretry追加のみ）のためE2E再実行は不要。

**総評**: 指摘3件は修正1件（CR75-1）・起票済み1件（CR75-2→TASK-69/70）・設計記録1件（CR75-3）で解消（却下・負債化なし）。レビュアー総評「TASK-75の主要仕様はよく反映されており、重大な配分・focus・空group実装不一致は見当たらない」どおり、P2のlenient統一によりTASK-75実装は完了状態。
