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

    assert_eq!(s.meta.agent, AgentId::Claude);
    assert_eq!(s.meta.id, "sample"); // from filename
    assert_eq!(s.meta.directory, "/Users/me/work/api");
    assert!(s.meta.timestamp > 0); // RFC3339 timestamp parsed, not the 0 fallback

    // content keeps only real user + assistant text
    assert!(s.content.contains("fix the auth refresh token bug"));
    assert!(s
        .content
        .contains("The refresh token was dropped on retry."));
    // excluded: local-command output, slash-command, tool_result, tool_use, isMeta
    assert!(!s.content.contains("noise"));
    assert!(!s.content.contains("/clear"));
    assert!(!s.content.contains("done"));
    assert!(!s.content.contains("toolu_x"));
    assert!(!s.content.contains("meta note"));

    // title = first real user prompt; message_count = real text messages
    assert_eq!(s.meta.title, "fix the auth refresh token bug");
    assert_eq!(s.meta.message_count, 2);
    assert!(!s.meta.yolo);
}

#[test]
fn claude_prefers_ai_title_over_first_prompt() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("s.jsonl");
    fs::write(
        &file,
        concat!(
            r#"{"type":"user","cwd":"/w","timestamp":"2026-06-04T13:20:16.361Z","message":{"role":"user","content":"long raw first prompt that should not be the title"}}"#,
            "\n",
            r#"{"type":"ai-title","sessionId":"s","aiTitle":"Concise Claude Conversation Title"}"#,
            "\n",
            r#"{"type":"summary","summary":"Later generic summary should not win"}"#,
            "\n",
        ),
    )
    .unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let s = a.parse(&file).unwrap();
    assert_eq!(s.meta.title, "Concise Claude Conversation Title");
    assert!(s.content.contains("long raw first prompt"));
}

#[test]
fn claude_uses_top_level_summary_title() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("s.jsonl");
    fs::write(
        &file,
        concat!(
            r#"{"type":"summary","summary":"Legacy Claude Summary Title","leafUuid":"u"}"#,
            "\n",
            r#"{"type":"user","cwd":"/w","timestamp":"2026-06-04T13:20:16.361Z","message":{"role":"user","content":"fallback prompt"}}"#,
            "\n",
        ),
    )
    .unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let s = a.parse(&file).unwrap();
    assert_eq!(s.meta.title, "Legacy Claude Summary Title");
}

#[test]
fn claude_preserves_long_normalized_title() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("s.jsonl");
    let long_title = "please review the terminal result table and make the repository and branch columns fit their visible content before the title column receives leftover width";
    let line = serde_json::json!({
        "type": "user",
        "cwd": "/w",
        "timestamp": "2026-06-04T13:20:16.361Z",
        "message": {
            "role": "user",
            "content": long_title.replace(' ', " \n ")
        }
    });
    fs::write(&file, format!("{line}\n")).unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let s = a.parse(&file).unwrap();
    assert_eq!(s.meta.title, long_title);
    assert!(s.meta.title.chars().count() > 80);
}

#[test]
fn claude_captures_branch_and_filters_internals() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    let file = proj.join("s.jsonl");
    fs::write(&file, concat!(
        r#"{"type":"user","cwd":"/w","gitBranch":"feat/x","timestamp":"2026-06-04T13:20:16.361Z","message":{"role":"user","content":"fix the bug"}}"#, "\n",
        r#"{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"},{"type":"tool_use","name":"Bash","id":"t"}]}}"#, "\n",
        r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"meta"}}"#, "\n",
    )).unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let s = a.parse(&file).unwrap();
    assert_eq!(s.meta.branch.as_deref(), Some("feat/x"));
    assert!(s.content.contains("fix the bug"));
    assert!(s.content.contains("done"));
    assert!(!s.content.contains("/clear"));
    assert!(!s.content.contains("meta"));
}

#[test]
fn claude_transcript_has_roles_and_code() {
    use hop::adapters::claude::ClaudeAdapter;
    use hop::adapters::Adapter;
    use hop::core::{Block, Role};
    use std::fs;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("s.jsonl");
    fs::write(&file, concat!(
        r#"{"type":"user","cwd":"/w","message":{"role":"user","content":"hi"}}"#, "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"text\n```rust\nfn a(){}\n```"}]}}"#, "\n",
    )).unwrap();

    let a = ClaudeAdapter::new(tmp.path().to_path_buf());
    let msgs = a.transcript(&file).unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Agent);
    assert!(
        matches!(msgs[1].blocks.last(), Some(Block::Code { lang, .. }) if lang.as_deref() == Some("rust"))
    );
}
