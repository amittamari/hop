use hop::adapters::codex::CodexAdapter;
use hop::adapters::Adapter;
use hop::core::AgentId;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex")
        .join(name)
}

#[test]
fn parses_meta_clean_text_and_detects_yolo() {
    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter
        .parse(&fixture("rollout-2026-06-04T10-00-00-codexsample.jsonl"))
        .unwrap();

    assert_eq!(s.agent, AgentId::Codex);
    assert_eq!(s.id, "codexsample"); // from session_meta.payload.id
    assert_eq!(s.directory, "/Users/me/work/web");

    // clean event_msg text only
    assert!(s.content.contains("refactor the auth guard"));
    assert!(s.content.contains("I split the guard into middleware."));
    // injected/tool/meta excluded
    assert!(!s.content.contains("AGENTS.md"));
    assert!(!s.content.contains("environment_context"));
    assert!(!s.content.contains("exec_command"));
    assert!(!s.content.contains("token_count"));

    assert_eq!(s.title, "refactor the auth guard");
    assert_eq!(s.message_count, 2);
    // any turn_context with never + danger-full-access => yolo
    assert!(s.yolo);
}

#[test]
fn scan_keys_by_full_uuid() {
    let tmp = tempfile::tempdir().unwrap();
    let day = tmp.path().join("sessions/2026/06/04");
    std::fs::create_dir_all(&day).unwrap();
    let uuid = "019d1fc6-6379-7e30-9abc-0123456789ab";
    let fname = format!("rollout-2026-06-04T10-00-00-{uuid}.jsonl");
    std::fs::write(
        day.join(&fname),
        "{\"type\":\"session_meta\",\"timestamp\":\"2026-06-04T10:00:00.000Z\",\"payload\":{\"id\":\"019d1fc6-6379-7e30-9abc-0123456789ab\",\"cwd\":\"/x\"}}\n",
    )
    .unwrap();

    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    let scanned = adapter.scan().unwrap();
    assert!(
        scanned.contains_key(uuid),
        "scan should key by the full uuid; got keys: {:?}",
        scanned.keys().collect::<Vec<_>>()
    );
}

#[test]
fn codex_captures_branch_and_repo_url() {
    use hop::adapters::codex::CodexAdapter;
    use hop::adapters::Adapter;
    let path = std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let s = a.parse(path).unwrap();
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.repo_url.as_deref(), Some("git@github.com:me/web.git"));
}

#[test]
fn codex_transcript_roles_and_filters_internals() {
    use hop::adapters::codex::CodexAdapter;
    use hop::adapters::Adapter;
    use hop::core::Role;
    let path = std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let msgs = a.transcript(path).unwrap();
    // only the user_message + agent_message survive; response_item/function_call/token_count dropped
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Agent);
}
