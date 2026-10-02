# Viper for Zed

[Viper](https://www.pm.inf.ethz.ch/research/viper.html) verification language (`.vpr`) support for [Zed](https://zed.dev).

## Features

- syntax highlighting, bracket matching, indentation and outline (`cmd-shift-o`), from a tree-sitter grammar written for this extension
- verification on open and save through ViperServer: failing assertions are underlined where they fail, with the message on hover and in the diagnostics panel (`cmd-shift-m`)
- verification progress and result in the status bar
- ViperServer's editor features: hover, go to definition, find references, rename, completion, signature help, document symbols

## Requirements

Java 11 or newer, on `PATH` or set through `java_path` below.

Everything else is downloaded on first use: ViperTools (ViperServer, Silicon, Carbon, Z3 and Boogie, about 120 MB) from the [Viper IDE releases](https://github.com/viperproject/viper-ide/releases), and the `viper-lsp` bridge from [this repo's releases](https://github.com/mihaicrisan04/zed-viper/releases).

## Configuration

All settings are optional. In your Zed `settings.json`:

```jsonc
{
  "lsp": {
    "viper-lsp": {
      "settings": {
        // "silicon" (symbolic execution, default) or "carbon" (verification condition generation)
        "backend": "silicon",
        // use your own Java instead of the one on PATH
        "java_path": "/path/to/bin/java",
        // use an existing ViperTools folder instead of downloading one; `VIPER_HOME` works too
        "viper_tools_path": "/path/to/ViperTools"
      },
      // use your own build of viper-lsp; one on PATH is picked up automatically
      "binary": { "path": "/path/to/viper-lsp" }
    }
  }
}
```

Run `editor: restart language server` after changing them.

To also show error messages at the end of the line, as well as on hover:

```json
{ "diagnostics": { "inline": { "enabled": true } } }
```

## How it works

ViperServer is a language server, but it listens on a TCP port instead of stdio, and only verifies when its client sends a custom `Verify` notification, which the VS Code extension does on save. `viper-lsp` is a small bridge that:

- starts ViperServer and connects to the port it reports
- passes LSP messages through in both directions
- sends `Verify` when a file is opened or saved
- answers the custom requests only the VS Code extension knows (`GetViperFileEndings`, `SetupProject`)
- turns ViperServer's custom `StateChange` notifications into standard `$/progress`

ViperServer's own log is written to `viper-lsp-server.log` in the system temp directory. The bridge logs to Zed's language server log (`dev: open language server logs`).

## Limitations

- verification runs on open and save, not while typing; parse and type errors do update while typing
- ViperServer only keeps errors for the file it verified last, so switching files clears the previous file's errors until it is saved again
- ViperTools has no Linux ARM build: install Viper yourself and set `viper_tools_path`
- so far only tested on macOS (Apple Silicon); Linux and Windows builds are produced but untested

## Development

Tools are managed with [mise](https://mise.jdx.dev); `mise tasks` lists everything:

- `mise run grammar:test`: tree-sitter corpus tests in `tree-sitter-viper/test/corpus`
- `mise run grammar:coverage`: parse Silver's ~1500 test programs (needs a [silver](https://github.com/viperproject/silver) checkout, `SILVER_TESTS` points at its `src/test/resources`)
- `mise run grammar:pin`: after committing a grammar change, point `extension.toml` at that commit
- `mise run lsp:test`, `mise run lsp:install`: test the bridge, install it into `~/.cargo/bin`

To try local changes, run `zed: install dev extension` and pick this folder. A `viper-lsp` on `PATH` takes precedence over the released one.

Layout:

- `tree-sitter-viper/`: the grammar, following Silver's `FastParser.scala`; permissive on purpose, since it is used for highlighting and not for rejecting programs
- `languages/viper/`: Zed language config and queries
- `viper-lsp/`: the bridge
- `src/lib.rs`: the extension, which finds Java and downloads ViperTools and the bridge

Releases: bump the version in `extension.toml`, `Cargo.toml` and `viper-lsp/Cargo.toml`, then push a `vX.Y.Z` tag. The release workflow builds `viper-lsp` for macOS, Linux and Windows and attaches it to the GitHub release.

## Credits

- [Viper](https://github.com/viperproject) and ViperServer are developed by the Programming Methodology group at ETH Zurich
- the highlight queries started from [Trzyq0712/tree-sitter-viper](https://github.com/Trzyq0712/tree-sitter-viper)

## License

MIT
