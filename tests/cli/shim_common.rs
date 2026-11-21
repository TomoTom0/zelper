// L3 fake zellij shimの共通正本（TASK-83 Phase 2。tests/cli/list_read_send.rsと
// tests/cli/remap.rsが`mod shim_common;`で共有。fake.rsと同じinclude patternで
// Cargo.tomlの[[test]]登録は不要——subdirectory fileはauto-discover対象外のため。
// test支援資産として条件書tag対象外〔fake.rsと同じ扱い〕）
//
// # 機構分類と命名規則（正本。新規機構追加時は本表へ反映すること）
//
// L3はassert_cmd→zeller実binary→shim子processという境界があり、test codeから
// shimへ渡せるchannelがenvに限られるため、応答差替・失敗注入はenv機構が構造上
// 必然（L2 FakeBackend〔tests/fake_backend/fake.rs・programmatic注入〕との
// 実装統合はしない。語彙対応はdocs/testing/test-plan.md §2.9対応表）。
//
// | 分類 | env名 | 機構 |
// |---|---|---|
// | 基盤 | `FAKE_LOG` | 全argv記録（`echo "$*"`。呼出回数・順序の不変性は主張しない。TASK-98: 空白結合のためargv境界はlog上で判別不能——空白含みsession名の単一argv要素保証はdelete-session/rename-session各handler内のargv要素数検査が担う） |
// | 基盤 | `FAKE_FIXTURES` | fixture dir（tests/fixtures/zellij） |
// | 基盤 | `FAKE_VERSION` | `--version`応答（未設定時は`zellij 0.44.3`） |
// | fixture選択系 | `FAKE_PANES` / `FAKE_TABS` | FAKE_FIXTURES内のfile名でlist-panes/list-tabs応答fixtureを選択 |
// | 逐次差替系 | `FAKE_DUP_AFTER_OVERRIDE` + `FAKE_DUP_PANES` | 呼出列に依存する応答差替（override-layout呼出後・close-pane呼出前のlist-panesのみdup fixtureへ。FAKE_DUP_PANES未設定時はpanes-three-dup.json） |
// | 応答内容系 | `FAKE_SESSIONS` / `FAKE_SESSIONS_EMPTY` | list-sessions応答の差替 / 0-session環境（rc=1再現） |
// | 応答内容系 | `FAKE_DUMP_SCREEN` / `FAKE_DUMP_SCREEN_FRAMES` | dump-screen応答の差替（FULL接尾） / pane毎呼出回数で内容が変わる応答（@@ATTEMPT@@区切り） |
// | 状態管理系（pane/tab） | `FAKE_PANES_JSON` / `FAKE_TABS_JSON` | 行単位object状態file（`${FAKE_LOG}.panes/.tabs`）の初期source。rename-pane/close-pane/new-pane（.panes）・new-tab/rename-tab-by-id/move-tab（.tabs）の作用をlist-panes/list-tabs応答へ反映 |
// | 状態管理系（session） | （envなし。状態fileはsetup時初期化） | delete-session→`${FAKE_LOG}.deleted`（list-sessions応答から除外）・rename-session→`.renamed`（from名をto名へ差替）・attach -b作成ok→`.sessions`（list-sessions応答へ追加）。FAKE_PANES_JSON/FAKE_TABS_JSONに依存しない独立機構。TASK-98: 削除・改名の応答反映は完全名一致（行から最後の`[Created `markerで名前を抽出し〔区切りASCII空白1文字のみ除去=本体parserと同一論理〕等値比較——空白含み名・marker含有名の共存で先頭word一致・接頭一致が非対象行まで巻き込むのを防ぐ） |
// | 状態管理系 | `FAKE_NEW_PANE_TEMPLATE` / `FAKE_NEW_TAB_TEMPLATE` | new-pane/new-tab handlerの採番object雛形（@ID@/@POS@ placeholder） |
// | 失敗注入系 | `FAKE_FAIL_LIST_TABS` | list-tabsをrc=1 + stderrで失敗 |
// | 失敗注入系 | `FAKE_FAIL_LIST_PANES` | list-panesをrc=1 + stderrで失敗（FAKE_FAIL_LIST_TABSと対称。TASK-64） |
// | 失敗注入系 | `FAKE_FAIL_SESSION` | 単一session名に一致するaction呼出全体をrc=1 + stderrで失敗（action入口の存在確認通過後の失敗=確認中のsession消滅・race模擬。TASK-64） |
// | 失敗注入系 | `FAKE_FAIL_DUMP_SCREEN` / `FAKE_FAIL_DUMP_SCREEN_AFTER` | 失敗させるpane idのspace区切りlist / `pane:成功回数`形式（成功回数超過から失敗） |
// | 失敗注入系 | `FAKE_FAIL_WRITE_CHARS` / `FAKE_FAIL_WRITE` / `FAKE_FAIL_SEND_KEYS` | 失敗させるpane idのspace区切りlist（該当actionをrc=1） |
// | 失敗注入系 | `FAKE_FAIL_MOVE_PANE` / `FAKE_FAIL_MOVE_TAB` | move-pane系/move-tab actionをrc=1 |
// | 失敗注入系 | `FAKE_FAIL_DELETE_SESSION` / `FAKE_FAIL_RENAME_SESSION` | 失敗させる対象名のspace区切りlist（該当handlerをrc=1） |
// | 失敗注入系 | `FAKE_FAIL_DETACH` | action detachをrc=1（TASK-83でFAKE_DETACH_FAILから命名規則へ統一） |
// | 黙失敗系 | `FAKE_MUTE_DELETE_SESSION` / `FAKE_MUTE_RENAME_SESSION` | rc=0だが作用を応答へ反映しない（postcondition不一致模擬） |
// | attach系応答制御 | `FAKE_SESSIONS_CREATE` | attach -b応答（ok / ok-mute / already-exists） |
//
// 命名規則（既存env名は原則維持。新規追加はこの規則に従うこと）:
// - `FAKE_FAIL_<ACTION>` = rc=1 + stderrの失敗注入
// - `FAKE_MUTE_<ACTION>` = rc=0で作用しない黙失敗
// - `FAKE_<SUBCOMMAND>` 系 = 応答の直接差替
// - `FAKE_*_JSON` / `FAKE_*_TEMPLATE` = 状態管理
// - `FAKE_FIXTURES` / `FAKE_LOG` = 基盤
//
// list-panes/list-tabs応答の優先順位（併用時。現行testは相互排他運用）:
// list-panes = `FAKE_PANES_JSON`（状態管理）＞ `FAKE_DUP_AFTER_OVERRIDE`（逐次差替）
//   ＞ `FAKE_PANES`（file名選択）＞ 既定`panes.json`
// list-tabs = `FAKE_TABS_JSON`（状態管理）＞ `FAKE_TABS`（file名選択）＞ 既定`tabs.json`
//
// arm毎の応答規則（TASK-83統一時のreconciliation。両旧shimで異なっていたarmは
// 当該armを行使するtest側の応答を採用した）:
// - list-sessions・dump-screen・action入口session存在検証: list_read_send.rs版
//   （状態管理・差替付き / 不在session名はrc=1 + Session not found）
// - current-tab-info: remap.rs版（tab 0固定の単一object JSON）
// - `--version`: FAKE_VERSION可変（未設定時は旧list_read_send.rs版と同一応答）
//
// TASK-98: 空白含みsession名（`a b`等）対応。FAKE_LOGは`echo "$*"`の空白結合の
// ためargv境界がlog上で判別不能であり、単一argv要素保証は (1) delete-session
// handlerのargv要素数検査（`delete-session <NAME> -f`の3要素固定。NAMEが分割
// 引き渡しされるとrc=1で失敗応答）(2) rename-session handlerの要素数検査
// （`action rename-session <to>`のto単独1要素）が担う。session名の照合
// （action入口の存在検証・delete/renameのlist-sessions応答反映）は、行から
// 最後の`[Created `markerで名前を抽出し（区切りASCII空白1文字のみ除去=本体
// parserと同一論理）等値比較する完全名一致（marker含有名〔実名`a [Created 5m ago]`
// 等〕との共存で`名前 [Created`接頭一致が反例になるため接頭一致は使わない。
// 実装レビューmajor対応）

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// fake zellij shimのwrite↔exec競合（ETXTBSY）対策: binary内のfake zellij
/// testを直列化する（CR59-5・TASK-73。src/zellij/process.rs test moduleと同一構成+契約doc comment）。
/// test fn冒頭でのみ取得すること。wrapper・helper内では再取得しないこと
/// （std::sync::Mutexは非reentrantのため同一thread再取得は永久blockし、
/// failではなくtest hangとして発覚する）。
pub(crate) static FAKE_ZELLIJ_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zellij")
}

