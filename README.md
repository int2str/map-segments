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
cargo install cargo-map-segments
```

## Usage

```
cargo map-segments <ELF_BINARY> [OPTIONS]
```

### Arguments

| Argument       | Description                          |
|----------------|--------------------------------------|
| `<ELF_BINARY>` | Path to the ELF binary to inspect    |

### Options

| Option                      | Description                                                                                                                                  |
|-----------------------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| `-m`, `--memory-map <PATH>` | Path to a `memory.x` linker script. If omitted, the sibling `<binary>.d` dependency file is parsed to locate `memory.x` automatically.       |
| `-w`, `--width <COLS>`      | Output width in columns. Defaults to the current terminal width, or 120 if stdout is not a terminal.                                         |

## Auto-detection of `memory.x`

When building with Cargo, a `<binary>.d` dependency file is generated alongside
the ELF binary. `cargo-map-segments` parses this file to automatically locate
the `memory.x` that was used to link the binary — no manual path required.

```
cargo map-segments target/thumbv7em-none-eabi/debug/my-firmware
```

If auto-detection fails, specify the path explicitly:

```
cargo map-segments target/thumbv7em-none-eabi/debug/my-firmware \
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