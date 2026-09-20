use std::fs;

#[test]
fn skill_reference_json_examples_are_valid() {
    let directory = format!(
        "{}/skills/jev-decisions/references",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut files = 0;
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        files += 1;
        let markdown = fs::read_to_string(&path).unwrap();
        let examples: Vec<&str> = markdown
            .split("```json")
            .skip(1)
            .map(|section| section.split("```").next().unwrap())
            .collect();
        assert!(
            !examples.is_empty(),
            "{} has no JSON example",
            path.display()
        );
        for example in examples {
            serde_json::from_str::<serde_json::Value>(example).unwrap_or_else(|error| {
                panic!("invalid JSON example in {}: {error}", path.display())
            });
        }
    }
    assert!(files > 0);
}
