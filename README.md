# DTXManiaRS

DTXManiaRS is a modern, drums-first DTX rhythm-game client. It supports
keyboard and electronic-drum play, guided calibration, practice loops, results
analysis, and DTXMania-compatible score files.

## Install and launch

The workspace requires Rust 1.95 or newer. The desktop build also needs the
native development libraries used by ALSA, udev, the Linux window stack,
libclang, and FFmpeg. Exact package names vary by distribution. The verified
Linux build has ALSA/udev, X11/Wayland, Clang/libclang, and FFmpeg development
packages available.

From the repository root:

```sh
cargo install --path app/dtxmaniars-desktop --bin dtxmaniars --locked
dtxmaniars
```

The first install may need network access for the Cargo registry and the two
pinned Git dependencies. Cargo installs to `~/.cargo/bin` by default. For a
window instead of borderless fullscreen:

```sh
DTXMANIARS_WINDOWED=1 dtxmaniars
```

For a local release binary without installing:

```sh
cargo build --release -p dtxmaniars-desktop
./target/release/dtxmaniars
```

## Add songs and start playing

By default the game scans `$XDG_CONFIG_HOME/dtxmaniars/`, or
`$HOME/.config/dtxmaniars/` when `XDG_CONFIG_HOME` is unset. Put each song in a
subdirectory there, or point the scanner at another root:

```sh
DTX_SONG_DIR=/absolute/path/to/songs dtxmaniars
```

On Song Select, press `F6` to import ZIP or 7z archives, or drag archives into
the window. RAR is identified but must be extracted manually. Press `F5` to
rescan. DTX, GDA, and G2D drum charts are supported; see
[Compatibility](docs/compatibility.md) for the precise chart, encoding, audio,
visual, and recovery contract.

Keyboard navigation uses arrows, `Enter`, and `Esc`. `Shift+Enter` starts
Practice. From the title, `F1` opens settings and `F2` opens the layout editor.
The Controls tab manages keyboard and MIDI profiles, device selection,
velocity threshold, lane bindings, and optional Pause/Restart system bindings.
The Gameplay tab contains input offset and the guided 120 BPM calibration.

Normal runs at `1.00x` with Standard fail rules can save records. Practice,
No Fail, and modified-speed runs are clearly labeled and do not update normal
records. Results identifies weak lanes/sections from normal-play timing and can
open Practice with the recommended loop already selected.

## Data and recovery

Settings and profiles live under the same XDG configuration directory as the
default song root. Native score history defaults to `scores.json` in the
directory from which the game was launched; set `DTX_SCORES_PATH` to an
absolute path when a stable location is important. Qualifying results also
update `<chart>.score.ini` next to writable chart files.

Back up the XDG `dtxmaniars` directory, `scores.json` (or the configured score
path), and chart-side `.score.ini` files. If startup reports an invalid config,
move that file aside and relaunch to use defaults. Do not delete the song root
as a general reset: it may contain the only copy of imported songs. See
[Data and persistence](docs/data-and-persistence.md) for every path, schema,
write guarantee, and safe reset procedure.

## Contribute

Use the smallest relevant package test while editing, then run the local gates:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --lib
cargo test -p docs-check
cargo run -p docs-check
```

Read [Contributing](docs/contributing.md) and the affected crate's `AGENTS.md`
before changing behavior. The vendored `references/` tree is read-only.

## Documentation

- [Roadmap](docs/roadmap.md)
- [Player guide](docs/player-guide.md)
- [Compatibility](docs/compatibility.md)
- [Data and persistence](docs/data-and-persistence.md)
- [Contributing](docs/contributing.md)
- [Decision records](docs/decisions/README.md)
