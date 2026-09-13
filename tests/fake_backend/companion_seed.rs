// L2: permissions.kdl seed matrix（test-plan §2.7 (c) R21〜R28・(j) seed書換競合。
// DD-10.4）。tempdir実file込みで検証する。
// 形式はE6実験（tmp/e6_plugin/run3.sh。session起動前seedで自動grantを実証）に基づく:
// node名 = plugin wasm絶対pathのquoted KDL node・権限は子のbare node。
// fail-first: 実装（TASK-39）前にfailする（test-plan §5のseed判定を含む）。
use zelper::companion::{REQUIRED_PERMISSIONS, ensure_permissions, seeded_content};
use zelper::error::ErrorClass;

const NODE: &str = "/cache/zelper/companion/0.1.0/zelper-companion.wasm";

/// companion node block（E6実験のpermissions.kdl形式）
fn node_block(node: &str, perms: &[&str]) -> String {
    let body: String = perms.iter().map(|p| format!("    {p}\n")).collect();
    format!("\"{node}\" {{\n{body}}}\n")
}

fn all_perms() -> Vec<&'static str> {
    REQUIRED_PERMISSIONS.to_vec()
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("zelper-seed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn perm_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("zellij").join("permissions.kdl")
}

// ---- 判定の純粋部分（§5 fail-first対象） ----

// [covers:companion-seed.r21-missing-file-creates-fresh-node]
#[test]
fn r21_missing_file_creates_fresh_node() {
    // file不存在 -> companion nodeのみの新規file内容
    let out = seeded_content(None, NODE).unwrap();
    let expect = node_block(NODE, &all_perms());
    assert_eq!(out.as_deref(), Some(expect.as_str()));
}

// [covers:companion-seed.r22-node-with-all-permissions-is-no-change]
#[test]
fn r22_node_with_all_permissions_is_no_change() {
    let content = node_block(NODE, &all_perms());
    assert_eq!(seeded_content(Some(&content), NODE).unwrap(), None);
}

// [covers:companion-seed.r23-missing-permissions-appended-keeping-others]
#[test]
fn r23_missing_permissions_are_appended_keeping_others() {
    // node存在・権限不足 -> 不足権限のみ追記して書戻し（他node・他権限は保持）
    let content = format!(
        "{}\n{}",
        node_block("/other/plugin.wasm", &["ReadApplicationState"]),
        node_block(NODE, &["ReadApplicationState", "ChangeApplicationState"]),
    );
    let out = seeded_content(Some(&content), NODE).unwrap().unwrap();
    // 不足していた ReadCliPipes が companion node に追記される
    assert!(out.contains(NODE), "raw: {out}");
    assert!(out.contains("ReadCliPipes"), "raw: {out}");
    // 既存の他node・他権限は保持される
    assert!(out.contains("/other/plugin.wasm"), "raw: {out}");
    assert_eq!(out.matches("ReadApplicationState").count(), 2, "raw: {out}");
    // companion node の権限は過剰に重複しない（既存2 + 追記1 = 3行）
    assert_eq!(
        out.matches("ChangeApplicationState").count(),
        1,
        "raw: {out}"
    );
}

// [covers:companion-seed.r24-absent-node-pure-text-append-with-newline-guard]
#[test]
fn r24_absent_node_is_pure_text_append_with_newline_guard() {
    // node不在 -> file末尾へのtext純append（kdl round-trip整形によらない）。
    // 既存内容が改行終端でなければ先頭に改行を補完（設計レビューR1）
    let other = node_block("/other/plugin.wasm", &["ReadApplicationState"]);

    // 改行終端あり
    let out = seeded_content(Some(&other), NODE).unwrap().unwrap();
    assert!(
        out.starts_with(&other),
        "既存内容は改変されない（純append）: {out}"
    );
    assert!(out.contains(&node_block(NODE, &all_perms())), "raw: {out}");

    // 改行終端なし: 前行末尾への連結によるKDL破壊を防ぐ改行補完
    let unterminated = other.trim_end();
    let out = seeded_content(Some(unterminated), NODE).unwrap().unwrap();
    assert!(
        out.starts_with(&format!("{unterminated}\n")),
        "改行終端でなければ先頭に改行を補完: {out}"
    );
    assert!(out.contains(NODE), "raw: {out}");
}

// [covers:companion-seed.r25-unparsable-existing-file-is-preflight-error]
#[test]
fn r25_unparsable_existing_file_is_preflight_error() {
    // 既存fileのKDL parse失敗 -> 書き換えずPreflight error（手動修復を促す）
    let broken = "\"/other/plugin.wasm\" {\n    ReadApplicationState\n"; // 未close
    let err = seeded_content(Some(broken), NODE).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);

    // 実fileも書き換えない
    let dir = tempdir("r25");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, broken).unwrap();
    let err = ensure_permissions(&path, NODE, None).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

// ---- 実file書込 ----

// [covers:companion-seed.r21-ensure-creates-dir-and-file-when-missing]
#[test]
fn r21_ensure_creates_dir_and_file_when_missing() {
    let dir = tempdir("r21");
    let path = perm_path(&dir); // dir/zellij/permissions.kdl（dir/zellijも存在しない）
    ensure_permissions(&path, NODE, None).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, node_block(NODE, &all_perms()));
}

