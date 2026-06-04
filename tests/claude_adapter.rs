use hop::adapters::claude::ClaudeAdapter;
use hop::adapters::Adapter;
use hop::core::AgentId;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

#[test]
fn parses_id_cwd_and_excludes_noise() {
    let adapter = ClaudeAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&fixture("sample.jsonl")).unwrap();

    assert_eq!(s.agent, AgentId::Claude);
    assert_eq!(s.id, "sample"); // from filename
    assert_eq!(s.directory, "/Users/me/work/api");
    assert!(s.timestamp > 0); // RFC3339 timestamp parsed, not the 0 fallback

    // content keeps only real user + assistant text
    assert!(s.content.contains("fix the auth refresh token bug"));
    assert!(s.content.contains("The refresh token was dropped on retry."));
    // excluded: local-command output, slash-command, tool_result, tool_use, isMeta
    assert!(!s.content.contains("noise"));
    assert!(!s.content.contains("/clear"));
    assert!(!s.content.contains("done"));
    assert!(!s.content.contains("toolu_x"));
    assert!(!s.content.contains("meta note"));

    // title = first real user prompt; message_count = real text messages
    assert_eq!(s.title, "fix the auth refresh token bug");
    assert_eq!(s.message_count, 2);
    assert!(!s.yolo);
}
