## 1. Restructure config module

- [x] 1.1 Move `src/config.rs` to `src/config/mod.rs` (no logic changes)
- [x] 1.2 Add `Serialize` derive to `Config`, `DisplayConfig`, `ColumnsConfig`, `LauncherConfig`
- [x] 1.3 Add `config_template()` function returning the commented scaffold string
- [x] 1.4 Add `config_path()` function returning the platform-resolved config file path
- [x] 1.5 Add test: template with comment prefixes stripped parses via `Config::from_toml_str`

## 2. CLI and command handlers

- [x] 2.1 Add `ConfigAction` enum (`Init`, `Edit`, `Show`, `Path`) and `Command::Config` variant to `cli.rs`
- [x] 2.2 Create `src/config/commands.rs` with handler functions: `cmd_init`, `cmd_edit`, `cmd_show`, `cmd_path`
- [x] 2.3 Wire `Command::Config` dispatch in `main.rs`

## 3. Tests

- [x] 3.1 Test `cmd_init` creates template file and no-ops when file exists
- [x] 3.2 Test `cmd_show` outputs valid TOML matching effective config