// [covers:companion-seed.r22-ensure-leaves-complete-file-untouched]
#[test]
fn r22_ensure_leaves_complete_file_untouched() {
    let dir = tempdir("r22");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let content = node_block(NODE, &all_perms());
    std::fs::write(&path, &content).unwrap();
    ensure_permissions(&path, NODE, None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
}

// [covers:companion-seed.r26-write-is-atomic-single-complete-content]
#[test]
fn r26_write_is_atomic_single_complete_content() {
    // 書込みは同dir temp file作成 + atomic rename（readerが途中状態を観測しない）。
    // L2では「書込結果が単一の完全な内容であること」を検証する
    // （temp file + renameの機構自体の実機確認はL4。R26の機構固定）
    let dir = tempdir("r26");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let stale = node_block("/old/plugin.wasm", &["ReadApplicationState"]);
    std::fs::write(&path, &stale).unwrap();

    ensure_permissions(&path, NODE, None).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    // 内容は seeded_content の結果と完全に一致する（部分書込・混在がない）
    assert_eq!(after, seeded_content(Some(&stale), NODE).unwrap().unwrap());
    // 同dirに一時fileが残留しない（temp + rename後の状態）
    let siblings: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        siblings,
        vec!["permissions.kdl".to_string()],
        "{siblings:?}"
    );
}

// [covers:companion-seed.r27-conflicting-rewrite-is-retried-on-new-content]
#[test]
fn r27_conflicting_rewrite_is_retried_on_new_content() {
    // 書戻し直前の再読込で内容が変化していたら、変化後の内容に対してやり直す
    // （bounded retry 3回）。1回の競合なら成功する
    let dir = tempdir("r27");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let initial = node_block("/other/plugin.wasm", &["ReadApplicationState"]);
    std::fs::write(&path, &initial).unwrap();

    // zellij他processの書換（競合）を模擬: 再読込のタイミングで1回だけ書き替わる。
    // zellij自身のgrant時書換は既存の他nodeを保持するため、競合内容にも他nodeを
    // 含める（他nodeの消失はzelperのseed対象外の変更）
    let conflict = format!(
        "{}{}",
        node_block("/other/plugin.wasm", &["ReadApplicationState"]),
        node_block(NODE, &["ReadApplicationState"])
    );
    let path_for_hook = path.clone();
    let fired = std::cell::Cell::new(false);
    let hook = move |_p: &std::path::Path| {
        if !fired.get() {
            fired.set(true);
            std::fs::write(&path_for_hook, &conflict).unwrap();
        }
    };
    ensure_permissions(&path, NODE, Some(&hook)).unwrap();

    // 最終内容は競合後の内容に基づく: 競合で書かれたnode（権限1つのみ）に
    // 不足権限が追記され、他nodeは保持される
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("/other/plugin.wasm"), "raw: {after}");
    assert!(after.contains("ReadCliPipes"), "raw: {after}");
    assert!(after.contains("ChangeApplicationState"), "raw: {after}");
}

