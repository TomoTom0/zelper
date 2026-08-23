# zelper docs 索引

ドキュメント構成の管理対象はこのREADME。実態と常に一致させる。

## 構成

- `design/` — 設計
  - `first/260821/` — 初回ハンドオフパッケージ（requirements.md / basic-design.md / development-plan.md / README.md）
  - `requirements-traceability.md` — Phase 2/8。要件分類とP0→DD章マップ、制約と設計指示
  - `detailed-design.md` — Phase 3。DD-1〜DD-12（CLI grammar / domain / backend / output / 操作設計 / remap・resize algorithm / safety）
  - `design-review.md` — Phase 4。DR-1〜DR-10の指摘とdisposition
- `research/` — 調査
  - `zellij-capabilities.md` — Phase 1成果物。zellij 0.44.3実機検証済み機能マトリクス、override-layout詳細、overflow実験結果、remap設計への帰結
- `testing/` — テスト設計
  - `test-plan.md` — Phase 5。テスト階層（unit / fake-backend / CLI契約 / podman統合）、remap計画行列R1〜R10、failure injection、JSON契約、preservation実証手法

## 規則

- docs/直下へのファイル配置は禁止。カテゴリディレクトリ配下に置く。新カテゴリはディレクトリ作成とこのREADMEの更新をセットで行う
- 命名: 恒久文書は`<kind>_<topic>.md`（timestampなし）、蓄積型は`<date>_<kind>_<topic>_<rel>.md`
- development-plan.mdが成果物パスを明示的に規定する場合（`design/detailed-design.md`等）はそのパスを優先する
- 一時ファイル（検証スクリプト・実験ログ等）はdocs/ではなく`tmp/`へ。Phase 1の実験記録は`tmp/phase1/`を参照
