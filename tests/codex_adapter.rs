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
fn flags_archived_sessions_by_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let active = tmp.path().join("sessions/2026/06/04");
    let archived = tmp.path().join("archived_sessions");
    std::fs::create_dir_all(&active).unwrap();
    std::fs::create_dir_all(&archived).unwrap();
    let line = "{\"type\":\"session_meta\",\"timestamp\":\"2026-06-04T10:00:00.000Z\",\"payload\":{\"id\":\"s\",\"cwd\":\"/x\"}}\n";
    let active_file = active.join("rollout-2026-06-04T10-00-00-active.jsonl");
    let archived_file = archived.join("rollout-2026-06-04T10-00-00-archived.jsonl");
    std::fs::write(&active_file, line).unwrap();
    std::fs::write(&archived_file, line).unwrap();

    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    assert!(
        !adapter.parse(&active_file).unwrap().archived,
        "sessions under sessions/ are not archived"
    );
    assert!(
        adapter.parse(&archived_file).unwrap().archived,
        "sessions under archived_sessions/ are archived"
    );
}

#[test]
fn codex_unarchive_command_wraps_session_id() {
    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter
        .parse(&fixture("rollout-2026-06-04T10-00-00-codexsample.jsonl"))
        .unwrap();
    assert_eq!(
        adapter.unarchive_command(&s),
        Some(vec!["codex".into(), "unarchive".into(), s.id.clone()])
    );
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
    let path =
        std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
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
    let path =
        std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let msgs = a.transcript(path).unwrap();
    // only the user_message + agent_message survive; response_item/function_call/token_count dropped
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Agent);
}

#[test]
fn codex_preserves_long_normalized_title() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp
        .path()
        .join("rollout-2026-06-04T10-00-00-longtitle.jsonl");
    let long_title = "please review the terminal result table and make the repository and branch columns fit their visible content before the title column receives leftover width";
    let meta = serde_json::json!({
        "type": "session_meta",
        "timestamp": "2026-06-04T10:00:00.000Z",
        "payload": { "id": "longtitle", "cwd": "/w" }
    });
    let message = serde_json::json!({
        "type": "event_msg",
        "timestamp": "2026-06-04T10:00:01.000Z",
        "payload": {
            "type": "user_message",
            "message": long_title.replace(' ', " \n ")
        }
    });
    std::fs::write(&file, format!("{meta}\n{message}\n")).unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&file).unwrap();
    assert_eq!(s.title, long_title);
    assert!(s.title.chars().count() > 80);
}

#[test]
fn codex_filters_event_message_tags_and_external_tool_blocks() {
    use hop::core::{Block, Role};

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp
        .path()
        .join("rollout-2026-06-04T10-00-00-tagsample.jsonl");
    std::fs::write(
        &file,
        concat!(
            r#"{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"tagsample","cwd":"/w"}}"#,
            "\n",
            r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:01.000Z","payload":{"type":"user_message","message":"<context>check last commit</context>\n- top\n  - nested"}}"#,
            "\n",
            r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:02.000Z","payload":{"type":"user_message","message":"<command-name>/clear</command-name>"}}"#,
            "\n",
            r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:03.000Z","payload":{"type":"agent_message","message":"[external_agent_tool_call: Bash]\ndescription: list files\ncommand: ls\n[/external_agent_tool_call]\nDone after tool."}}"#,
            "\n",
            r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:04.000Z","payload":{"type":"agent_message","message":"<environment_context>\n<cwd>/w</cwd>\n</environment_context>"}}"#,
            "\n",
        ),
    )
    .unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&file).unwrap();
    assert_eq!(s.title, "check last commit - top - nested");
    assert_eq!(s.message_count, 2);
    assert!(s.content.contains("check last commit"));
    assert!(s.content.contains("Done after tool."));
    assert!(!s.content.contains("<context>"));
    assert!(!s.content.contains("external_agent_tool_call"));
    assert!(!s.content.contains("<command-name>"));
    assert!(!s.content.contains("environment_context"));

    let msgs = adapter.transcript(&file).unwrap();
    assert_eq!(msgs[0].role, Role::User);
    assert!(matches!(
        msgs[0].blocks.first(),
        Some(Block::Prose(text)) if text.contains("  - nested")
    ));
}
