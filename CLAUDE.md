# zelper プロジェクトルール

## タスク管理

- 計画書（development plan・設計書等）に基づいて作業を開始する場合、計画把握の時点で判明している作業単位をすべてtmに起票（goal・order付き）してから着手する。やむを得ず後回しにする場合、「それをやる」という作業自体をタスクとして起票し、管理から外さない。「管理に入っていない後でやる」を許さない

## subagent運用

- バックグラウンドsubagentへ作業を委譲する前に、必要な権限を .claude/settings.local.json のallowlistと照合する。不足する場合は先にユーザーに権限追加を提案し、承認後に委譲する（deny量産の防止）
- 主要成果物（設計・実装・テスト結果）の完了時に、作成者と別のsubagentによる独立レビューを挟む。指摘は `docs/design/design-review.md` 等にID付きで記録し、修正/P1化/却下のdispositionを残す

## 実験・テスト環境

- ユーザー実環境のツール（zellij等）を用いる実験・テストは、podman等のsandboxで実行する。命名規則・運用規律のみの隔離で実環境に直接あたるテストはしない
- zellij実験のsandbox手順: podman + debian:12-slim + ホストのzellijバイナリをread-only mount、イメージ取得時のみnetwork（実行時は --network=none）、コンテナ内で `script -qec` によりPTY駆動、スクリプトとログは `tmp/` 配下に保存

## 参照

- 開発計画・要件・基本設計: `docs/design/first/260821/`（requirements.md / basic-design.md / development-plan.md。優先順位はこの順）
- ドキュメント構成は `docs/README.md`、テスト構成は `tests/README.md` で管理する（無ければ作成する）
