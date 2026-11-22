use crate::cli::FilterArgs;
use crate::domain::PaneState;
use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::output::json::TargetedResult;
use crate::zellij::ZellijBackend;
use std::time::{Duration, Instant};

/// watch実行の引数（DD-15。main.rs dispatchから構築）
#[derive(Debug)]
pub struct WatchArgs<'a> {
    pub panes: &'a [String],
    pub filter: &'a FilterArgs,
    /// PATTERNを正規表現として照合（大文字小文字無視は`(?i)`）
    pub regex: bool,
    /// scrollback込みで取得（dump-screen -f）
    pub full: bool,
    /// polling間隔（秒）。>= 1.0 かつ有限
    pub interval: f64,
    /// 最大試行回数
    pub max: u32,
    /// 監視pattern（`--`以降。v1はちょうど1件）
    pub pattern: &'a [String],
    pub json: bool,
}

/// watchのhit報告（run()の出力内容。data 7 fieldと対応）
#[derive(Debug)]
pub struct WatchHit {
    pub pane: PaneState,
    pub pattern: String,
    pub regex: bool,
    pub attempts_used: u32,
    pub elapsed_secs: f64,
    pub matched_lines: Vec<String>,
    pub pane_errors: Vec<TargetedResult<String>>,
}

pub fn run(backend: &dyn ZellijBackend, args: &WatchArgs) -> Result<(), ZelperError> {
    let hit = execute(backend, args, std::thread::sleep)?;
    report(&hit, args.json);
    Ok(())
}