/// fake zellij実行可能fileをworkdirに生成し、(PATHに設定すべきdir, log path)を返す。
/// FAKE_VERSION envで --version 応答を、FAKE_PANES/FAKE_TABS envでfixtureを差し替え
pub(crate) fn setup_fake_zellij(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("zelper-fake-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shim = dir.join("zellij");
    let script = r#"#!/usr/bin/env bash
echo "$*" >> "$FAKE_LOG"
if [[ "$1" == "--version" ]]; then echo "zellij ${FAKE_VERSION:-0.44.3}"; exit 0; fi
if [[ "$1" == "list-sessions" ]]; then
  # TASK-89: FAKE_SESSIONS_EMPTY設定時は0-session環境（list-sessions自体がrc=1で
  # 失敗する実環境・M5 (0)実測）を再現。TASK-93: 0件文面は実zellij（0.44.3）と
  # 同じstderr出力へ修正（rc=1 + stderr完全一致 + stdout空がbackendの0件decode
  # 条件。TASK-92 S0実測）。attach -bでの作成NAME反映（${FAKE_LOG}.sessions）が
  # あれば通常応答へ戻る（FAKE_SESSIONS_CREATE=okの状態管理）
  state="${FAKE_LOG}.sessions"
  if [[ -n "$FAKE_SESSIONS_EMPTY" && ! -s "$state" ]]; then
    echo "No active zellij sessions found." >&2; exit 1
  fi
  # TASK-84: ${FAKE_LOG}.deleted（delete-session handlerの削除記録）に含まれる
  # 名前の行は応答から除外する（delete-session後のlist-sessions再取得 =
  # postcondition検証への応答反映）。TASK-98: 除外は完全名一致——行から最後の
  # markerで名前を抽出し（区切りASCII空白1文字のみ除去。本体parserと同一論理）
  # 等値比較する。先頭word一致・`名前 [Created`接頭一致は空白含み名（a b）や
  # marker含有名（a と a [Created 5m ago] の共存）で非対象行まで巻き込むため
  # 使わない（実装レビューmajor対応）
  if [[ -n "$FAKE_SESSIONS" ]]; then all="$FAKE_SESSIONS"; else all="fake-sess [Created 1m ago] (current)"; fi
  [[ -s "$state" ]] && all="$all"$'\n'"$(cat "$state")"
  if [[ -s "${FAKE_LOG}.deleted" ]]; then
    all="$(printf '%s\n' "$all" | awk '
      function sess_name(line,  m, pos, i, last, nm, w) {
        m = "[Created "; pos = 1; last = 0
        while ((i = index(substr(line, pos), m)) > 0) { last = pos + i - 1; pos = last + 1 }
        if (last == 0) { split(line, w, " "); return w[1] }
        if (substr(line, last - 1, 1) == " ") nm = substr(line, 1, last - 2)
        else nm = substr(line, 1, last - 1)
        return nm
      }
      NR==FNR { del[$0] = 1; next }
      { nm = sess_name($0); if (nm in del) next; print }' "${FAKE_LOG}.deleted" -)"
  fi
  # TASK-95: ${FAKE_LOG}.renamed（rename-session handlerの記録）に含まれる
  # from名の行は応答行の先頭field（session名）をto名へ差し替える
  # （rename-session後のlist-sessions再取得 = postcondition検証への応答反映。
  # .deletedの除外機構と同型の状態file機構。行のEXITED表記等の残りは不変）。
  # TASK-98: 差替は完全名一致。.renamed行は `<from> <to>`（toはzellerのUsage
  # 検査で空白不含が保証されるため、最終token=to・残り全体=fromとして復元）。
  # 行側の名前は最後のmarkerで抽出し（本体parserと同一論理）等値比較——
  # marker含有名（x と x [Created 5m ago] の共存）で接頭一致が非対象行まで
  # 書き換えるため使わない（実装レビューmajor対応）
  if [[ -s "${FAKE_LOG}.renamed" ]]; then
    all="$(printf '%s\n' "$all" | awk '
      function sess_name(line,  m, pos, i, last, nm, w) {
        m = "[Created "; pos = 1; last = 0
        while ((i = index(substr(line, pos), m)) > 0) { last = pos + i - 1; pos = last + 1 }
        if (last == 0) { split(line, w, " "); return w[1] }
        if (substr(line, last - 1, 1) == " ") nm = substr(line, 1, last - 2)
        else nm = substr(line, 1, last - 1)
        return nm
      }
      NR==FNR {
        n = split($0, p, " "); from = $0
        sub(/ [^ ]+$/, "", from); rn[from] = p[n]; next
      }
      { nm = sess_name($0)
        if (nm in rn) $0 = rn[nm] substr($0, length(nm) + 1)
        print }' "${FAKE_LOG}.renamed" -)"
  fi
  printf '%s\n' "$all"
  exit 0
fi
if [[ "$1" == "delete-session" ]]; then
  # TASK-84: `zellij delete-session <NAME> -f`（top-level positional。--session接頭
  # なし）。FAKE_FAIL_DELETE_SESSION（失敗させるsession名のspace区切りlist）に一致
  # する名前はrc=1 + stderrで失敗（部分失敗注入。zellerの事前NoTargetで不在には
  # 到達しないため汎用失敗応答でよい）。FAKE_MUTE_DELETE_SESSION=1はrc=0だが
  # list-sessions応答へ反映しない（rc=0・postcondition不一致模擬）。通常は成功
  # stdout 2行（M5 E実測形式。zellerはこれをparseしない）+ 削除名を
  # ${FAKE_LOG}.deletedへ記録
  # TASK-84指摘3: 削除開始の順序検証用marker。shim自体のstderrはbackendに
  # captureされてtestへ届かないため、親（zeller）のfd 2（testがcaptureするstderr）
  # へ直接出す。zellerのwarning行がこのmarkerより前に現れることで「自壊前通知」
  # （§2.3）を検証する
  echo "shim: delete-session handler start" > /proc/$PPID/fd/2 2>/dev/null || true
  # TASK-98: argv要素数検査——`delete-session <NAME> -f`の3要素固定（NAMEは
  # 空白含みでも単一argv要素）。FAKE_LOG（echo "$*"の空白結合）では境界が見え
  # ないため、NAMEを分割して渡す誤実装をここで失敗応答として検出する
  if [[ $# -ne 3 || "$3" != "-f" ]]; then
    echo "shim: delete-session requires exactly: delete-session <NAME> -f (NAME as a single argv element)" >&2
    exit 1
  fi
  dname="$2"
  for fail in $FAKE_FAIL_DELETE_SESSION; do
    if [[ "$fail" == "$dname" ]]; then echo "boom" >&2; exit 1; fi
  done
  if [[ -z "$FAKE_MUTE_DELETE_SESSION" ]]; then
    echo "$dname" >> "${FAKE_LOG}.deleted"
  fi
  echo "force killed and deleted."
  echo "Session: $dname successfully deleted."
  exit 0
fi
if [[ "$1" == "attach" ]]; then
  # TASK-89: `zellij attach ...`（positional形式のtop-level argv。--session接頭なし）
  if [[ "$2" == "-b" ]]; then
    # FAKE_SESSIONS_CREATE: attach -b応答制御（作成ok / ok-mute〔list-sessions応答へ
    # 反映しない〕/ already-exists〔M5 (e1)実測のrc=1・stderr再現〕）
    case "$FAKE_SESSIONS_CREATE" in
      already-exists) echo "Session already exists" >&2; exit 1;;
      ok-mute) exit 0;;
      *) echo "$3 [Created 1m ago] (current)" >> "${FAKE_LOG}.sessions"; exit 0;;
    esac
  fi
  exit 0
fi
if [[ "$3" == "action" ]]; then
  # TASK-89: 不在session名へのactionは実zellijと同じ rc=1・Session not found +
  # 稼働session候補表示で応答（M3 (b2)実測）。稼働session = FAKE_SESSIONS
  # （未設定時は既定のfake-sess）+ attach -b作成分（${FAKE_LOG}.sessions）
  sess="$2"
  state="${FAKE_LOG}.sessions"
  lines=""
  [[ -s "$state" ]] && lines="$(cat "$state")"
  if [[ -n "$FAKE_SESSIONS" ]]; then lines="$lines"$'\n'"$FAKE_SESSIONS"; else lines="$lines"$'\n'"fake-sess [Created 1m ago] (current)"; fi
  # TASK-98: session照合は完全名一致——行から最後のmarkerで名前を抽出し（本体
  # parserと同一論理）等値比較。先頭word比較（$1 == s）は空白含みsession名
  # （sess="a b"）を誤拒否し、`名前 [Created`接頭一致はmarker含有名で不在名を
  # 誤受理するため使わない（実装レビューmajor対応）。候補行の表示も名前全体
  # （最後のmarkerより前から抽出）
  if ! printf '%s\n' "$lines" | awk -v s="$sess" '
    function sess_name(line,  m, pos, i, last, nm, w) {
      m = "[Created "; pos = 1; last = 0
      while ((i = index(substr(line, pos), m)) > 0) { last = pos + i - 1; pos = last + 1 }
      if (last == 0) { split(line, w, " "); return w[1] }
      if (substr(line, last - 1, 1) == " ") nm = substr(line, 1, last - 2)
      else nm = substr(line, 1, last - 1)
      return nm
    }
    { if (sess_name($0) == s) found = 1 }
    END { exit found ? 0 : 1 }'; then
    echo "Session '$sess' not found. The following sessions are active:" >&2
    printf '%s\n' "$lines" | awk '
      function sess_name(line,  m, pos, i, last, nm, w) {
        m = "[Created "; pos = 1; last = 0
        while ((i = index(substr(line, pos), m)) > 0) { last = pos + i - 1; pos = last + 1 }
        if (last == 0) { split(line, w, " "); return w[1] }
        if (substr(line, last - 1, 1) == " ") nm = substr(line, 1, last - 2)
        else nm = substr(line, 1, last - 1)
        return nm
      }
      NF { print sess_name($0) }' >&2
    exit 1
  fi
  shift 3
  action="$1"; shift
  # TASK-64: FAKE_FAIL_SESSION（単一session名）に一致するaction呼出は全体をrc=1 + stderrで
  # 失敗させる。上記存在確認は通過しているsession（list-sessions一覧に存在）のactionのみが
  # 失敗する=確認中のsession消滅・race模擬。list tabs/panesのOperationFailed wrapping契約
  # （list-display.session-missing-action-failure-operation-failed）の検証用。
  # 未設定時は従来挙動のまま（後方互換）
  if [[ -n "$FAKE_FAIL_SESSION" && "$sess" == "$FAKE_FAIL_SESSION" ]]; then
    echo "boom" >&2; exit 1
  fi
  case "$action" in
    list-panes)
      # TASK-64: FAKE_FAIL_LIST_PANES設定時はlist-panesをrc=1 + stderrで失敗させる
      # （FAKE_FAIL_LIST_TABSと対称・未設定時は従来応答のまま後方互換）
      if [[ -n "$FAKE_FAIL_LIST_PANES" ]]; then echo "boom" >&2; exit 1; fi
      # TASK-65: FAKE_PANES_JSON設定時は行単位の状態file（1行=1pane object）で
      # pane状態を管理し、rename-pane/close-pane/new-pane handlerの作用を
      # list-panes応答へ反映する（未設定時はfixture応答。FAKE_TABS_JSONの
      # list-tabs分岐と同一のguard構造）
      state="${FAKE_LOG}.panes"
      if [[ -n "$FAKE_PANES_JSON" ]]; then
        if [[ ! -f "$state" ]]; then
          # 末尾はawkで行送りを保証（sed出力の最終行に改行が無いと、
          # new-pane handlerのappendが前行へ連結し状態fileの行構造が壊れる）
          printf '%s' "$FAKE_PANES_JSON" | tr -d '\n' \
            | sed -e 's/^\[ *//' -e 's/ *\]$//' -e 's/}, *{/}\n{/g' \
            | awk '{ print }' > "$state"
        fi
        echo '['
        awk 'NR > 1 { printf "," } { printf "%s\n", $0 }' "$state"
        echo ']'
        exit 0
      fi
      # TASK-81: dup spawnのemulation——FAKE_DUP_AFTER_OVERRIDE設定時、
      # override-layout呼出後かつclose-pane呼出前のlist-panes応答のみdup pane入り
      # fixture（panes-three-dup.json等）へ差し替える（close-pane後はdupが消えた
      # 状態=反転で照合成立した結果を表現）
      if [[ -n "${FAKE_DUP_AFTER_OVERRIDE:-}" ]] \
        && grep -q "action override-layout" "$FAKE_LOG" \
        && ! grep -q "action close-pane" "$FAKE_LOG"; then
        cat "$FAKE_FIXTURES/${FAKE_DUP_PANES:-panes-three-dup.json}"
      else
        cat "$FAKE_FIXTURES/${FAKE_PANES:-panes.json}"
      fi
      exit 0;;
    list-tabs)
      if [[ -n "$FAKE_FAIL_LIST_TABS" ]]; then echo "boom" >&2; exit 1; fi
      # TASK-88: FAKE_TABS_JSON設定時は行単位の状態file（1行=1tab object）で
      # tab順序を管理し、move-tabのposition移動をlist-tabs応答へ反映する
      # （未設定時は従来のfixture応答）
      state="${FAKE_LOG}.tabs"
      if [[ -n "$FAKE_TABS_JSON" ]]; then
        if [[ ! -f "$state" ]]; then
          printf '%s' "$FAKE_TABS_JSON" | tr -d '\n' \
            | sed -e 's/^\[ *//' -e 's/ *\]$//' -e 's/}, *{/}\n{/g' > "$state"
        fi
        echo '['
        awk 'NR > 1 { printf "," } { printf "%s\n", $0 }' "$state"
        echo ']'
        exit 0
      fi
      cat "$FAKE_FIXTURES/${FAKE_TABS:-tabs.json}"; exit 0;;
    current-tab-info)
      # anchor = tab 0（fixture最初のtab。単体object形式。remap.rs版を正本化）
      printf '{"position":0,"name":"Tab #1","active":true,"panes_to_hide":0,"is_fullscreen_active":false,"is_sync_panes_active":false,"are_floating_panes_visible":true,"other_focused_clients":null,"active_swap_layout_name":null,"is_swap_layout_dirty":false,"viewport_rows":60,"viewport_columns":200,"display_area_rows":60,"display_area_columns":200,"selectable_tiled_panes_count":3,"selectable_floating_panes_count":0,"tab_id":0,"has_bell_notification":null,"is_flashing_bell":false}\n'
      exit 0;;
    dump-screen)
      pane=""; full=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) pane="$2"; shift 2;;
          -f) full="FULL"; shift;;
          *) shift;;
        esac
      done
      # pane毎のdump-screen呼出回数counter（FAKE_DUMP_SCREEN_FRAMES /
      # FAKE_FAIL_DUMP_SCREEN_AFTERで使用。counter fileはshim内部管理）
      cnt="${FAKE_LOG}.cnt.$pane"
      n=0
      [[ -f "$cnt" ]] && n=$(<"$cnt")
      n=$((n + 1))
      printf '%s' "$n" > "$cnt"
      # FAKE_FAIL_DUMP_SCREEN_AFTER（pane:成功回数のspace区切りlist）に一致するpaneは
      # 成功回数を超えた呼び出しから失敗させる（polling中のpane消滅模擬。REVIEW-1）
      for spec in $FAKE_FAIL_DUMP_SCREEN_AFTER; do
        if [[ "$spec" == "$pane:"* ]]; then
          limit="${spec##*:}"
          if (( n > limit )); then echo "boom" >&2; exit 1; fi
        fi
      done
      # FAKE_FAIL_DUMP_SCREEN（失敗させるpane idのspace区切りlist）に一致するpaneは
      # 失敗させる。未設定時は現行挙動のまま（後方互換）
      for fail in $FAKE_FAIL_DUMP_SCREEN; do
        if [[ "$fail" == "$pane" ]]; then echo "boom" >&2; exit 1; fi
      done
      # FAKE_DUMP_SCREEN_FRAMES設定時は呼出回数（pane毎counter file）で内容が変わる
      # 応答を返す（@@ATTEMPT@@区切り。frame数を超えたら最終frameを継続。
      # watchの多attempt hit/exhaust再現。未設定時は従来の応答経路）
      if [[ -n "$FAKE_DUMP_SCREEN_FRAMES" ]]; then
        printf '%s' "$FAKE_DUMP_SCREEN_FRAMES" | awk -v n="$n" '
          BEGIN { RS = "@@ATTEMPT@@" }
          { f[NR] = $0 }
          END { if (n > NR) print f[NR]; else print f[n] }'
        exit 0
      fi
      # FAKE_DUMP_SCREEN設定時は応答を差し替え（末尾行へFULL接尾）。未設定時は現状通り
      if [[ -n "$FAKE_DUMP_SCREEN" ]]; then
        printf '%s%s\n' "$FAKE_DUMP_SCREEN" "$full"
      else
        echo "SCREEN[$pane]$full"
      fi
      exit 0;;
    write-chars)
      pane=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) pane="$2"; shift 2;;
          *) shift;;
        esac
      done
      # FAKE_FAIL_WRITE_CHARS（失敗させるpane idのspace区切りlist）に一致するpaneは
      # 失敗させる。未設定時は "ok" exit 0（従来のcatch-all応答と同一）
      for fail in $FAKE_FAIL_WRITE_CHARS; do
        if [[ "$fail" == "$pane" ]]; then echo "boom" >&2; exit 1; fi
      done
      echo "ok"; exit 0;;
    # TASK-79: FAKE_FAIL_WRITE（失敗させるpane idのspace区切りlist）に一致するpaneへの
    # `write -p`（--enter併用時のCR書き込み write 13）は失敗させる。未設定時は
    # rc=0 "ok"（従来のcatch-all応答と同一）
    write)
      wpane=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) wpane="$2"; shift 2;;
          *) shift;;
        esac
      done
      for fail in $FAKE_FAIL_WRITE; do
        if [[ "$fail" == "$wpane" ]]; then echo "boom" >&2; exit 1; fi
      done
      echo "ok"; exit 0;;
    # TASK-79: FAKE_FAIL_SEND_KEYS（失敗させるpane idのspace区切りlist）に一致するpaneへの
    # send-keysは失敗させる。未設定時はrc=0 "ok"（従来のcatch-all応答と同一）
    send-keys)
      kpane=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) kpane="$2"; shift 2;;
          *) shift;;
        esac
      done
      for fail in $FAKE_FAIL_SEND_KEYS; do
        if [[ "$fail" == "$kpane" ]]; then echo "boom" >&2; exit 1; fi
      done
      echo "ok"; exit 0;;
    # TASK-89: FAKE_FAIL_DETACH設定時はaction detachを非zero終了させる。
    # 未設定時は rc=0 silent no-op成功（client不在の実zellij挙動・M3 (b)と同一）
    detach)
      if [[ -n "$FAKE_FAIL_DETACH" ]]; then echo "boom" >&2; exit 1; fi
      echo "ok"; exit 0;;
    # TASK-88: FAKE_FAIL_MOVE_PANE設定時はmove-pane系actionを非zero終了させる
    # （OperationFailed exit 5の検証用）。未設定時はrc=0（pane状態は変えないため
    # zeller側の幾何比較はno-op warning経路になる）
    move-pane|move-pane-backwards)
      if [[ -n "$FAKE_FAIL_MOVE_PANE" ]]; then echo "boom" >&2; exit 1; fi
      echo "ok"; exit 0;;
    # TASK-65: FAKE_PANES_JSON状態file（${FAKE_LOG}.panes）が存在するときのみ
    # 作用するpane状態変更handler群（未設定時は従来のcatch-all "ok"のまま）。
    # pane行の特定は "id": N と "is_plugin": B の組（FAKE_PANES_JSONは
    # fixture panes.jsonと同一のfield形式で与える）
    rename-pane)
      rspec=""; rname=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) rspec="$2"; shift 2;;
          *) rname="$1"; shift;;
        esac
      done
      state="${FAKE_LOG}.panes"
      if [[ -f "$state" && -n "$rspec" ]]; then
        # is_pluginの照合値はJSONリテラル（false/true）。0/1では一致しない
        rplug=false
        if [[ "$rspec" == plugin_* ]]; then rplug=true; fi
        awk -v id="\"id\": ${rspec##*_}," -v plug="\"is_plugin\": $rplug," \
            -v name="$rname" '
          index($0, id) && index($0, plug) {
            line = $0
            gsub(/"title": *"[^"]*"/, "\"title\": \"" name "\"", line)
            print line
            next
          }
          { print }
        ' "$state" > "${state}.new" && mv "${state}.new" "$state"
      fi
      echo "ok"; exit 0;;
    close-pane)
      cspec=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) cspec="$2"; shift 2;;
          *) shift;;
        esac
      done
      state="${FAKE_LOG}.panes"
      if [[ -f "$state" && -n "$cspec" ]]; then
        # is_pluginの照合値はJSONリテラル（false/true）。0/1では一致しない
        cplug=false
        if [[ "$cspec" == plugin_* ]]; then cplug=true; fi
        awk -v id="\"id\": ${cspec##*_}," -v plug="\"is_plugin\": $cplug," \
            '! (index($0, id) && index($0, plug))' "$state" \
          > "${state}.new" && mv "${state}.new" "$state"
      fi
      echo "ok"; exit 0;;
    # TASK-65: new-pane。FAKE_PANES_JSON設定時はpanes状態fileが不在ならlazy init
    # してから作用（list-panes分岐と同一の初期化）。id採番は既存terminal paneの
    # id最大値+1（terminal_1/2が既存ならterminal_3開始）。採番した terminal_N を
    # stdoutへechoし（parse_created_paneはstdout全体をpane specとしてparse）、
    # FAKE_NEW_PANE_TEMPLATE（@ID@=数値placeholderの1 pane object JSON）の行をappend
    new-pane)
      # 引数境界検証（TASK-67レビューminor-1）: `--`以降は独立引数列（DD-11 pane
      # map: -- command）。--以降に空白を含む引数があれば複数wordの誤結合（例:
      # ["codex","exec"]を"codex exec"の単一引数へ結合する誤実装）として失敗応答
      # する。echo "$*"の空白連結logでは引数境界が見えないためhandler内で検査。
      # 空白含みの正当な単一command語（quote渡し）は本shimの対象外
      seen_dd=0
      for a in "$@"; do
        if [[ $seen_dd -eq 1 && "$a" == *[[:space:]]* ]]; then
          echo "shim: args after -- must be separate argv elements" >&2; exit 1
        fi
        [[ "$a" == "--" ]] && seen_dd=1
      done
      state="${FAKE_LOG}.panes"
      if [[ -n "$FAKE_PANES_JSON" ]]; then
        if [[ ! -f "$state" ]]; then
          # 末尾はawkで行送りを保証（appendの連結防止。list-panes分岐と同一）
          printf '%s' "$FAKE_PANES_JSON" | tr -d '\n' \
            | sed -e 's/^\[ *//' -e 's/ *\]$//' -e 's/}, *{/}\n{/g' \
            | awk '{ print }' > "$state"
        fi
        next_id=$(awk '
          /"is_plugin": *false/ && match($0, /"id": *[0-9]+/) {
            v = substr($0, RSTART + 6, RLENGTH - 6) + 0
            if (v > m) m = v
          }
          END { print m + 1 }' "$state")
        printf '%s\n' "${FAKE_NEW_PANE_TEMPLATE//@ID@/$next_id}" >> "$state"
        echo "terminal_$next_id"
        exit 0
      fi
      echo "ok"; exit 0;;
    # TASK-67: new-tab。FAKE_TABS_JSON設定時はtabs状態fileが不在ならlazy initして
    # から作用（list-tabs分岐と同一の初期化）。id採番は既存tabのtab_id最大値+1。
    # 採番した id をstdoutへechoし（parse_created_tabはstdout全体をu32としてparse）、
    # FAKE_NEW_TAB_TEMPLATE（@ID@=採番id数値・@POS@=append時の行番号=position
    # placeholderの1 tab object JSON）の行をappend（add tab postconditionの
    # list-tabs存在確認への反映。new-pane handlerと同一構成）
    new-tab)
      # 引数境界検証（TASK-67レビューminor-1）: -l/--layout-stringは単一値option
      # （DD-11 tab map: -l NAME・-l PATH・--layout-string KDL）。値の次のtokenが
      # 「不在か--」であることを検査し、値を複数argvへ分割する誤実装（例: inline
      # KDL "layout {}"を"layout"と"{}"の2引数へ分割）を失敗応答で検出する。
      # echo "$*"の空白連結logでは引数境界が見えないためhandler内で検査
      i=1
      while [[ $i -le $# ]]; do
        if [[ "${!i}" == "-l" || "${!i}" == "--layout-string" ]]; then
          v=$((i + 1)); nx=$((i + 2))
          if [[ $v -gt $# ]] || { [[ $nx -le $# ]] && [[ "${!nx}" != "--" ]]; }; then
            echo "shim: -l/--layout-string requires a single value argument" >&2; exit 1
          fi
          i=$nx
        else
          i=$((i + 1))
        fi
      done
      state="${FAKE_LOG}.tabs"
      if [[ -n "$FAKE_TABS_JSON" ]]; then
        if [[ ! -f "$state" ]]; then
          printf '%s' "$FAKE_TABS_JSON" | tr -d '\n' \
            | sed -e 's/^\[ *//' -e 's/ *\]$//' -e 's/}, *{/}\n{/g' \
            | awk '{ print }' > "$state"
        fi
        next_id=$(awk '
          match($0, /"tab_id": *[0-9]+/) {
            v = substr($0, RSTART + 9, RLENGTH - 9) + 0
            if (v > m) m = v
          }
          END { print m + 1 }' "$state")
        next_pos=$(wc -l < "$state")
        line="${FAKE_NEW_TAB_TEMPLATE//@ID@/$next_id}"
        printf '%s\n' "${line//@POS@/$next_pos}" >> "$state"
        echo "$next_id"
        exit 0
      fi
      echo "ok"; exit 0;;
    # TASK-65: rename-tab-by-id <id> <name>。FAKE_TABS_JSONのtabs状態fileの
    # id一致行のnameを書換（rename tabのpostcondition反映）
    rename-tab-by-id)
      state="${FAKE_LOG}.tabs"
      if [[ -f "$state" && -n "$1" ]]; then
        awk -v tid="$1" -v name="$2" '
          $0 ~ ("\"tab_id\": *" tid "[,}]") {
            line = $0
            gsub(/"name": *"[^"]*"/, "\"name\": \"" name "\"", line)
            print line
            next
          }
          { print }
        ' "$state" > "${state}.new" && mv "${state}.new" "$state"
      fi
      echo "ok"; exit 0;;
    # TASK-95: `zellij --session <from> action rename-session <to>`（run_action_as
    # 形式・M4 H実測）。FAKE_FAIL_RENAME_SESSION（失敗させるfrom名のspace区切り
    # list）に一致するfromはrc=1 + stderrで失敗（実行段の失敗注入。zellerの事前
    # 検査で不在・衝突へは到達しないため汎用失敗応答——FAKE_FAIL_DELETE_SESSION
    # と同一pattern。L3条件では行使しないが実zellij応答形式の再現資産として機構
    # を維持〔設計§2.10行使分担〕）。FAKE_MUTE_RENAME_SESSION=1はrc=0だが
    # ${FAKE_LOG}.renamedへ記録しない（rc=0黙失敗模擬。M4 I型）。通常はrc=0 +
    # renamed状態fileへ `<from> <to>` 行を追記
    rename-session)
      # TASK-98: argv要素数検査——`action rename-session <to>`のto単独1要素
      # （toを分割して渡す誤実装の検出。FAKE_LOGの空白結合では境界が見えない）
      if [[ $# -ne 1 ]]; then
        echo "shim: rename-session requires exactly one NEW_NAME argv element" >&2
        exit 1
      fi
      for fail in $FAKE_FAIL_RENAME_SESSION; do
        if [[ "$fail" == "$sess" ]]; then echo "boom" >&2; exit 1; fi
      done
      if [[ -z "$FAKE_MUTE_RENAME_SESSION" ]]; then
        # from（$sess）は空白含み可。反映側（list-sessions分岐）は行全体から
        # 最終token（=to）を除いてfromを復元する
        echo "$sess $1" >> "${FAKE_LOG}.renamed"
      fi
      exit 0;;
    # TASK-88: FAKE_FAIL_MOVE_TAB設定時はmove-tabを非zero終了させる。未設定時は
    # FAKE_TABS_JSONの状態file上で1 position移動（edgeはwrap）し、各行のposition
    # fieldを行番号へ再採番する（tab交換の2d-1回呼出に対してpostconditionの
    # position交換が成立する応答）
    move-tab)
      if [[ -n "$FAKE_FAIL_MOVE_TAB" ]]; then echo "boom" >&2; exit 1; fi
      dir=""; tid=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          left|right) dir="$1"; shift;;
          -t) tid="$2"; shift 2;;
          *) shift;;
        esac
      done
      state="${FAKE_LOG}.tabs"
      if [[ -f "$state" && -n "$tid" ]]; then
        awk -v dir="$dir" -v tid="$tid" '
          { a[NR] = $0 }
          END {
            n = NR; pos = 0
            for (i = 1; i <= n; i++)
              if (a[i] ~ ("\"tab_id\": *" tid "[,}]")) { pos = i; break }
            if (pos == 0) { for (i = 1; i <= n; i++) print a[i]; exit }
            line = a[pos]
            if (dir == "right") new_pos = (pos < n) ? pos + 1 : 1
            else new_pos = (pos > 1) ? pos - 1 : n
            k = 0
            for (j = 1; j <= n; j++) if (j != pos) b[++k] = a[j]
            k = 0
            for (i = 1; i <= n; i++) {
              if (i == new_pos) out[i] = line
              else { k++; out[i] = b[k] }
            }
            for (i = 1; i <= n; i++) {
              l = out[i]
              rep = "\"position\": " (i - 1)
              gsub(/"position": *[0-9]+/, rep, l)
              print l
            }
          }' "$state" > "${state}.new" && mv "${state}.new" "$state"
      fi
      echo "ok"; exit 0;;
    *) echo "ok"; exit 0;;
  esac
