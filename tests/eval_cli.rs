use std::process::Command;

#[test]
fn validator_cli_is_offline_and_reports_valid_and_invalid_files() {
    let root = env!("CARGO_MANIFEST_DIR");
    let valid = Command::new(env!("CARGO_BIN_EXE_jev-mcp"))
        .args(["eval", "validate", "tests/fixtures/eval-valid.jsonl"])
        .current_dir(root)
        .env_remove("TYPESAFE_API_KEY")
        .output()
        .unwrap();
    assert!(valid.status.success());
    assert_eq!(
        String::from_utf8(valid.stdout).unwrap(),
        "validated 3 evaluation cases\n"
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_jev-mcp"))
        .args(["eval", "validate", "tests/fixtures/eval-invalid.jsonl"])
        .current_dir(root)
        .env_remove("TYPESAFE_API_KEY")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("must match")
    );
}
