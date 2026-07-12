use hop::adapters::Adapter;
use hop::adapters::codex::CodexAdapter;
use hop::core::AgentId;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex").join(name)
}

#[test]
fn parses_meta_clean_text_and_detects_yolo() {
    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&fixture("rollout-2026-06-04T10-00-00-codexsample.jsonl")).unwrap();

    assert_eq!(s.meta.agent, AgentId::Codex);
    assert_eq!(s.meta.id, "codexsample"); // from session_meta.payload.id
    assert_eq!(s.meta.directory, "/Users/me/work/web");

    // clean event_msg text only
    assert!(s.content.contains("refactor the auth guard"));
    assert!(s.content.contains("I split the guard into middleware."));
    // injected/tool/meta excluded
    assert!(!s.content.contains("AGENTS.md"));
    assert!(!s.content.contains("environment_context"));
    assert!(!s.content.contains("exec_command"));
    assert!(!s.content.contains("token_count"));

    assert_eq!(s.meta.title, "refactor the auth guard");
    assert_eq!(s.meta.message_count, 2);
    // any turn_context with never + danger-full-access => yolo
    assert!(s.meta.yolo);

    // enrichment metadata (M1/M2/M3)
    assert_eq!(s.meta.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(s.meta.commit.as_deref(), Some("abc1234def"));
    assert_eq!(s.meta.source.as_deref(), Some("cli"));
}

#[test]
fn first_non_empty_model_wins_over_trailing_review_turn() {
    let tmp = tempfile::tempdir().unwrap();
    let day = tmp.path().join("sessions/2026/06/04");
    std::fs::create_dir_all(&day).unwrap();
    let file = day.join("rollout-2026-06-04T10-00-00-modelpick.jsonl");
    let lines = concat!(
        r#"{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"modelpick","cwd":"/x"}}"#,
        "\n",
        r#"{"type":"turn_context","timestamp":"2026-06-04T10:00:01.000Z","payload":{"model":"gpt-5.5"}}"#,
        "\n",
        r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:02.000Z","payload":{"type":"user_message","message":"hi"}}"#,
        "\n",
        r#"{"type":"turn_context","timestamp":"2026-06-04T10:00:03.000Z","payload":{"model":"codex-auto-review"}}"#,
        "\n",
    );
    std::fs::write(&file, lines).unwrap();
    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&file).unwrap();
    assert_eq!(s.meta.model.as_deref(), Some("gpt-5.5"));
}

#[test]
fn object_form_source_reduces_to_variant_key() {
    let tmp = tempfile::tempdir().unwrap();
    let day = tmp.path().join("sessions/2026/06/04");
    std::fs::create_dir_all(&day).unwrap();
    let file = day.join("rollout-2026-06-04T10-00-00-subagent.jsonl");
    // A nested SubAgent source must not fail the session_meta line: cwd and the
    // message still parse, and `source` reduces to the "subagent" marker.
    let lines = concat!(
        r#"{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"subagent","cwd":"/repo","source":{"subagent":{"other":"guardian"}}}}"#,
        "\n",
        r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:02.000Z","payload":{"type":"user_message","message":"do the thing"}}"#,
        "\n",
    );
    std::fs::write(&file, lines).unwrap();
    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&file).unwrap();
    assert_eq!(s.meta.directory, "/repo");
    assert_eq!(s.meta.source.as_deref(), Some("subagent"));
    // The adapter owns the interactivity judgment: a sub-agent thread is not
    // interactive, so the engine will skip it.
    assert!(!adapter.is_interactive(&s));
}

#[test]
fn object_form_thread_source_does_not_fail_line_and_drives_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let day = tmp.path().join("sessions/2026/06/04");
    std::fs::create_dir_all(&day).unwrap();
    let file = day.join("rollout-2026-06-04T10-00-00-threadsrc.jsonl");
    // `thread_source` is parsed loosely like `source`: an object variant must not
    // fail the session_meta line (cwd/git still parse), and a non-interactive
    // thread_source marks the session non-interactive even when `source` is a
    // benign interactive origin.
    let lines = concat!(
        r#"{"type":"session_meta","timestamp":"2026-06-04T10:00:00.000Z","payload":{"id":"threadsrc","cwd":"/repo","source":"cli","thread_source":{"subagent":{"parent":"root"}}}}"#,
        "\n",
        r#"{"type":"event_msg","timestamp":"2026-06-04T10:00:02.000Z","payload":{"type":"user_message","message":"do the thing"}}"#,
        "\n",
    );
    std::fs::write(&file, lines).unwrap();
    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&file).unwrap();
    assert_eq!(s.meta.directory, "/repo");
    // The non-interactive thread_source wins over the interactive `source`.
    assert_eq!(s.meta.source.as_deref(), Some("subagent"));
    assert!(!adapter.is_interactive(&s));
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
        !adapter.parse(&active_file).unwrap().meta.archived,
        "sessions under sessions/ are not archived"
    );
    assert!(
        adapter.parse(&archived_file).unwrap().meta.archived,
        "sessions under archived_sessions/ are archived"
    );
}

