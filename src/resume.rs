use anyhow::{bail, Result};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// chdir to `directory`, then exec-replace this process with `argv`.
/// On success this never returns. Returns Err only if exec/setup fails.
pub fn exec_resume(directory: &str, argv: &[String]) -> Result<std::convert::Infallible> {
    if argv.is_empty() {
        bail!("cannot resume: empty command");
    }
    if !directory.is_empty() {
        // best-effort chdir; a vanished dir shouldn't block resume
        let _ = std::env::set_current_dir(directory);
    }
    let err = Command::new(&argv[0]).args(&argv[1..]).exec();
    bail!("failed to exec {}: {err}", argv[0]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_argv_is_rejected() {
        let err = exec_resume("/tmp", &[]).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }
}
