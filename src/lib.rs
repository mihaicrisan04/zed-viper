use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

struct ViperExtension;

impl zed::Extension for ViperExtension {
    fn new() -> Self {
        ViperExtension
    }

    // The worktree's shell env carries mise's java, VIPER_HOME and VIPER_BACKEND.
    fn language_server_command(&mut self, _: &LanguageServerId, worktree: &Worktree) -> Result<zed::Command> {
        let command = worktree
            .which("viper-lsp")
            .ok_or("viper-lsp is not on PATH, run `cargo install --path viper-lsp` in the zed-viper repo")?;
        Ok(zed::Command { command, args: vec![], env: worktree.shell_env() })
    }
}

zed::register_extension!(ViperExtension);
