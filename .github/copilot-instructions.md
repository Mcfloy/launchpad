# Copilot Instructions for Launchpad

## Project Overview

**Launchpad** is a Rust application for playing sounds/music from a MIDI Launchpad controller (Mini MK3, MiniMK2, X). It provides pagination allowing the same key to trigger different sounds across multiple pages and bookmarks: up to 64 sounds/page, unlimited pages, 8 bookmarks.

- **Size**: ~800KB source, ~758 lines of Rust
- **Language**: Rust nightly (requires `let_chains` feature)
- **License**: GPL-3.0
- **Key Dependencies**: midir (MIDI), rodio/cpal (audio), config-file/serde_yaml (config)

## Build Requirements

### System Dependencies (CRITICAL - Install First)

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev pkg-config
```

**Without `libasound2-dev`, builds WILL fail** with alsa-sys pkg-config errors.

### Rust Toolchain (MUST Use Nightly)

```bash
rustup toolchain install nightly
rustup override set nightly
rustup show  # Verify nightly is active
```

**Stable Rust will NOT compile** this project due to unstable `let_chains` feature.

### Build Commands

```bash
cargo check         # Quick validation (~8s)
cargo build         # Debug build (~15s clean, ~3-8s incremental)
cargo build --release  # Release build (~34s clean)
cargo fmt           # Format code (requires: rustup component add rustfmt --toolchain nightly)
cargo clippy        # Lint code (requires: rustup component add clippy --toolchain nightly)
cargo test          # No tests exist - will run 0 tests
```

**Known Issues**:
- Formatting inconsistencies exist (2-space indentation, import ordering)
- Clippy shows 10 warnings (collapsible ifs, needless borrows, deprecated Into)
- serde_yaml 0.9.34 shows deprecation warning (expected, not critical)

## Project Structure

```
.
├── Cargo.toml           # Manifest (edition 2024)
├── config.yaml          # Runtime config (MIDI/audio devices, bookmarks)
├── src/
│   ├── main.rs          # Entry point: event loop, MIDI/audio setup (~250 lines)
│   ├── config.rs        # Config struct (MIDI devices, bookmarks, hold mode)
│   ├── audio.rs         # Audio device selection and playback
│   ├── midi.rs          # MIDI handling, grid management, actions
│   ├── referential.rs   # Page/note management (Note, Page, Referential)
│   └── launchpad/mod.rs # Model-specific note mappings
└── pages/               # Sound page files (created at runtime)
```

### Key Components

**main.rs**: Loads config → sets up MIDI I/O → sets up audio (default + virtual) → initializes referential from `pages/` → processes MIDI events → plays sounds → manages hold modes (Normal/Stop/Pause)

**config.rs**: MIDI devices, audio devices, 7 bookmarks, hold mode setting

**referential.rs**: 
- `Note`: sound mapping (note_id, path, color)
- `Page`: collection of notes
- `Referential`: page management and navigation

**Pages format** (semicolon-separated):
```
11;pew.mp3;13
```
Note ID (11-89, skip 19,29,39,49,59,69,79) ; File path ; Color (0-127)

## Common Issues & Solutions

| Issue | Solution |
|-------|----------|
| alsa-sys build failure | Install `libasound2-dev` |
| let_chains feature error | Use nightly: `rustup override set nightly` |
| cargo-fmt not installed | `rustup component add rustfmt --toolchain nightly` |
| cargo-clippy not installed | `rustup component add clippy --toolchain nightly` |
| Runtime failure | Check config.yaml exists, MIDI devices connected, pages/ has files |

**Debug Mode**: Set `debug_mode: true` in config.yaml to list available MIDI/audio devices.

## Development Workflow

1. Ensure nightly: `rustup show`
2. Make changes
3. Validate: `cargo check` (fast)
4. Build: `cargo build`
5. Format: `cargo fmt` (recommended)
6. Lint: `cargo clippy` (fix warnings in modified code)

**Binary locations**:
- Debug: `target/debug/launch-soundpad` (~52 MB with symbols)
- Release: `target/release/launch-soundpad` (optimized)

**Testing**: No unit tests exist. Manual testing requires physical Launchpad, configured config.yaml, pages/, and audio devices.

## Configuration

**config.yaml** (project root, required for runtime):
```yaml
midi_in_device: MIDIIN2 (LPMiniMK3 MIDI)
midi_out_device: MIDIOUT2 (LPMiniMK3 MIDI)
output_device: 
virtual_device: CABLE Input (VB-Audio Virtual Cable)
bookmark_1: pages
bookmark_2: 
# ... bookmark_7
debug_mode: true
hold_to: Normal  # or Stop, Pause
```

## CI/CD

**No GitHub Actions or CI configured.** All validation is local only.

## Known TODOs (Don't Fix Unless Asked)

- `src/main.rs:39`: Auto-generate default config if missing
- `src/config.rs:27`: Create Config init function

## Important Reminders

✓ **Trust these instructions** - validated against actual codebase  
✓ **Nightly is mandatory** - stable will fail  
✓ **Install libasound2-dev first** - prevents build failures  
✓ **No tests exist** - cargo test runs 0 tests  
✓ **GPL-3.0 license** - modifications must comply  
✓ **Format before commit** - run `cargo fmt`
