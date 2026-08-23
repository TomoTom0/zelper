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

**検証**: 修正後 L1〜L3テスト71件合格・clippy警告0・統合S5〜S10再実行でFAIL 0件（tmp/phase7/integration-report.md 第2ラウンド記録）。

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
