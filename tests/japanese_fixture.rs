use std::{collections::BTreeMap, fs, process::Command};

use serde_json::Value;

#[test]
fn japanese_fixture_is_valid_and_balanced() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new(env!("CARGO_BIN_EXE_jev-mcp"))
        .args(["eval", "validate", "eval/ja-coding-minimal.jsonl"])
        .current_dir(root)
        .env_remove("TYPESAFE_API_KEY")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "validated 12 evaluation cases\n"
    );

    let input = fs::read_to_string(format!("{root}/eval/ja-coding-minimal.jsonl")).unwrap();
    let cases: Vec<Value> = input
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let counts = cases.iter().fold(BTreeMap::new(), |mut counts, case| {
        *counts
            .entry(case["question"]["type"].as_str().unwrap())
            .or_insert(0) += 1;
        counts
    });
    assert_eq!(counts.get("noul"), Some(&4));
    assert_eq!(counts.get("choice"), Some(&4));
    assert_eq!(counts.get("score"), Some(&4));
    assert!(cases.iter().any(|case| {
        case["expected"]["allowed"]
            .as_array()
            .is_some_and(|allowed| allowed.iter().any(|value| value == "unknown"))
    }));
    assert!(cases.iter().any(|case| {
        case["expected"]["allowed"]
            .as_array()
            .is_some_and(|allowed| allowed.len() > 1)
    }));
}