/// polling loop本体（DD-15・TASK-87設計§2.6）。出力は行わない。
/// attempt間のsleepは引数経由で注入可能（prodはstd::thread::sleep・testは記録用。
/// REVIEW-1: sleep位置〔初期sleepなし・最終attempt後なし〕のtest保証のため）
pub fn execute<S: FnMut(Duration)>(
    backend: &dyn ZellijBackend,
    args: &WatchArgs,
    mut sleep: S,
) -> Result<WatchHit, ZelperError> {
    // 1. 引数検証（違反はUsage exit 2。対象解決・polling開始より前）
    if args.pattern.len() != 1 {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            format!(
                "watch requires exactly one PATTERN after `--` (got {})",
                args.pattern.len()
            ),
        ));
    }
    let pattern = args.pattern[0].clone();
    if pattern.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "PATTERN must not be empty; an empty pattern would match every line",
        ));
    }
    if !(args.interval.is_finite() && args.interval >= 1.0) {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "--interval must be a finite number of seconds >= 1.0",
        ));
    }
    // 有限かつ下限を満たしていてもDurationへ変換不能な巨大値（例: 1e30）は
    // sleep時の変換panicを招くため引数検証の段階で拒否する（REVIEW-1）
    let interval_dur = Duration::try_from_secs_f64(args.interval)
        .map_err(|_| ZelperError::new(ErrorClass::Usage, "--interval is too large"))?;
    if args.max < 1 {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "--max (max attempts) must be at least 1",
        ));
    }
    let re = if args.regex {
        Some(regex::Regex::new(&pattern).map_err(|e| {
            ZelperError::new(ErrorClass::Usage, format!("invalid --regex pattern: {e}"))
        })?)
    } else {
        None
    };
    let matches_line = |line: &str| -> bool {
        match &re {
            Some(r) => r.is_match(line),
            None => line.contains(&pattern),
        }
    };

    // 2. 対象解決（開始時に1回のみ。毎poll再解決しない）
    let all_panes = backend.list_panes()?;
    let spec = super::read::build_spec(backend, args.panes, args.filter)?;
    let set = crate::selector::resolve(&spec, &all_panes)?;
    let total = set.panes.len();

    // 3. polling loop（attempt 1はsleepなし即時実行。最終attempt後はsleepしない）
    let mut remaining: Vec<PaneState> = set.panes.clone();
    let mut pane_errors: Vec<TargetedResult<String>> = Vec::new();
    let start = Instant::now();
    let mut attempts_used: u32 = 0;
    loop {
        attempts_used += 1;
        let mut hit: Option<(PaneState, Vec<String>)> = None;
        let mut next: Vec<PaneState> = Vec::with_capacity(remaining.len());
        for pane in &remaining {
            match backend.dump_screen(&pane.id, args.full) {
                Ok(text) => {
                    // 照合はdump-screen生テキストの行単位（M7実測: escape混入なしのため
                    // ANSI正規化は不導入）。hit paneはvisual order順の最初
                    if hit.is_none() {
                        let matched: Vec<String> = text
                            .lines()
                            .filter(|l| matches_line(l))
                            .map(|l| l.to_string())
                            .collect();
                        if !matched.is_empty() {
                            hit = Some((pane.clone(), matched));
                            break;
                        }
                    }
                    next.push(pane.clone());
                }
                Err(e) => {
                    // dump失敗（pane消滅等）: 当該paneを以後のattemptから除外し
                    // pane_errorsへ記録。他paneは継続
                    pane_errors.push(TargetedResult {
                        target: pane.id.as_spec(),
                        ok: false,
                        detail: None,
                        error: Some(e.message().to_string()),
                    });
                }
            }
        }
        if let Some((pane, matched_lines)) = hit {
            return Ok(WatchHit {
                pane,
                pattern,
                regex: args.regex,
                attempts_used,
                elapsed_secs: start.elapsed().as_secs_f64(),
                matched_lines,
                pane_errors,
            });
        }
        remaining = next;
        if attempts_used >= args.max {
            break;
        }
        // attempt < max → 前poll終了後のsleep（drift許容）
        sleep(interval_dur);
    }

    // 4. 打ち切り（no hit）。pane障害なし/全pane障害はOperationFailed 5・
    // 一部pane障害はPartialFailure 6
    let elapsed_secs = start.elapsed().as_secs_f64();
    let approx = (args.max as f64 * args.interval).trunc() as u64;
    let not_found = |detail: String| {
        ZelperError::new(
            ErrorClass::OperationFailed,
            format!(
                "pattern not found within {} attempts (~{approx}s){detail}",
                args.max
            ),
        )
        .with_data(serde_json::json!({
            "attempts_used": attempts_used,
            "elapsed_secs": elapsed_secs,
            "pattern": pattern,
        }))
    };
    if pane_errors.is_empty() {
        Err(not_found(String::new()))
    } else if pane_errors.len() < total {
        Err(ZelperError::new(
            ErrorClass::PartialFailure,
            format!(
                "pattern not found within {} attempts (~{approx}s); {}/{} panes stopped responding",
                args.max,
                pane_errors.len(),
                total
            ),
        )
        .with_data(serde_json::json!({ "results": pane_errors })))
    } else {
        Err(not_found(format!(
            "; all {total} target panes stopped responding"
        )))
    }
}

/// hit報告の出力。JSONはdata 7 field（DD-15）。hit pane単一のobjectであり
/// DD-4.2「multi-target操作はdata.results[]」の一般規定に対する例外
fn report(hit: &WatchHit, json: bool) {
    if json {
        let env = serde_json::json!({
            "schema_version": output::json::SCHEMA_VERSION,
            "ok": true,
            "data": {
                "target": hit.pane.id.as_spec(),
                "pattern": hit.pattern,
                "regex": hit.regex,
                "attempts_used": hit.attempts_used,
                "elapsed_secs": hit.elapsed_secs,
                "matched_lines": hit.matched_lines,
                "pane_errors": hit.pane_errors,
            },
        });
        println!("{env}");
    } else {
        // readと同型のpane header + match行すべて
        println!(
            "=== {} ({}, tab:{} {}) ===",
            hit.pane.id.as_spec(),
            hit.pane.title,
            hit.pane.tab_id.0,
            hit.pane.tab_name
        );
        for line in &hit.matched_lines {
            println!("{line}");
        }
        for e in &hit.pane_errors {
            println!("FAILED {}: {}", e.target, e.error.as_deref().unwrap_or("?"));
        }
    }
}
