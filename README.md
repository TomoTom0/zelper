# zelper

Zellij向けの構造化automation/orchestration CLI。複数pane/tab・外部script・coding agent・layout操作を、単一の一貫したcommand文法で扱います。

```text
zelper send --command codex -- y        # codexを走らせたpane全てにyを送る
zelper read --tab agents                # agents tabの全paneの画面を読む
zelper remap three                      # 動いているpaneをprocess保持のまま3分割layoutへ
```

## install

前提: zellij >= 0.44.1（`--layout-string`導入版。詳細は「互換性」章）

### GitHub Releases から（推奨）

Linux x86_64 用の静的binary（musl build。glibc versionを問わない）を配布。
[Releases page](https://github.com/TomoTom0/zelper/releases) に各版の資産がある。
archiveにはbinaryとともに `LICENSE-MIT` / `LICENSE-APACHE` が同梱される。
以下は同じdirectoryへ2file（archiveとchecksum）をdownloadして検証・展開する手順。
`releases/latest/download/` URLは常に最新版を指すためversion名の記載が不要になる:

```bash
curl -LO https://github.com/TomoTom0/zelper/releases/latest/download/zelper-x86_64-unknown-linux-musl.tar.gz
curl -LO https://github.com/TomoTom0/zelper/releases/latest/download/sha256sums.txt
sha256sum -c sha256sums.txt
tar xzf zelper-x86_64-unknown-linux-musl.tar.gz
chmod +x zelper
mv zelper ~/.local/bin/    # PATHの通ったdirへ
```

### mise

Linux x86_64向け（archiveが1つのみのためubiはplatform無判定で選択する。macOS/arm64では誤導入となる）

```bash
mise use -g ubi:TomoTom0/zelper
```

### 開発者向け

```bash
cargo install --git https://github.com/TomoTom0/zelper
# または clone して
cargo install --path .
cargo build --release   # target/release/zelper（buildのみ）
```

License: MIT OR Apache-2.0（`LICENSE-MIT` / `LICENSE-APACHE` 参照）

## 対象session

`--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ > error（候補表示）。

## 対象指定（全verb共通）

- positional = pane ID（`terminal_3` / `plugin_1` / bare `3`）。複数指定可
- filter option: `--tab`（ID or 一意な名前）/ `--name`（title完全一致）/ `--command`（部分一致）/ `--cwd` / `--all`
- positionalとfilterの和集合。単一対象を要求する操作で複数ヒットは `ambiguous target` error（候補列出）
- 並び順は常に決定的（tab position → y → x のvisual order）

## command

```text
zelper list sessions|tabs|panes|layouts [--tab T] [--json]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME
zelper rename tab TAB NAME
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T | --session-scope] [--overflow nest|tabs] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab TAB... [--empty] [--yes] [--dry-run] [--json]
zelper completion bash|zsh|fish
zelper docs readme | llm usage|skill|snippet
                                        （ドキュメント配布物の出力。docs/usage/参照）
```

## 出力とexit status

`--json` は `schema_version:1` / `ok` / `data`（または `error.class`）を返します。multi-target操作は `results[]` にper-target結果を格納し、部分失敗を隠しません。

```text
0 成功 / 2 usage / 3 対象解決失敗(no target|ambiguous) / 4 zellij不在|未サポートversion
5 操作失敗 / 6 部分失敗 / 7 preflight・layout不正・検証失敗
```

## remapの意味論

既存の動いているpaneのprocessを保持したまま、指定layoutに再配置します。

- 適用はtab単位。他tabは保護（`--apply-only-to-active-tab`相当を内部で常時使用）
- **pane数 M ≤ slot数 N**: 全pane保存。空slotは既定shellで埋まります
- **M > N**: 既定はerror。Zellijの制約により「保存したまま追加tabへはみ出す」ことはできません（実験確定）。明示選択:
  - `--overflow nest`: 全pane保存。overflow分は第1tab内に入れ子配置。**layout形状は保証されない**
  - `--overflow tabs`: layoutを追加tabで反復。overflow分のpaneはcloseされ、commandを再起動して再構成（そのpaneのみ破壊的）。dry-runで保存/再作成の別を事前確認できる
- floating paneはremapで失われるため、存在時はerror。`--embed-floating` でtiled化（process保持）してから組入れ（dry-runでは計画に含めるだけで切替は実行しない）
- 適用後にpane IDの生存（tabs modeでは再作成paneのcommand一致）を検証し、不一致はexit 7で報告
- atomicityは主張しない。途中失敗時は実行済み/失敗/未実行を報告

## 互換性

- 要件: zellij >= 0.44.1（`--layout-string`導入版）
- 検証: zellij 0.44.3（実機統合テスト S1〜S10: list/read/send/rename/add/remove/resize/remap含む全verb、プロセス保存はheartbeat pid継続で実証）。0.44.1単体の実機検証は未実施
- 利用する公開interface: `zellij action`（list-panes/list-tabs --json、override-layout、new-pane/new-tab等）、`zellij list-sessions`、`zellij setup`

## 既知の制限

- 既存paneを別tabへprocess保持で移す手段はZellijに存在しない。remapのoverflowも保存不可（上記）
- `resize`は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない（step刻みはzellij側仕様）
- `list sessions` のJSONはzellijが提供しないため、`list-sessions`テキストをparse（session名は正確、付帯情報は簡略）
- `--overflow tabs` の再構成commandは `pane_command` 文字列の空白分割のため、引用を含むcommandは崩れうる
- tab IDはclose後に再利用されるため、zelperは取得したtab IDを即時使用のみに用いる
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。zellij本体の解決（config.kdlの`layout_dir`）と一致させること

## docs

- 要件・設計・経緯: `docs/README.md` の索引を参照
  - 機能調査（実験根拠）: `docs/research/zellij-capabilities.md`
  - 詳細設計: `docs/design/detailed-design.md`
- coding agent向け配布物の索引: `docs/usage/README.md`（適用手順・形式比較: `docs/usage/distribution.md`）
- テスト: `tests/README.md`
