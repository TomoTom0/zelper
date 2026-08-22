// L1: JSON契約のerror class ↔ exit status対応（DD-4.3対応表の全行。test-plan §2.10）
use zelper::error::{ErrorClass, ZelperError};
use zelper::output::json;

fn exit_of(class: ErrorClass) -> i32 {
    ZelperError::new(class, "m").exit_code()
}

#[test]
fn class_exit_mapping_full_table() {
    assert_eq!(exit_of(ErrorClass::Usage), 2);
    assert_eq!(exit_of(ErrorClass::NoTarget), 3);
    assert_eq!(exit_of(ErrorClass::AmbiguousTarget), 3);
    assert_eq!(exit_of(ErrorClass::ZellijUnavailable), 4);
    assert_eq!(exit_of(ErrorClass::UnsupportedVersion), 4);
    assert_eq!(exit_of(ErrorClass::OperationFailed), 5);
    assert_eq!(exit_of(ErrorClass::PartialFailure), 6);
    assert_eq!(exit_of(ErrorClass::Preflight), 7);
    assert_eq!(exit_of(ErrorClass::LayoutNotFound), 7);
    assert_eq!(exit_of(ErrorClass::LayoutInvalid), 7);
    assert_eq!(exit_of(ErrorClass::VerificationFailed), 7);
}

#[test]
fn json_error_envelope_shape() {
    let e = ZelperError::with_candidates(
        ErrorClass::AmbiguousTarget,
        "target matched 2 panes",
        vec!["terminal_3".into(), "terminal_5".into()],
    );
    let out = json::err(&e);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "AmbiguousTarget");
    assert_eq!(v["error"]["candidates"].as_array().unwrap().len(), 2);
}

#[test]
fn json_ok_envelope_shape() {
    let out = json::ok(serde_json::json!({ "sessions": ["a"] }));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["sessions"][0], "a");
}

#[test]
fn targeted_result_stable_fields() {
    let r = json::TargetedResult {
        target: "terminal_3".to_string(),
        ok: false,
        detail: None::<String>,
        error: Some("dump-screen failed".into()),
    };
    let s = serde_json::to_string(&r).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["target"], "terminal_3");
    assert_eq!(v["ok"], false);
    assert!(v.get("detail").is_none()); // skip_serializing_if
    assert_eq!(v["error"], "dump-screen failed");
}
