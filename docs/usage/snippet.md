## zelper（Zellij操作CLI）

zellij sessionを操作するCLI。`zellij`（>= 0.44.3）がインストールされている前提。

- 対象session: `--session NAME` > 環境変数 `ZELLIJ_SESSION_NAME` > 実行中sessionが1つならそれ
- 対象指定: positional pane ID（`terminal_3` / bare `3`、複数可）+ filter（`--tab` IDか一意な名前 / `--name` title完全一致 / `--command` 部分一致 / `--cwd` / `--all`）の和集合
- 構造把握は `zelper list panes --json` から。破壊的操作は `--dry-run --json` で事前確認
- `list -c`（compact）はhuman表示の縮小（panesはtab+短縮cwdのみ・非selectable plugin paneを除外、他は名前のみ）。`--json` 併用時はJSON優先

```text
zelper list sessions|tabs|panes|layouts [--tab T] [--json] [-c|--compact]
zelper read [PANE...] [filters] [--full] [--tail N] [--json]
zelper send (PANE... | filters) ( -- TEXT | --keys KEY... ) [--enter] [--json]
zelper rename pane PANE NAME | zelper rename tab TAB NAME | zelper rename session NAME NEW_NAME
zelper resize pane PANE (grow|shrink) (left|right|up|down) [STEPS]
zelper resize equalize [--tab T | PANE...] [--json]
zelper remap LAYOUT [--tab T] [--embed-floating] [--dry-run] [--json]
zelper remap --path FILE | --inline KDL   （layout 3sourceは相互排他）
zelper add pane [--tab T] [--count N] [--name NAME] [--cwd DIR] [-- CMD...]
zelper add tab  [--count N] [--name NAME] [--cwd DIR] [--layout NAME | --path F | --inline KDL] [-- CMD...]
zelper add session NAME [--layout NAME|PATH] [--cwd DIR] [--json]
zelper remove pane PANE... [--yes] [--dry-run] [--json]
zelper remove tab TAB... [--empty] [--yes] [--dry-run] [--json]
zelper remove session NAME... [--yes] [--dry-run] [--json]      （session完全削除。selector 4者排他:
zelper remove session (--all | --dead | --me) [--yes] [--dry-run] [--json]
                                                                 NAME... / --all / --dead / --me。
                                                                 常に--yes/--dry-run必須〔単一でも〕。
                                                                 --meは自session削除でbest-effort）
zelper move pane PANE (left|right|up|down|rotate) [--backwards] [--json]
zelper move pane PANE --to-tab TAB | --swap-with PANE   （3形式は相互排他）
zelper move tab TAB --swap-with TAB [--json]
zelper [--session NAME] attach [--json]      （対話attach。TTY必須）
zelper [--session NAME] detach [--json]
zelper watch [PANE...] [filters] [--regex] [--full] [--interval SECS] [--max N] [--json] -- PATTERN
```

誤用注意:

- `send` のtextは `--` 区切り必須: `zelper send --command codex -- y`
- `move` は単一pane/tabの移動・交換（全体再配置はremap）: `zelper move pane 3 left` / `zelper move pane 3 --to-tab 1` / `zelper move pane 3 --swap-with 5`（cross-tabのみ）/ `zelper move tab 1 --swap-with 3`。direction・--to-tab・--swap-withは三者排他かつちょうど1つ。pane/tab IDは不変
- `watch` のPATTERNも `--` 区切り必須（1件・optionは `--` の前）: `zelper watch --command codex -- READY`（polling: 既定1s間隔・最大30回。hitでexit 0）
- session lifecycle: `zelper add session NAME` はdetached sessionの非対話作成（session 0件環境でも動作）。`attach` は実TTY前提（非TTYはexit 7）。`detach` はclient不在でもno-op成功（複数client中のdetachは最新clientのみ切断）
- `remap` は既存paneのprocessをすべて保持してlayoutへ再配置（既存paneのkill/restartなし。照合不成立でzelperがspawnした複製paneのみcloseし報告。pane数 > slot数でもlayout反復で全paneを配置）
- exit status: 0 成功 / 2 usage / 3 対象解決失敗 / 4 zellij不在|未サポートversion / 5 操作失敗 / 6 部分失敗 / 7 preflight・layout不正・検証失敗
- `--json` は `schema_version:1` / `ok` / `data`（または `error.class`）。multi-targetは `results[]` に部分失敗も含む。`watch` のhit時は例外でhit pane単一のobject（`target` / `matched_lines` 等。障害paneは `pane_errors`）
