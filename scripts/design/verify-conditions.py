#!/usr/bin/env python3
"""zelper条件書verify script（docs/design/conditions-migration.md §5）.

tests/design/の条件書（*.toml）とcovers tag（`// [covers:<id>]`）の機械検証。
構造規則の正本はtest-structure skill（~/.claude/skills/test-structure/SKILL.md）、
適用仕様はdocs/design/conditions-migration.md §5。

検査:
  (a) dangling tag: 全tagのidがいずれかの条件書のcondition idと一致
  (b) 網羅: 全condition idが少なくとも1つのtagでcoverされている
  (c) schema: [meta]必須field・condition id形式・全局id一意・area=stem一致・
      target_function/description/given非空・expect_* 1つ以上・
      verified=false時unverifiable_reason非空・source_lines形式
  (d) source_lines自動同期: tag走査結果（正本）への書き戻し。置換後text全体を
      in-memoryでparse検証し、pass時のみ書込み（meta等の他fieldには触れない）
  (e) [[excluded]]構造: item/source/reason非空・decided_atのYYYY-MM-DD形式・
      decided_byの値域（claude/human/codex）。空配列は許容
  (f) --complete時のみ: #[test]を持つ全fn（tests/・src/・plugin/src/）への
      tag付け漏れ検出

起動: uv run --no-project scripts/design/verify-conditions.py [options]
exit code: 0 = pass / 1 = fail / 2 = usage error

--bootstrapの既知の非対応（skill §4「手動コメント保持」）: tNN fileを再生成する
場合は既存fileの手動コメント（meta直前のcomment行等）を保持せず上書きする。
stable idが付いたfile・読み取れないfileはskipして上書きしない。本移行では
tmp/使い捨ての対照表生成にのみ使用するため実害なし（設計書§5.5）。
"""

import argparse
import os
import re
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

if sys.version_info < (3, 11):
    print(
        "FAIL: python 3.11+ required (tomllib unavailable). "
        "起動は uv run --no-project scripts/design/verify-conditions.py",
        file=sys.stderr,
    )
    sys.exit(1)

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
DESIGN_DIR = REPO_ROOT / "tests" / "design"
SCAN_GLOBS = ("tests/**/*.rs", "src/**/*.rs", "plugin/src/**/*.rs")

