# zed-viper

local Zed extension for the [Viper](https://viper.ethz.ch) verification language (`.vpr`)

- `tree-sitter-viper/`: tree-sitter grammar, written from Silver's `FastParser.scala`; permissive on purpose, it's for highlighting
- `languages/viper/`: Zed language config and queries (highlights, brackets, indents, outline); `tree-sitter-viper/queries/` symlinks here
- `viper-lsp/`: stdio bridge to ViperServer (which only speaks LSP over TCP); verifies on open and save, so errors show inline with hover, plus ViperServer's hover, go to definition, completion, rename
- `src/lib.rs`: tells Zed to launch `viper-lsp` from `PATH` with the worktree's shell env

## install

- `mise run lsp:install` (`cargo install --path viper-lsp`)
- Zed → `zed: install dev extension` → pick this folder

`viper-lsp` reads `VIPER_HOME` (default `~/opt/ViperTools`), `VIPER_BACKEND` (`silicon` or `carbon`, default `silicon`) and `VIPER_JAVA` (default `java` on `PATH`). ViperServer's own log goes to `$TMPDIR/viper-lsp-server.log`, the bridge's to Zed's LSP log (`dev: open language server logs`).

## grammar workflow

- `mise run grammar:test`: corpus tests in `tree-sitter-viper/test/corpus`
- `mise run grammar:coverage`: parse Silver's 1500 test files (expects a checkout at `~/opt/src/silver`, override with `SILVER_TESTS`)
- after a grammar change: commit, `mise run grammar:pin`, commit `extension.toml`, then `zed: rebuild dev extension`

## credits

- [Trzyq0712/tree-sitter-viper](https://github.com/Trzyq0712/tree-sitter-viper) was the starting point for the highlight queries
