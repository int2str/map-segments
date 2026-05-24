# cargo-map-segments

A Cargo subcommand that visualises how ELF binary sections are laid out across
the memory regions defined in a linker script (`memory.x`).

This is an adaptation of the excellent [`espsegs`](https://github.com/bjoernQ/espsegs)
tool by [@bjoernQ](https://github.com/bjoernQ). While originally written for nRF
targets, it should work equally well for ESP and any other embedded project that
uses a standard GNU `memory.x` linker script.

## Example output

```
.vector_table 00000000     256 FLASH    [▏                                      ]
.text         00000100   41404 FLASH    [███                                    ]
.rodata       0000a2c0   11808 FLASH    [   ▏                                   ]

.data         20000000      80 RAM      [▏                                      ]
.bss          20000050     520 RAM      [▏                                      ]
.uninit       20000258    1024 RAM      [▏                                      ]
```

## Installation

```
cargo install map-segments
```

## Usage

```
cargo map-segments [OPTIONS]
```

Common examples:

```
# Build with cargo and inspect the first executable artifact
cargo map-segments

# Build and inspect a specific target
cargo map-segments --bin my-app
cargo map-segments --example demo

# Build with build/profile options
cargo map-segments --release
cargo map-segments --target thumbv7em-none-eabi --features defmt

# Skip build and inspect an existing ELF directly
cargo map-segments --binary target/thumbv7em-none-eabi/debug/my-firmware

# Provide memory.x explicitly
cargo map-segments --memory-map path/to/memory.x
```

### Options

| Option | Description |
|---|---|
| `-b`, `--binary <BINARY>` | Path to an ELF binary to analyse directly. Skips `cargo build` entirely. |
| `--bin <BIN>` | Binary target to build and analyse. |
| `--example <EXAMPLE>` | Example target to build and analyse. |
| `--release` | Build in release mode (default: debug). |
| `-F`, `--features <FEATURES>` | Space- or comma-separated list of features to activate. |
| `--all-features` | Activate all available features. |
| `--no-default-features` | Do not activate the `default` feature. |
| `--profile <PROFILE>` | Build artifacts with the specified profile. |
| `--target <TARGET>` | Build for the target triple. |
| `-p`, `--package <PACKAGE>` | Package to build. |
| `--manifest-path <MANIFEST_PATH>` | Path to `Cargo.toml`. |
| `-m`, `--memory-map <MEMORY_MAP>` | Path to a `memory.x` linker script. If omitted, it is located automatically via Cargo fingerprint metadata. |
| `-w`, `--width <WIDTH>` | Output width in columns. Defaults to terminal width, or 120 if stdout is not a terminal. |

## Auto-detection of `memory.x`

When building with Cargo, `map-segments` inspects Cargo fingerprint metadata
to locate the `memory.x` copied into the build script output directory
(`OUT_DIR/memory.x`).

```
cargo map-segments --bin my-firmware
```

If auto-detection fails, specify the path explicitly:

```
cargo map-segments --binary target/thumbv7em-none-eabi/debug/my-firmware \
    --memory-map memory.x
```

## `memory.x` format

Standard GNU linker script `MEMORY` blocks are supported, including:

- Hex (`0x...`) and decimal literals
- `K` and `M` size suffixes
- `+` and `-` arithmetic expressions
- `ORIGIN(NAME)` and `LENGTH(NAME)` cross-region references
- `/* */` block comments and `//` line comments

## License

GPL-3.0-only — see [LICENSE](LICENSE).