#[test]
fn codex_unarchive_command_wraps_session_id() {
    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let s = adapter.parse(&fixture("rollout-2026-06-04T10-00-00-codexsample.jsonl")).unwrap();
    assert_eq!(
        adapter.unarchive_command(&s),
        Some(vec!["codex".into(), "unarchive".into(), s.meta.id.clone()])
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
    use hop::adapters::Adapter;
    use hop::adapters::codex::CodexAdapter;
    let path =
        std::path::Path::new("tests/fixtures/codex/rollout-2026-06-04T10-00-00-codexsample.jsonl");
    let a = CodexAdapter::new(std::path::PathBuf::from("/unused"));
    let s = a.parse(path).unwrap();
    assert_eq!(s.meta.branch.as_deref(), Some("main"));
    assert_eq!(s.meta.repo_url.as_deref(), Some("git@github.com:me/web.git"));
}

#[test]
fn codex_transcript_roles_and_filters_internals() {
    use hop::adapters::Adapter;
    use hop::adapters::codex::CodexAdapter;
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
    let file = tmp.path().join("rollout-2026-06-04T10-00-00-longtitle.jsonl");
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
    assert_eq!(s.meta.title, long_title);
    assert!(s.meta.title.chars().count() > 80);
}

#[test]
fn codex_filters_event_message_tags_and_external_tool_blocks() {
    use hop::core::{Block, Role};

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("rollout-2026-06-04T10-00-00-tagsample.jsonl");
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
    assert_eq!(s.meta.title, "check last commit - top - nested");
    assert_eq!(s.meta.message_count, 2);
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

#[test]
fn paginated_history_uses_response_items_without_duplicates() {
    use hop::core::Role;

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("rollout-2026-07-11T10-00-00-paginated.jsonl");
    let lines = [
        serde_json::json!({
            "type": "session_meta",
            "timestamp": "2026-07-11T10:00:00Z",
            "payload": { "id": "paginated", "cwd": "/w", "history_mode": "paginated" }
        }),
        serde_json::json!({
            "type": "event_msg",
            "timestamp": "2026-07-11T10:00:01Z",
            "payload": { "type": "user_message", "message": "legacy duplicate" }
        }),
        serde_json::json!({
            "type": "response_item",
            "timestamp": "2026-07-11T10:00:02Z",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "<skills_instructions>hidden</skills_instructions>" },
                    { "type": "input_text", "text": "## My request for Codex:\nship" },
                    { "type": "input_image", "image_url": "data:image/png;base64,x" },
                    { "type": "input_text", "text": "the fix" }
                ]
            }
        }),
        serde_json::json!({
            "type": "response_item",
            "timestamp": "2026-07-11T10:00:03Z",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [
                    { "type": "output_text", "text": "Done" },
                    { "type": "output_text", "text": "and tested" }
                ]
            }
        }),
        serde_json::json!({
            "type": "response_item",
            "timestamp": "2026-07-11T10:00:04Z",
            "payload": {
                "type": "message",
                "role": "system",
                "content": [{ "type": "input_text", "text": "internal" }]
            }
        }),
        serde_json::json!({
            "type": "response_item",
            "timestamp": "2026-07-11T10:00:05Z",
            "payload": { "type": "function_call", "name": "exec_command" }
        }),
    ];
    std::fs::write(
        &file,
        lines.iter().map(serde_json::Value::to_string).collect::<Vec<_>>().join("\n"),
    )
    .unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let session = adapter.parse(&file).unwrap();
    let messages = adapter.transcript(&file).unwrap();

    assert_eq!(session.meta.title, "ship the fix");
    assert_eq!(session.meta.message_count, 2);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Agent);
    assert!(session.content.contains("ship\nthe fix"));
    assert!(session.content.contains("Done\nand tested"));
    assert!(!session.content.contains("legacy duplicate"));
    assert!(!session.content.contains("hidden"));
    assert!(!session.content.contains("internal"));
    assert!(!session.content.contains("exec_command"));
}

#[test]
fn history_mode_defaults_and_empty_preferred_source_falls_back() {
    fn write_session(path: &std::path::Path, history_mode: Option<&str>, records: &str) {
        let mode =
            history_mode.map(|value| format!(r#", "history_mode": "{value}""#)).unwrap_or_default();
        std::fs::write(
            path,
            format!(
                r#"{{"type":"session_meta","timestamp":"2026-07-11T10:00:00Z","payload":{{"id":"fallback","cwd":"/w"{mode}}}}}
{records}
"#
            ),
        )
        .unwrap();
    }

    let tmp = tempfile::tempdir().unwrap();
    let legacy = tmp.path().join("rollout-2026-07-11T10-00-00-legacy.jsonl");
    write_session(
        &legacy,
        None,
        concat!(
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"legacy wins"}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"response duplicate"}]}}"#
        ),
    );

    let paginated_fallback =
        tmp.path().join("rollout-2026-07-11T10-00-00-paginated-fallback.jsonl");
    write_session(
        &paginated_fallback,
        Some("paginated"),
        r#"{"type":"event_msg","payload":{"type":"user_message","message":"event fallback"}}"#,
    );

    let legacy_fallback = tmp.path().join("rollout-2026-07-11T10-00-00-legacy-fallback.jsonl");
    write_session(
        &legacy_fallback,
        Some("future-mode"),
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"response fallback"}]}}"#,
    );

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    assert_eq!(adapter.parse(&legacy).unwrap().meta.title, "legacy wins");
    assert_eq!(adapter.parse(&paginated_fallback).unwrap().meta.title, "event fallback");
    assert_eq!(adapter.parse(&legacy_fallback).unwrap().meta.title, "response fallback");
}