fi
exit 0
"#;
    // 実行内容とmode確定後にのみ公開する。最終の実行pathはwrite open状態にならない
    // （LinuxのETXTBSY回避）。
    let staged = dir.join("zellij.staged");
    std::fs::write(&staged, script).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(staged, &shim).unwrap();
    let log = dir.join("calls.log");
    // TASK-89 REVIEW-2指摘8: FAKE_SESSIONS_CREATEの状態file（${FAKE_LOG}.sessions）を
    // setup時に初期化する（PID再利用で既存dirを再利用した際の前回作成分の残留を
    // 排除。空fileはshimの [[ -s ]] で「無し」と同一扱い）
    std::fs::write(dir.join("calls.log.sessions"), "").unwrap();
    // TASK-88 REVIEW-3指摘4: FAKE_TABS_JSONのtab順序状態file（${FAKE_LOG}.tabs）も
    // setup時に削除する（存在するとshimが入力JSONを無視して前回の交換後状態を
    // 返すため、PID再利用でdirを再利用した際にargv列・期待positionがずれる。
    // 削除によりshimがFAKE_TABS_JSONから再初期化する）
    let _ = std::fs::remove_file(dir.join("calls.log.tabs"));
    // TASK-84: delete-session handlerの削除記録（${FAKE_LOG}.deleted）もsetup時に
    // 初期化する（.sessionsと同一の理由: PID再利用でdirを再利用した際の前回削除
    // 分の残留を排除。空fileはshimの [[ -s ]] で「無し」と同一扱い）
    std::fs::write(dir.join("calls.log.deleted"), "").unwrap();
    // TASK-95: rename-session handlerの記録（${FAKE_LOG}.renamed）もsetup時に
    // 初期化する（.deletedと同一の理由: PID再利用でdirを再利用した際の前回分の
    // 残留を排除。空fileはshimの [[ -s ]] で「無し」と同一扱い）
    std::fs::write(dir.join("calls.log.renamed"), "").unwrap();
    // TASK-65: FAKE_PANES_JSONのpane状態file（${FAKE_LOG}.panes）もsetup時に
    // 削除する（PID再利用で既存dirを再利用した際の前回のrename/close/new-pane
    // 作用後状態の残留を排除。tabsと同一の理由）
    let _ = std::fs::remove_file(dir.join("calls.log.panes"));
    (dir, log)
}
