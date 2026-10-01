# zed-viper

local Zed extension for the [Viper](https://viper.ethz.ch) verification language (`.vpr`)

- `tree-sitter-viper/`: tree-sitter grammar, written from Silver's `FastParser.scala`; permissive on purpose, it's for highlighting
- `languages/viper/`: Zed language config and queries (highlights, brackets, indents, outline); `tree-sitter-viper/queries/` symlinks here

## install

Zed → `zed: install dev extension` → pick this folder

## grammar workflow

- `mise run grammar:test`: corpus tests in `tree-sitter-viper/test/corpus`
- `mise run grammar:coverage`: parse Silver's 1500 test files (expects a checkout at `~/opt/src/silver`, override with `SILVER_TESTS`)
- after a grammar change: commit, `mise run grammar:pin`, commit `extension.toml`, then `zed: rebuild dev extension`

## credits

- [Trzyq0712/tree-sitter-viper](https://github.com/Trzyq0712/tree-sitter-viper) was the starting point for the highlight queries
