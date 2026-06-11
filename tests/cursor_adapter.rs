use hop::adapters::cursor::CursorAdapter;
use hop::adapters::Adapter;
use hop::core::AgentId;
use std::path::PathBuf;

const UUID: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cursor")
        .join(name)
}

/// Build the standard tree under `root`:
///   root/<slug>/agent-transcripts/<uuid>/<uuid>.jsonl  (canonical)
///   root/<slug>/agent-transcripts/<uuid>/hook-sidecar.jsonl  (sidecar)
fn setup_tree(root: &std::path::Path) -> PathBuf {
    let slug = "myproject";
    let conv_dir = root.join(slug).join("agent-transcripts").join(UUID);
    std::fs::create_dir_all(&conv_dir).unwrap();

    let canonical = conv_dir.join(format!("{UUID}.jsonl"));
    std::fs::copy(fixture("sample.jsonl"), &canonical).unwrap();
    std::fs::copy(
        fixture("hook-sidecar.jsonl"),
        conv_dir.join("hook-sidecar.jsonl"),
    )
    .unwrap();

    canonical
}

// ── Test 1: parse / extract ───────────────────────────────────────────────────

#[test]
fn parses_transcript_and_strips_noise() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = setup_tree(tmp.path());

    let adapter = CursorAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&canonical).unwrap();

    assert_eq!(s.agent, AgentId::Cursor);
    assert_eq!(s.id, UUID);

    // Title from first <user_query>
    assert_eq!(s.title, "fix the cursor session bug");

    // No worker.log → directory falls back to ""
    assert_eq!(s.directory, "");

    // 4 messages: 2 user + 2 assistant (each has a text block)
    assert_eq!(s.message_count, 4);

    // Content contains user prose and assistant text
    assert!(s.content.contains("fix the cursor session bug"));
    assert!(s.content.contains("I'll fix the bug."));
    assert!(s.content.contains("Done."));

    // tool_use blocks and XML tags must be stripped
    assert!(!s.content.contains("tool_use"));
    assert!(!s.content.contains("read_file"));
    assert!(!s.content.contains("write_file"));
    assert!(!s.content.contains("<user_query>"));
    assert!(!s.content.contains("<file_path>"));

    // timestamp from file mtime (non-zero)
    assert!(s.timestamp > 0);

    // No store.db → yolo defaults to false
    assert!(!s.yolo);
}

// ── Test 2: scan skips hook sidecars ─────────────────────────────────────────

#[test]
fn scan_skips_hook_sidecars() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = setup_tree(tmp.path());

    let adapter = CursorAdapter::new(tmp.path().to_path_buf());
    let scanned = adapter.scan().unwrap();

    // Exactly one entry for the UUID
    assert_eq!(
        scanned.len(),
        1,
        "expected 1 session; got keys: {:?}",
        scanned.keys().collect::<Vec<_>>()
    );
    assert!(
        scanned.contains_key(UUID),
        "scan should key by UUID; got: {:?}",
        scanned.keys().collect::<Vec<_>>()
    );
    assert_eq!(scanned[UUID].path, canonical);
}

// ── Test 3: store.db enrichment ───────────────────────────────────────────────

#[test]
fn enriches_from_store_db() {
    let tmp = tempfile::tempdir().unwrap();
    let slug = "myproject";

    // The adapter derives: chats_root = self.root.parent().join("chats")
    // So we use root = tmp/projects/ so that chats_root = tmp/chats/.
    let projects_dir = tmp.path().join("projects");

    // Transcript tree under projects/<slug>/agent-transcripts/<uuid>/
    let conv_dir = projects_dir.join(slug).join("agent-transcripts").join(UUID);
    std::fs::create_dir_all(&conv_dir).unwrap();
    let canonical = conv_dir.join(format!("{UUID}.jsonl"));
    std::fs::copy(fixture("sample.jsonl"), &canonical).unwrap();

    // worker.log — points to a real, canonicalisable path
    let project_dir = projects_dir.join(slug);
    let workspace_path = project_dir.to_str().unwrap().to_string();
    std::fs::write(
        project_dir.join("worker.log"),
        format!("info workspacePath={workspace_path}\n"),
    )
    .unwrap();

    // chats/<md5(realpath(workspace))>/<uuid>/store.db
    let real = std::fs::canonicalize(&workspace_path).unwrap();
    let hash = format!("{:x}", md5::compute(real.to_string_lossy().as_bytes()));
    let db_dir = tmp.path().join("chats").join(&hash).join(UUID);
    std::fs::create_dir_all(&db_dir).unwrap();

    let conn = rusqlite::Connection::open(db_dir.join("store.db")).unwrap();
    conn.execute("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT)", [])
        .unwrap();
    let json = r#"{"name":"My Chat","createdAt":1700000000000,"isRunEverything":true}"#;
    conn.execute(
        "INSERT INTO meta VALUES ('0', ?1)",
        [&hex::encode(json.as_bytes())],
    )
    .unwrap();
    drop(conn);

    let adapter = CursorAdapter::new(projects_dir);
    let s = adapter.parse(&canonical).unwrap();

    assert_eq!(s.title, "My Chat");
    assert_eq!(s.timestamp, 1_700_000_000);
    assert!(s.yolo);
}

// ── Test 4: cwd from worker.log ───────────────────────────────────────────────

#[test]
fn reads_cwd_from_worker_log() {
    let tmp = tempfile::tempdir().unwrap();
    let slug = "myproject";
    let conv_dir = tmp.path().join(slug).join("agent-transcripts").join(UUID);
    std::fs::create_dir_all(&conv_dir).unwrap();
    let canonical = conv_dir.join(format!("{UUID}.jsonl"));
    std::fs::copy(fixture("sample.jsonl"), &canonical).unwrap();

    // Write worker.log with a path that contains a space
    let project_dir = tmp.path().join(slug);
    std::fs::write(
        project_dir.join("worker.log"),
        "info workspacePath=/tmp/my workspace\n",
    )
    .unwrap();

    let adapter = CursorAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&canonical).unwrap();

    assert_eq!(s.directory, "/tmp/my workspace");
}

// ── Test 5: resume_command ────────────────────────────────────────────────────

#[test]
fn resume_command_non_yolo_and_yolo() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = setup_tree(tmp.path());

    let adapter = CursorAdapter::new(tmp.path().to_path_buf());
    let s = adapter.parse(&canonical).unwrap();

    let non_yolo = adapter.resume_command(&s, false);
    assert_eq!(non_yolo, vec!["cursor-agent", "--resume", UUID]);

    let yolo_cmd = adapter.resume_command(&s, true);
    assert_eq!(yolo_cmd, vec!["cursor-agent", "--force", "--resume", UUID]);
}

// ── Test 6: AgentId round-trip ────────────────────────────────────────────────

#[test]
fn agent_id_cursor_round_trip() {
    assert_eq!(AgentId::from_slug("cursor"), Some(AgentId::Cursor));
    assert_eq!(AgentId::Cursor.slug(), "cursor");
    assert_eq!(AgentId::Cursor.badge(), "CURSOR");
}