#[test]
fn codex_filters_all_injected_context_blocks_and_request_prefix() {
    let tags = [
        "user_instructions",
        "environment_context",
        "apps_instructions",
        "skills_instructions",
        "plugins_instructions",
        "collaboration_mode",
        "multi_agent_mode",
        "realtime_conversation",
        "context_window_guidance",
        "context_window",
        "system-reminder",
    ];
    let mut message = String::new();
    for tag in tags {
        use std::fmt::Write as _;
        writeln!(message, "<{tag}>hidden {tag}</{tag}>").unwrap();
    }
    message.push_str("## My request for Codex:\nkeep this request");

    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("rollout-2026-07-11T10-00-00-context.jsonl");
    let meta = serde_json::json!({
        "type": "session_meta",
        "timestamp": "2026-07-11T10:00:00Z",
        "payload": { "id": "context", "cwd": "/w" }
    });
    let event = serde_json::json!({
        "type": "event_msg",
        "payload": { "type": "user_message", "message": message }
    });
    std::fs::write(&file, format!("{meta}\n{event}\n")).unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let session = adapter.parse(&file).unwrap();
    assert_eq!(session.meta.title, "keep this request");
    assert_eq!(session.content.trim(), "keep this request");
}

#[test]
fn codex_title_skips_review_mode_boilerplate() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("rollout-2026-07-11T10-00-00-review.jsonl");
    let records = [
        serde_json::json!({
            "type": "session_meta",
            "timestamp": "2026-07-11T10:00:00Z",
            "payload": { "id": "review", "cwd": "/w" }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": "## Code review guidelines:\nReview this carefully."
            }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": { "type": "agent_message", "message": "Acknowledged." }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": { "type": "user_message", "message": "Find the regression" }
        }),
    ];
    std::fs::write(
        &file,
        records.iter().map(serde_json::Value::to_string).collect::<Vec<_>>().join("\n"),
    )
    .unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    let session = adapter.parse(&file).unwrap();
    assert_eq!(session.meta.title, "Find the regression");
    assert!(session.content.contains("## Code review guidelines:"));
}

#[test]
fn scans_and_parses_compressed_rollouts_and_prefers_plain_siblings() {
    let tmp = tempfile::tempdir().unwrap();
    let sessions = tmp.path().join("sessions/2026/07/11");
    let archived = tmp.path().join("archived_sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::create_dir_all(&archived).unwrap();

    let raw = concat!(
        r#"{"type":"session_meta","timestamp":"2026-07-11T10:00:00Z","payload":{"id":"compressed","cwd":"/w"}}"#,
        "\n",
        r#"{"type":"event_msg","payload":{"type":"user_message","message":"from compressed"}}"#,
        "\n"
    );
    let compressed_path = archived.join("rollout-2026-07-11T10-00-00-compressed.jsonl.zst");
    let compressed = zstd::stream::encode_all(raw.as_bytes(), 0).unwrap();
    std::fs::write(&compressed_path, compressed).unwrap();

    let sibling_raw = raw.replace("compressed", "sibling");
    let plain_sibling = sessions.join("rollout-2026-07-11T10-00-00-sibling.jsonl");
    let compressed_sibling = sessions.join("rollout-2026-07-11T10-00-00-sibling.jsonl.zst");
    std::fs::write(&plain_sibling, &sibling_raw).unwrap();
    std::fs::write(
        &compressed_sibling,
        zstd::stream::encode_all(sibling_raw.as_bytes(), 0).unwrap(),
    )
    .unwrap();

    let adapter = CodexAdapter::new(tmp.path().to_path_buf());
    let scanned = adapter.scan().unwrap();
    assert_eq!(scanned["compressed"].path, compressed_path);
    assert!(scanned["compressed"].mtime > 0);
    assert_eq!(scanned["sibling"].path, plain_sibling);

    let session = adapter.parse(&scanned["compressed"].path).unwrap();
    assert_eq!(session.meta.id, "compressed");
    assert_eq!(session.meta.title, "from compressed");
    assert!(session.meta.archived);
    assert_eq!(adapter.transcript(&scanned["compressed"].path).unwrap().len(), 1);
}

#[test]
fn corrupt_compressed_rollout_is_a_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("rollout-2026-07-11T10-00-00-corrupt.jsonl.zst");
    std::fs::write(&file, b"not a zstd stream").unwrap();

    let adapter = CodexAdapter::new(PathBuf::from("/unused"));
    assert!(adapter.parse(&file).is_err());
}