// [covers:companion-seed.r28-persistent-conflict-aborts-without-writing]
#[test]
fn r28_persistent_conflict_aborts_without_writing() {
    // retry打ち切り後も競合が続く -> 書込まずPreflight error
    // （last-writer-winsで他pluginの権限を黙って失わせない）
    let dir = tempdir("r28");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let initial = node_block("/other/plugin.wasm", &["ReadApplicationState"]);
    std::fs::write(&path, &initial).unwrap();

    // 毎回内容が変わる（カウンタ増加）: 競合が続く
    let path_for_hook = path.clone();
    let counter = std::cell::Cell::new(0u32);
    let hook = move |_p: &std::path::Path| {
        let n = counter.get() + 1;
        counter.set(n);
        std::fs::write(&path_for_hook, format!("\"/race/{n}.wasm\" {{\n}}\n")).unwrap();
    };
    let err = ensure_permissions(&path, NODE, Some(&hook)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(
        err.message().contains("conflict") || err.message().contains("retry"),
        "競合であることが利用者に伝わる: {}",
        err.message()
    );
    // zelperは書いていない: fileはhookの最終書込内容のまま（companion node無し）
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(!after.contains(NODE), "raw: {after}");
}

// ---- C2/C4（TASK-39コードレビュー対応） ----

/// XDG_CACHE_HOME隔離（companion_dir()解決の検証に使用。static Mutexで直列化）
static XDG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn with_isolated_xdg<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
    let _guard = XDG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("zelper-seed-xdg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let prev = std::env::var("XDG_CACHE_HOME").ok();
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きし、即座に復元する
    unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
    let out = f(&dir);
    match prev {
        Some(p) => unsafe { std::env::set_var("XDG_CACHE_HOME", p) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    out
}

// [covers:companion-seed.concurrent-seeds-serialize-via-advisory-lock]
#[test]
fn c2_concurrent_seeds_serialize_via_advisory_lock() {
    // zelper間の書込はadvisory lock（flock）で直列化される: Aがcritical section内で
    // 遅延している間に開始したBは、Aのlock解放（= Aの書込完了）までblockする。
    // Bの所要時間がAの残り保持時間以上になることで固定する
    let dir = tempdir("c2");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let initial = format!(
        "{}{}",
        node_block("/other/plugin.wasm", &["ReadApplicationState"]),
        node_block(NODE, &["ReadApplicationState"]),
    );
    std::fs::write(&path, &initial).unwrap();

    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
    // A: NODEの不足権限を追記する。hook内で300ms待ちcritical sectionを長くする
    let order_a = order.clone();
    let path_a = path.clone();
    let a = std::thread::spawn(move || {
        let hook = move |_p: &std::path::Path| {
            order_a.lock().unwrap().push("A");
            std::thread::sleep(std::time::Duration::from_millis(300));
        };
        ensure_permissions(&path_a, NODE, Some(&hook)).unwrap();
    });
    // Aがhookに入った（= lockを保持してcritical sectionにいる）ことを確認してから
    // Bを開始する（開始タイミングの非決定性を排除）
    let wait_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while order.lock().unwrap().is_empty() {
        assert!(
            std::time::Instant::now() < wait_deadline,
            "Aがhookに入らない（test setup破綻）"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // B: 別versionのcompanion node（存在しないため追記が発生する）
    let node_b = "/cache/zelper/companion/0.2.0/zelper-companion.wasm";
    let order_b = order.clone();
    let path_b = path.clone();
    let b = std::thread::spawn(move || {
        let b_start = std::time::Instant::now();
        let hook = move |_p: &std::path::Path| {
            order_b.lock().unwrap().push("B");
        };
        ensure_permissions(&path_b, node_b, Some(&hook)).unwrap();
        b_start.elapsed()
    });
    a.join().unwrap();
    let b_elapsed = b.join().unwrap();

    // 直列化: BはAの解放待ちでblockするため、Aの残り保持時間(約300ms)以上かかる。
    // lockが無ければBは即座に完了する（回帰するときの判別点）
    assert!(
        b_elapsed >= std::time::Duration::from_millis(250),
        "BはAのlock解放までblockする: {b_elapsed:?}"
    );
    // hook実行順もA -> B
    assert_eq!(*order.lock().unwrap(), vec!["A", "B"]);
    // 最終内容: Aの書込をBが読み、Bがそれに追記する（lost-updateなし）
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("/other/plugin.wasm"), "raw: {after}");
    assert!(after.contains(NODE), "raw: {after}");
    assert!(after.contains(node_b), "raw: {after}");
    assert!(after.contains("ReadCliPipes"), "raw: {after}");
    // lock fileは残留しない
    assert!(
        !path.parent().unwrap().join(".zelper-seed.lock").exists(),
        "lock fileを削除する"
    );
}

// [covers:companion-seed.stale-lock-file-tolerated-and-removed]
#[test]
fn c2_stale_lock_file_is_tolerated_and_removed() {
    // 残留lock file（異常終了等）があってもacquireは機能し、抜けたら削除される
    let dir = tempdir("c2stale");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path.parent().unwrap().join(".zelper-seed.lock"), "stale").unwrap();
    ensure_permissions(&path, NODE, None).unwrap();
    assert!(!path.parent().unwrap().join(".zelper-seed.lock").exists());
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written, node_block(NODE, &all_perms()));
}

// [covers:companion-seed.failed-seed-write-leaves-no-partial-temp-file]
#[test]
fn c4_failed_seed_write_leaves_no_partial_temp_file() {
    // temp file pathをdirectoryで塞いでwrite失敗を発生させる。部分書込のtemp
    // fileが新規に残留しないこと（best-effort cleanup）を確認する
    let dir = tempdir("c4");
    let path = perm_path(&dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    // 権限が1つ欠けているため書込が試みられる
    std::fs::write(&path, node_block(NODE, &["ReadApplicationState"])).unwrap();
    let tmp = path.parent().unwrap().join(format!(
        ".permissions.kdl.zelper-tmp-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::create_dir(&tmp).unwrap();

    let err = ensure_permissions(&path, NODE, None).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    // 残留は事前に作ったdirectoryのみ（zelperは新規fileを作っていない）
    let mut names: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            ".permissions.kdl.zelper-tmp-".to_string() + &std::process::id().to_string(),
            "permissions.kdl".to_string(),
        ],
        "{names:?}"
    );
}

// [covers:companion-seed.failed-wasm-extract-write-leaves-no-partial-temp-file]
#[test]
fn c4_failed_wasm_extract_write_leaves_no_partial_temp_file() {
    // wasm extractのtemp書込失敗でも新規fileを残留しない
    with_isolated_xdg(|_xdg| {
        let dir = zelper::companion::companion_dir().unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join(format!(".zelper-companion.wasm.tmp-{}", std::process::id()));
        std::fs::create_dir(&tmp).unwrap();

        let err = zelper::companion::ensure_companion_wasm().unwrap_err();
        assert_eq!(*err.class(), ErrorClass::Preflight);
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![".zelper-companion.wasm.tmp-".to_string() + &std::process::id().to_string()],
            "{names:?}"
        );
    });
}
