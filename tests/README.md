# zelper tests 索引

テスト構成の管理対象はこのREADME。詳細仕様は docs/testing/test-plan.md。

## 構成（実ファイル一覧）

- `unit/` — L1 純粋ロジック
  - `selector.rs` — 対象解決（positional/filter/和集合/曖昧/visual order/空tab定義）
  - `parser.rs` — zellij出力parse（version/sessions/作成ID/panes・tabs JSON実形式fixture）
  - `error_map.rs` — error.class ↔ exit status対応表・JSON envelope契約
- `cli/` — L3 CLI契約（assert_cmd + fake zellij shim）
  - `list_read_send.rs` — list/read/send契約、排他規則、exit code、補完生成、JSON error envelope。shimはtest実行時に生成（argv記録・fixture応答）
- `fake_backend/` — L2 決定的fake backend
  - `fake.rs` — FakeBackend（状態持ち・呼び出し記録・失敗注入・frozen no-op）
  - `rename_add_remove.rs` — rename検証・add作成検証・remove安全gate/`--empty`
  - `resize.rs` — step実行・no-op打ち切り・equalize収束・振動終了・不在ID/異tab混在error
  - `remap.rs` — planner matrix R1〜R10・KDL slot数/instance生成・実行順序・R7途中失敗・R8 floating・R6 session-scope・レビュー回帰（bare bool KDL正規化・multi-tab layout N=先頭tab・dry-run --tab非切替・instance tab rename・部分適用報告・PR#1: plugin leaf slot非消費・再作成command検証失敗化・floating変更のpreflight後延期・shellのみpaneのcwd保持・検証失敗のJSON envelope単一化・子なしtab/layout nodeのslot非消費・MR-32: plugin config子nodeのslot非消費）
- `fixtures/zellij/` — 実zellij 0.44.3出力fixture（panes.json / tabs.json）
- `fake_backend/fake.rs` 内のshim生成は `cli/list_read_send.rs`（L3）

## L4統合テスト（実zellij）

- runner/harness: `tmp/phase7/`（エフェメラル。podman + debian:12-slim + zellij 0.44.3 + zelper releaseバイナリ（musl static build）。common.sh + s1〜s11スクリプトで再現可能）
- 実行記録: `tmp/phase7/integration-report.md` — **S1〜S11全PASS**（修正後再実行含む。remap保存はheartbeat pid継続+pane ID同一で実証、overflow tabs modeの再作成paneのcommand一致も検証、plugin config子nodeのslot除外はbare/wrapper両形式で検証）
- 既知の検証限界: zellij 0.44.3では空tabを作成できない（new-tabが必ずpaneを作る）ため、`remove tab --empty`の実削除は代替検証（dry-run計画・非空tab保護・error契約）のみ
- 前提: config.kdl配置（Setup Wizard抑止）・表示200x60・heartbeatプロセス保存証明

## 実行

```bash
cargo test          # L1〜L3（78テスト）
cargo clippy --all-targets && cargo fmt --check   # lint
# L4はtmp/phase7/harnessをpodmanで実行（Phase 7記録を参照）
# 注: zellij session内で実行してもテストは隔離済み（ZELLIJ_SESSION_NAMEを除去）
```

## 規則

- 実装前fail-first対象はtest-plan §5（selector・remap planner・parser・error対応）
- fixtureは実機出力からの抽出物とし、改変時は出典をコメントで明記
- テスト追加時はこのREADMEの該当行を更新する
