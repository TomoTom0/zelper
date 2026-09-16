# zelper

Zellij向けの構造化automation/orchestration CLI。複数pane/tab・外部script・coding agent・layout操作を、単一の一貫したcommand文法で扱います。

```text
zelper send --command codex -- y        # codexを走らせたpane全てにyを送る
zelper read --tab agents                # agents tabの全paneの画面を読む
zelper remap three                      # 動いているpaneをprocess保持のまま3分割layoutへ
```

## install

前提: zellij >= 0.44.3（詳細は「互換性」章）

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
mkdir -p ~/.local/bin   # 多くのdistroで、存在すればlogin時にPATHへ含まれる
mv zelper ~/.local/bin/
```

### mise

Linux x86_64向け（archiveが1つのみのためplatform判定なしで選択される。macOS/arm64では誤導入となる）

```bash
mise use -g github:TomoTom0/zelper
```

miseはsupply-chain保護（`minimum_release_age`、default 24h）により公開24h以内のreleaseをlatest解決から隠す。公開直後のversionをinstallする場合は`@0.1.0`のようにversionを固定する。

### 開発者向け

```bash
rustup target add wasm32-wasip1   # companion plugin wasmのbuildに必要
cargo install --git https://github.com/TomoTom0/zelper
# または clone して
cargo install --path .
mise run install        # 同等（repoのmise.tomlに定義済み）
cargo build --release   # target/release/zelper（buildのみ）
```

build時にcompanion plugin（wasm）を自動buildしてbinaryへ埋め込むため、install・配布形態は従来どおり単一binary（追加assetなし）。prebuilt wasmを使う場合は `ZELPER_PLUGIN_WASM` envにpathを指定する（CI等の高速化・escape hatch）。

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
zelper remap LAYOUT [--tab T] [--embed-floating] [--dry-run] [--json]
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

既存の動いているpaneのprocessを**すべて**保持したまま、指定layoutに再配置します。対象はsession全体のselectable・tiledなterminal pane（`--tab T` 指定時はそのtabのpaneに絞り込み、かつそのtabが再配置のanchorになる）。

- pane数 M・slot数 N から **k = max(1, ceil(M/N))** 個のlayout instanceを生成し、layoutを反復して全paneを配置します。**M > N でもerrorになりません**
- **kill/restartする経路は存在しません**。他tabへのpane移動はcompanion plugin（zelper binaryに埋め込んだwasm）によるプロセス保存移動で行います
- instance 0は既存tab（`--tab`指定時はそのtab、省略時はactive tab。tab名は保持）。instance 1以降は新規tabに `<layout名>-2`、`<layout名>-3` …と連番で命名（`--path`はfile stem、`--inline`は `remap-2` 等がbase）
- 割当順はvisual order（tab position → y → x）で決定的。空slot（最終instanceにのみ発生）は既定shellで埋まります。移動によって空になったtabはzellijにより自動closeされます
- 移動が必要な場合（k > 1、または対象paneの一部がanchor tab外）のみcompanion pluginを使用し、状態変更前に応答性確認（probe）を行います。probe不成立なら一切の状態変更を行わず中断（exit 5）。移動が不要ならcompanion pluginは一切使われません
- floating paneはremapで移動できないため、存在時はpreflight error。`--embed-floating` でtiled化（process保持）してから組入れ
- 適用後にpane IDの生存・割当tab所属・各tabのpane数を検証し、不一致はexit 7で報告
- atomicityは主張しない。途中失敗時は実行済み/失敗/未実行を報告

## 互換性

- 要件: zellij >= 0.44.3（companion plugin駆動のため）。0.44.x系列を超えるversion（0.45.0等）は未実証としてexit 4で拒否します
- 検証: zellij 0.44.3 + zellij-tile 0.44.3（実機統合テスト S-v2-1〜6: 9 pane → 3 instanceの全pid保存、dry-runの状態不変、移動不要時のplugin不使用、floating paneのpreflight errorと `--embed-floating` 組入れ、probe不成立時の状態変更前中断を実証）
- companion plugin: remapで移動が必要なときのみ動作。binary埋込のwasmを `$XDG_CACHE_HOME/zelper/companion/<version>/` に展開し、`$XDG_CACHE_HOME/zellij/permissions.kdl` に権限をseed（不足分のみ追記）して権限dialogを自動化します。installは単一binaryのままで追加assetなし
- 利用する公開interface: `zellij action`（list-panes/list-tabs --json、override-layout、pipe --plugin、new-pane/new-tab等）、`zellij list-sessions`、`zellij setup`

## 既知の制限

- remapのslot割当は、group（instance/tab）所属とcommand+argsが一意なpaneのslot対応に限り決定的。同一commandの複数pane間・複数shell pane間のslot順は保証しません
- `pane_command` に `"` `'` `\` 改行・制御文字を含むpaneは、空白分割でquotingを復元できずslot照合を外しうる（warningを出して実行し、適用後検証で検出）
- `resize`は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない（step刻みはzellij側仕様）
- `list sessions` のJSONはzellijが提供しないため、`list-sessions`テキストをparse（name/created/current。作成時刻はzellijの相対表記のまま）。EXITED（resurrection待ちのdead session）は除外して表示・session解決する
- tab IDはclose後に再利用されるため、zelperは取得したtab IDを即時使用のみに用いる
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。zellij本体の解決（config.kdlの`layout_dir`）と一致させること

## docs

- 要件・設計・経緯: `docs/README.md` の索引を参照
  - 機能調査（実験根拠）: `docs/research/zellij-capabilities.md`
  - 詳細設計: `docs/design/detailed-design.md`
- coding agent向け配布物の索引: `docs/usage/README.md`（適用手順・形式比較: `docs/usage/distribution.md`）
- テスト: `tests/README.md`
