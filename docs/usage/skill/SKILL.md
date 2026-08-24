---
name: zelper
description: Zellij構造化操作CLI zelperの使い方。zellij sessionのpane/tab構造の読み取り、複数paneへの一括入力送信、layout再配置（remap）、rename、resize、pane/tabの追加削除を行うときに使用。zellijやtmux的な多pane作業環境の操作、coding agent稼働paneの管理、画面内容の取得、多pane一括コマンド送信に遭遇したら自律的にロードする。
---

# zelper - Zellij構造化操作CLI

zelperはzellij sessionをverb-first文法で操作するCLI。`zellij`バイナリ（>= 0.44.1）をbackendとして呼ぶ。

## 実行原則

- 対象session: `--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ > error（候補表示）
- 破壊的操作（`remove`、`remap --overflow tabs`）はまず `--dry-run --json` で計画を確認する
- 構造の把握は `zelper list panes --json` から始める（pane ID / title / command / 位置を一括取得）

## 対象指定（全verb共通）

- positional = pane ID（`terminal_3` / `plugin_1` / bare `3`。`3`は`terminal_3`と同義）。複数指定可
- filter option: `--tab`（tab ID or 一意な名前）/ `--name`（pane title完全一致）/ `--command`（pane command部分一致）/ `--cwd` / `--all`
- positionalとfilterの和集合が対象。単一対象を要求する操作で複数ヒットは `ambiguous target` error（候補列出）
- 並び順は常に決定的（tab position → y → x のvisual order）

## verb一覧

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
```

誤用しやすい点:

- `send` のtext指定は `--` 区切り必須: `zelper send --command codex -- y`（`-- y`を忘れるとusage error exit 2）
- `rename` はnoun形式: `zelper rename pane 3 build` / `zelper rename tab agents work`
- `resize` は方向まで指定: `zelper resize pane 3 grow right 2`
- `remap` のlayout指定は名前 / `--path` / `--inline` の3source相互排他。併用はusage error
- `--tab agents` のような名前指定は一意な名前のみ。同名tabが複数あればtab IDを使う

## 出力とexit status

`--json` は `schema_version:1` / `ok` / `data`（または `error.class`）を返す。multi-target操作は `results[]` にper-target結果を格納し、部分失敗を隠さない。

```text
0 成功 / 2 usage / 3 対象解決失敗(no target|ambiguous) / 4 zellij不在|未サポートversion
5 操作失敗 / 6 部分失敗 / 7 preflight・layout不正・検証失敗
```

## remapの意味論（誤用注意）

既存の動いているpaneのprocessを保持したまま、指定layoutに再配置する。適用はtab単位（他tabは保護）。

- **pane数 M <= slot数 N**: 全pane保存。空slotは既定shellで埋まる
- **M > N**: 既定はerror。overflowは明示選択:
  - `--overflow nest`: 全pane保存。overflow分は第1tab内に入れ子配置。layout形状は保証されない
  - `--overflow tabs`: layoutを追加tabで反復。overflow分のpaneはcloseされcommandを再起動（そのpaneのみ破壊的）
- floating paneが存在するとerror。`--embed-floating` でtiled化（process保持）してから組入れ
- atomicityは主張しない。途中失敗時は実行済み/失敗/未実行を報告

## 既知の制限

- 既存paneを別tabへprocess保持で移す手段はZellijに存在しない。remapのoverflowも保存不可
- `resize` は反復と幾何検証による近似。正確な行/列数・完全均等は保証しない
- `list sessions` のJSONはzellijが提供しないためテキストparse（session名は正確、付帯情報は簡略。`--json`でも付帯情報を過信しない）
- `--overflow tabs` の再構成commandは `pane_command` 文字列の空白分割のため、引用を含むcommandは崩れうる
- tab IDはclose後に再利用されるため、取得したtab IDは即時使用のみに用いる
- layout名解決は `ZELLIJ_LAYOUT_DIR` > `~/.config/zellij/layouts`。zellij本体の解決（config.kdlの`layout_dir`）と一致させること

## 典型ユースケース

```text
zelper list panes --json                      # 構造把握（最初の一歩）
zelper read --tab agents                      # agents tabの全pane画面を読む
zelper send --command codex -- y              # codex稼働pane全てにyを送る
zelper remap three --dry-run --json           # 3分割layout再配置の計画確認
zelper add pane --name build -- cargo build   # build用paneを追加してコマンド実行
zelper remove pane 12                         # pane削除（2対象以上で--yes確認要求）
```