TAG_RE = re.compile(r"^\s*//\s*\[covers:([a-z0-9.-]+)\]\s*$")
TEST_ATTR_RE = re.compile(r"^\s*#\[test\]\s*$")
FN_RE = re.compile(
    r"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?"
    r'(?:extern\s+"[A-Za-z0-9_]*"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)'
)
ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*\.[a-z0-9]+(?:-[a-z0-9]+)*$")
TNN_ID_RE = re.compile(r"^[a-z0-9-]+\.t\d+$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
SOURCE_LINE_RE = re.compile(r"^[^:\s]+:\d+$")
DECIDED_BY = ("claude", "human", "codex")
EXPECT_FIELDS = (
    "expect_return",
    "expect_return_shape",
    "expect_throw",
    "expect_no_throw",
)
META_FIELDS = (
    "feature",
    "source_file",
    "verified",
    "verified_at",
    "verified_by",
    "test_command",
    "source_hash",
)
CHECK_ORDER = (
    "dangling-tag",
    "coverage",
    "schema",
    "excluded",
    "source-lines",
    "untagged",
)
JST = timezone(timedelta(hours=9))


# ---------------------------------------------------------------------------
# 走査（scanner）


def scan_rust_file(path):
    """rust file 1件を走査し、fn単位のentry列を返す。

    紐付け規則（設計書§4.4）: tag行を検出した後、同じfile内で以降最初に現れる
    fn宣言（attribute行・doc comment行を読み飛ばす）を対象fnとみなす。
    source_linesにはtag行の位置（repo相対 path:line）を記録する。

    returns: (entries, error)
    entries: {file, fn, fn_line, test_attr_line, is_test, tags, doc}
      tags は [(id, tag行番号)]。doc は直前の /// 群を結合した文字列。
    """
    entries = []
    rel = path.relative_to(REPO_ROOT).as_posix()
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        return [], [(rel, f"read error: {e}")]
    pending_tags = []
    pending_test_at = None
    pending_doc = []
    for line_no, line in enumerate(text.splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith("///"):
            pending_doc.append(stripped[3:].strip())
            continue
        if stripped.startswith("//"):
            m = TAG_RE.match(line)
            if m:
                pending_tags.append((m.group(1), line_no))
            continue
        if TEST_ATTR_RE.match(line):
            if pending_test_at is None:
                pending_test_at = line_no
            continue
        m = FN_RE.match(line)
        if m:
            entries.append(
                {
                    "file": rel,
                    "fn": m.group(1),
                    "fn_line": line_no,
                    "test_attr_line": pending_test_at,
                    "is_test": pending_test_at is not None,
                    "tags": pending_tags,
                    "doc": " ".join(d for d in pending_doc if d),
                }
            )
            pending_tags = []
            pending_test_at = None
            pending_doc = []
    return entries, []


def scan_repo():
    """SCAN_GLOBSを走査して全fn entryを収集する。"""
    files = set()
    for g in SCAN_GLOBS:
        files.update(REPO_ROOT.glob(g))
    entries = []
    errors = []
    for f in sorted(files):
        file_entries, errs = scan_rust_file(f)
        entries.extend(file_entries)
        errors.extend(errs)
    return files, entries, errors


def build_tag_refs(entries):
    """tag走査結果（正本）: id -> [repo相対 path:line, ...]（昇順）。"""
    refs = {}
    for e in entries:
        for tag_id, line_no in e["tags"]:
            refs.setdefault(tag_id, []).append(f"{e['file']}:{line_no}")
    for v in refs.values():
        v.sort(key=lambda s: (s.rsplit(":", 1)[0], int(s.rsplit(":", 1)[1])))
    return refs


def untagged_test_fns(entries):
    """#[test]を持ち直前tag行がないfn。"""
    return [e for e in entries if e["is_test"] and not e["tags"]]


# ---------------------------------------------------------------------------
# 検査


def as_list(v):
    if v is None:
        return []
    if isinstance(v, dict):
        return [v]
    if isinstance(v, list):
        return v
    return []


def check_condition(add, tname, cond, strict_source, all_ids):
    if not isinstance(cond, dict):
        add("schema", tname, "-", "[[condition]]がtableでない")
        return
    cid = cond.get("id")
    if not isinstance(cid, str) or not ID_RE.match(cid):
        add(
            "schema",
            tname,
            str(cid),
            "id形式が不正（<area>.<kebab-topic>・小文字ASCII英数字と-・区切りの.は1つ・_不可）",
        )
    else:
        all_ids.append((tname, cid))
        area = cid.split(".", 1)[0]
        stem = Path(tname).stem
        if area != stem:
            add("schema", tname, cid, f"area '{area}' != file stem '{stem}'")
    for f in ("target_function", "description", "given"):
        v = cond.get(f)
        if not isinstance(v, str) or not v.strip():
            add("schema", tname, str(cid), f"{f}が空")
    expects = [
        f
        for f in EXPECT_FIELDS
        if isinstance(cond.get(f), str) and cond[f].strip()
    ]
    if not expects:
        add(
            "schema",
            tname,
            str(cid),
            "expect_return / expect_return_shape / expect_throw / expect_no_throw のいずれか1つ以上が非空で必要",
        )
    verified = cond.get("verified")
    if verified is not None and not isinstance(verified, bool):
        add("schema", tname, str(cid), "verifiedはbool（true/false）")
    if verified is False:
        reason = cond.get("unverifiable_reason")
        if not isinstance(reason, str) or not reason.strip():
            add(
                "schema",
                tname,
                str(cid),
                "verified = false の条件はunverifiable_reason非空必須",
            )
    sl = cond.get("source_lines")
    if sl is not None:
        if not isinstance(sl, str):
            add("schema", tname, str(cid), "source_linesは文字列（path:lineのカンマ区切り）")
        else:
            for elem in sl.split(","):
                elem = elem.strip()
                if elem and not SOURCE_LINE_RE.match(elem):
                    add(
                        "schema",
                        tname,
                        str(cid),
                        f"source_lines要素 '{elem}' がpath:line形式でない",
                    )
    src = cond.get("source")
    if strict_source and (not isinstance(src, str) or not src.strip()):
        add("schema", tname, str(cid), "--strict-source: source（出典）が空")


def check_excluded(add, tname, ex):
    if not isinstance(ex, dict):
        add("excluded", tname, "-", "[[excluded]]がtableでない")
        return
    for f in ("item", "source", "reason"):
        v = ex.get(f)
        if not isinstance(v, str) or not v.strip():
            add("excluded", tname, "-", f"{f}が空")
    da = ex.get("decided_at")
    if da is not None and (not isinstance(da, str) or not DATE_RE.match(da)):
        add("excluded", tname, "-", f"decided_at '{da}' がYYYY-MM-DD形式でない")
    db = ex.get("decided_by")
    if db is not None and db not in DECIDED_BY:
        add("excluded", tname, "-", f"decided_by '{db}' はclaude/human/codexのいずれかでない")


def check_design(args, refs):
    """tests/design/*.tomlを検査する。

    returns: (fails, toml_files, texts, parsed_ok, n_conditions, n_excluded)
    """
    fails = []

    def add(check, target, ident, detail):
        fails.append((check, target, ident, detail))

    toml_files = sorted(DESIGN_DIR.glob("*.toml"))
    texts = {}
    parsed_ok = set()
    n_conditions = 0
    n_excluded = 0
    all_ids = []

    for tf in toml_files:
        tname = tf.relative_to(REPO_ROOT).as_posix()
        try:
            text = tf.read_text(encoding="utf-8")
        except OSError as e:
            add("schema", tname, "-", f"read error: {e}")
            continue
        try:
            data = tomllib.loads(text)
        except tomllib.TOMLDecodeError as e:
            add("schema", tname, "-", f"toml parse error: {e}")
            continue
        texts[tf] = text
        parsed_ok.add(tf)

        stem = tf.stem
        meta = data.get("meta")
        if not isinstance(meta, dict):
            add("schema", tname, "-", "[meta] tableがない")
        else:
            for f in META_FIELDS:
                if f not in meta:
                    add("schema", tname, "-", f"[meta]必須field欠落: {f}")
            if "feature" in meta and meta["feature"] != stem:
                add(
                    "schema",
                    tname,
                    "-",
                    f"[meta].feature '{meta['feature']}' != file stem '{stem}'",
                )
            if "verified" in meta and not isinstance(meta["verified"], bool):
                add("schema", tname, "-", "[meta].verifiedはbool（true/false）")

        for cond in as_list(data.get("condition")):
            n_conditions += 1
            check_condition(add, tname, cond, args.strict_source, all_ids)
        for ex in as_list(data.get("excluded")):
            n_excluded += 1
            check_excluded(add, tname, ex)

    # 全局id一意性
    seen = set()
    dup_reported = set()
    for tname, cid in all_ids:
        if cid in seen and cid not in dup_reported:
            add("schema", tname, cid, "idが全局一意でない（重複）")
            dup_reported.add(cid)
        seen.add(cid)

    # (a) dangling tag
    id_set = set(seen)
    for tag_id in refs:
        if tag_id not in id_set:
            for loc in refs[tag_id]:
                add("dangling-tag", loc, tag_id, "条件書に存在しないidへのtag")

    # (b) 網羅
    for tname, cid in all_ids:
        if cid not in refs:
            add("coverage", tname, cid, "tagでcoverされていない条件")

    return fails, toml_files, texts, parsed_ok, n_conditions, n_excluded


# ---------------------------------------------------------------------------
# source_lines自動同期（d）


def sync_block(block, refs):
    """1条件block内のsource_lines行を置換/挿入する。

    変更なしはNone、変更ありは新しい行listを返す。toml全体の再直列化は行わず
    当該行のみを扱う（meta等の他fieldには触れない）。
    """
    id_k = None
    cid = None
    id_indent = ""
    sl_idx = None
    sl_indent = ""
    for k, ln in enumerate(block):
        if id_k is None:
            m = re.match(r'^(\s*)id\s*=\s*[\'"]([^\'"]*)[\'"]', ln)
            if m:
                id_k = k
                cid = m.group(2)
                id_indent = m.group(1)
        if sl_idx is None:
            m = re.match(r"^(\s*)source_lines\s*=", ln)
            if m:
                sl_idx = k
                sl_indent = m.group(1)
    if id_k is None:
        return None
    expected = ", ".join(refs.get(cid, []))
    if sl_idx is not None:
        m = re.match(r'^\s*source_lines\s*=\s*"([^"]*)"', block[sl_idx])
        current = m.group(1) if m else None
        if current == expected:
            return None
        eol = "\n" if block[sl_idx].endswith("\n") else ""
        new_block = list(block)
        new_block[sl_idx] = f'{sl_indent}source_lines = "{expected}"{eol}'
        return new_block
    if not expected:
        return None
    new_block = list(block)
    if not block[id_k].endswith("\n"):
        new_block[id_k] = block[id_k] + "\n"
    new_block.insert(id_k + 1, f'{id_indent}source_lines = "{expected}"\n')
    return new_block


def sync_text(text, refs):
    """toml text内の各[[condition]] blockのsource_linesを同期する。

    returns: (new_text, changed)
    """
    lines = text.splitlines(keepends=True)
    out = []
    changed = False
    n = len(lines)
    i = 0
    block_ends = ("[[condition]]", "[[excluded]]", "[meta]")

    def _starts_header(s, headers):
        st = s.lstrip()
        return any(st.startswith(h) for h in headers)

    while i < n:
        line = lines[i]
        if _starts_header(line, ("[[condition]]",)):
            j = i + 1
            while j < n and not _starts_header(lines[j], block_ends):
                j += 1
            block = lines[i:j]
            new_block = sync_block(block, refs)
            if new_block is None:
                out.extend(block)
            else:
                out.extend(new_block)
                changed = True
            i = j
            continue
        out.append(line)
        i += 1
    return "".join(out), changed


# ---------------------------------------------------------------------------
# 本体（verify mode）


def report_fails(fails):
    order = {name: i for i, name in enumerate(CHECK_ORDER)}
    for check, target, ident, detail in sorted(
        fails, key=lambda f: order.get(f[0], 99)
    ):
        print(f"FAIL {check} {target}: {ident}: {detail}")


def run_verify(args):
    _, entries, scan_errors = scan_repo()
    refs = build_tag_refs(entries)

    if args.list_untagged:
        for e in untagged_test_fns(entries):
            print(f"UNTAGGED {e['file']}:{e['test_attr_line']}: {e['fn']}")

    fails, toml_files, texts, parsed_ok, n_conditions, n_excluded = check_design(
        args, refs
    )

    # (f) tag付け漏れ（--complete時のみ有効）
    if args.complete:
        for e in untagged_test_fns(entries):
            fails.append(
                (
                    "untagged",
                    f"{e['file']}:{e['test_attr_line']}",
                    e["fn"],
                    "#[test] fnの直前にtag行がない",
                )
            )

    if fails or scan_errors:
        for rel, detail in scan_errors:
            print(f"FAIL scan {rel}: -: {detail}")
        report_fails(fails)
        sys.exit(1)

    # (d) source_lines同期: 検査pass時にのみ書込む。置換後text全体をin-memory
    # parse検証し、passしたfileのみ書込む（検証fail時は書込まない）
    synced = 0
    for tf in toml_files:
        if tf not in parsed_ok:
            continue
        new_text, changed = sync_text(texts[tf], refs)
        if not changed:
            continue
        try:
            tomllib.loads(new_text)
        except tomllib.TOMLDecodeError as e:
            report_fails(
                [
                    (
                        "source-lines",
                        tf.relative_to(REPO_ROOT).as_posix(),
                        "-",
                        f"同期後textのparse検証失敗（書込みせず）: {e}",
                    )
                ]
            )
            sys.exit(1)
        tmp_path = tf.with_name(tf.name + ".tmp")
        tmp_path.write_text(new_text, encoding="utf-8")
        os.replace(tmp_path, tf)
        synced += 1

    n_tags = sum(len(v) for v in refs.values())
    print(
        f"OK: {len(toml_files)} files, {n_conditions} conditions, "
        f"{n_tags} tags, {n_excluded} excluded ({synced} files synced)"
    )


# ---------------------------------------------------------------------------
# bootstrap mode（移行限定・設計書§5.5）


def escape_toml(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def file_has_stable_id(path):
    """stable id（tNNでないid）を1つでも持つ条件書はskipして上書きしない。

    読み取れないfile（parse error等）もstable扱い（True）とし上書きしない:
    手書き中の壊れたtomlをbootstrapで破壊しないための安全側扱い。
    """
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return True
    for cond in as_list(data.get("condition")):
        cid = cond.get("id") if isinstance(cond, dict) else None
        if isinstance(cid, str) and not TNN_ID_RE.match(cid):
            return True
    return False


def render_bootstrap(rel, stem, fns, verified, today):
    lines = [
        "# bootstrap生成（docs/design/conditions-migration.md §5.5・使い捨て土台）",
        f"# test file: {rel}。source/given/expect_*は出典起点へ書き直す前提（skill §5）",
        "[meta]",
        f'feature = "{stem}"',
        'source_file = ""',
        f'verified = {"true" if verified else "false"}',
        f'verified_at = "{today if verified else ""}"',
        f'verified_by = "{"claude" if verified else ""}"',
        'test_command = ""',
        'source_hash = ""',
        "",
    ]
    for i, e in enumerate(fns, 1):
        lines += [
            "[[condition]]",
            f'id = "{stem}.t{i:02d}"',
            f'target_function = "{e["fn"]}"',
            'source = ""',
            f'description = "{escape_toml(e["doc"])}"',
            f'source_lines = "{rel}:{e["test_attr_line"]}"',
            'given = ""',
            'expect_return_shape = ""',
            "",
        ]
    return "\n".join(lines)


def run_bootstrap(args):
    _, entries, scan_errors = scan_repo()
    if scan_errors:
        for rel, detail in scan_errors:
            print(f"FAIL scan {rel}: -: {detail}")
        sys.exit(1)
    out_dir = (REPO_ROOT / args.out).resolve() if args.out else DESIGN_DIR
    by_file = {}
    for e in entries:
        if e["is_test"]:
            by_file.setdefault(e["file"], []).append(e)
    out_dir.mkdir(parents=True, exist_ok=True)
    today = datetime.now(JST).date().isoformat()
    n_files = n_conds = n_skip = 0
    for rel in sorted(by_file):
        stem = rel[:-3] if rel.endswith(".rs") else rel
        if stem.startswith("tests/"):
            stem = stem[len("tests/"):]
        stem = stem.replace("/", "-").replace("_", "-")
        fns = by_file[rel]
        out_path = out_dir / f"{stem}.toml"
        if out_path.exists() and file_has_stable_id(out_path):
            n_skip += 1
            continue
        out_path.write_text(
            render_bootstrap(rel, stem, fns, args.verified, today),
            encoding="utf-8",
        )
        n_files += 1
        n_conds += len(fns)
    print(
        f"BOOTSTRAP: {n_files} files, {n_conds} conditions generated "
        f"({n_skip} stable files skipped) -> {out_dir}"
    )


# ---------------------------------------------------------------------------


def main():
    p = argparse.ArgumentParser(
        prog="verify-conditions.py",
        description=(
            "tests/design条件書とcovers tagの機械検証"
            "（docs/design/conditions-migration.md §5）"
        ),
    )
    p.add_argument(
        "--strict-source",
        action="store_true",
        help="source（出典）欄の空欄をfailにする",
    )
    p.add_argument(
        "--complete",
        action="store_true",
        help="検査(f)を有効化: #[test]を持つ全fnのtag付け漏れをfailにする",
    )
    p.add_argument(
        "--list-untagged",
        action="store_true",
        help="tagを持たない#[test] fn一覧を出力する（exit codeには影響しない）",
    )
    p.add_argument(
        "--bootstrap",
        action="store_true",
        help="既存testから条件書を抽出生成する移行限定mode",
    )
    p.add_argument(
        "--out",
        metavar="DIR",
        help="--bootstrapの出力先dir（default: tests/design/）",
    )
    p.add_argument(
        "--verified",
        action="store_true",
        help=(
            "--bootstrap併用: metaへ検証記録（verified/verified_at/verified_by）"
            "を記入する（本移行では未使用: 設計書§5.5）"
        ),
    )
    args = p.parse_args()
    if args.verified and not args.bootstrap:
        p.error("--verifiedは--bootstrapと併用します")
    if args.out and not args.bootstrap:
        p.error("--outは--bootstrapと併用します")
    if args.bootstrap and (
        args.strict_source or args.complete or args.list_untagged
    ):
        p.error(
            "--bootstrapは検査option（--strict-source/--complete/--list-untagged）"
            "と併用できません"
        )
    if args.bootstrap:
        run_bootstrap(args)
    else:
        run_verify(args)


if __name__ == "__main__":
    main()
