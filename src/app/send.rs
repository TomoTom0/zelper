use crate::cli::FilterArgs;
use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::zellij::ZellijBackend;

pub fn run(
    backend: &dyn ZellijBackend,
    panes: &[String],
    filter: &FilterArgs,
    text: &[String],
    keys: &[String],
    enter: bool,
    json: bool,
) -> Result<(), ZelperError> {
    if text.is_empty() && keys.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "nothing to send: pass TEXT after `--` or use --keys",
        ));
    }
    let all_panes = backend.list_panes()?;
    let spec = super::read::build_spec(backend, panes, filter)?;
    let set = crate::selector::resolve(&spec, &all_panes)?;

    // write-charsは単一文字列: `--`以降のargv要素を単一spaceで連結する
    // （各argv内の空白は保持され、argv間はshellと同じ単一spaceになる）
    let text_joined = text.join(" ");
    let mut results: Vec<output::json::TargetedResult<String>> = Vec::new();

    for pane in &set.panes {
        let outcome: Result<(), String> = (|| {
            if !text.is_empty() {
                backend
                    .write_chars(&pane.id, &text_joined)
                    .map_err(|e| e.message().to_string())?;
                if enter {
                    backend
                        .write_bytes(&pane.id, &[13])
                        .map_err(|e| e.message().to_string())?;
                }
            } else {
                backend
                    .send_keys(&pane.id, keys)
                    .map_err(|e| e.message().to_string())?;
            }
            Ok(())
        })();
        results.push(output::json::TargetedResult {
            target: pane.id.as_spec(),
            ok: outcome.is_ok(),
            detail: None,
            error: outcome.err(),
        });
    }

    let failures = results.iter().filter(|r| !r.ok).count();
    let err = if failures == 0 {
        None
    } else {
        Some(
            ZelperError::new(
                if failures < results.len() {
                    ErrorClass::PartialFailure
                } else {
                    ErrorClass::OperationFailed
                },
                format!("{failures}/{} sends failed", results.len()),
            )
            .with_data(serde_json::json!({ "results": results })),
        )
    };

    if json {
        if failures == 0 {
            let env = serde_json::json!({
                "schema_version": output::json::SCHEMA_VERSION,
                "ok": true,
                "data": { "results": results },
            });
            println!("{env}");
            Ok(())
        } else {
            // 失敗時のenvelope（per-target結果をdataとして同梱）はmainから出力される
            Err(err.expect("failure error"))
        }
    } else {
        for r in &results {
            match (&r.ok, &r.error) {
                (true, _) => println!("sent -> {}", r.target),
                (false, Some(e)) => println!("FAILED -> {}: {}", r.target, e),
                (false, None) => println!("FAILED -> {}", r.target),
            }
        }
        match err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }
}
