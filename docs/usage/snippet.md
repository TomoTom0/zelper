## zelper（Zellij操作CLI）

zellij sessionを操作するCLI。`zellij`（>= 0.44.1）がインストールされている前提。

- 対象session: `--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ
- 対象指定: positional pane ID（`terminal_3` / bare `3`、複数可）+ filter（`--tab` IDか一意な名前 / `--name` title完全一致 / `--command` 部分一致 / `--cwd` / `--all`）の和集合
- 構造把握は `zelper list panes --json` から。破壊的操作は `--dry-run --json` で事前確認

```text
zelper list sessions|tabs|panes|layouts [--tab T] [--json]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME | zelper rename tab TAB NAME
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T | --session-scope] [--overflow nest|tabs] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab TAB... [--empty] [--yes] [--dry-run] [--json]
```

誤用注意:

- `send` のtextは `--` 区切り必須: `zelper send --command codex -- y`
- `remap` は既存paneのprocessを保持してlayout再配置。pane数 > slot数は既定error。`--overflow tabs` はoverflow paneを再起動する（破壊的）
- exit status: 0 成功 / 2 usage / 3 対象解決失敗 / 4 zellij不在|未サポートversion / 5 操作失敗 / 6 部分失敗 / 7 preflight・layout不正・検証失敗
- `--json` は `schema_version:1` / `ok` / `data`（または `error.class`）。multi-targetは `results[]` に部分失敗も含む
