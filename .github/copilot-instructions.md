# Copilot Instructions for Launchpad

## Project Overview

**Launchpad** is a Rust application that plays sounds/music from a MIDI Launchpad controller (tested with Launchpad Mini MK3, MiniMK2, and X models). It provides a pagination system allowing the same keybind to trigger different sounds across multiple pages and bookmarks, with support for up to 64 sounds per page, unlimited pages, and 8 bookmarks.

- **Repository Size**: ~800KB (source only, ~660MB with build artifacts)
- **Lines of Code**: ~758 lines of Rust
- **Language**: Rust (requires nightly toolchain for `let_chains` feature)
- **License**: GNU General Public License v3
- **Dependencies**: MIDI I/O (midir), audio playback (rodio, cpal), config management (config-file, serde_yaml)

## Build Requirements

### System Dependencies

**ALWAYS install these system packages BEFORE attempting to build:**

```bash
sudo apt-get update
sudo apt-get install -y libasound2-dev pkg-config
```

**Critical**: Without `libasound2-dev`, the build will fail with a `pkg-config` error for the `alsa` library. This is NOT optional.

### Rust Toolchain

**ALWAYS use Rust nightly toolchain**. The project uses the unstable `let_chains` feature and will NOT compile with stable Rust.

```bash
# Install nightly toolchain
rustup toolchain install nightly

# Set nightly as default for this project directory
rustup override set nightly

# Verify nightly is active
rustup show  # Should show nightly as active
```

### Build Commands

**Build Sequence (ALWAYS run in this order):**

1. **Development build** (~15 seconds from clean state):
   ```bash
   cargo build
   ```

2. **Release build** (~34 seconds from clean state):
   ```bash
   cargo build --release
   ```

3. **Quick check without building binary** (~8 seconds):
   ```bash
   cargo check
   ```

**Incremental builds** after code changes take ~3-8 seconds depending on the scope of changes.

## Code Quality Tools

### Formatting

The codebase currently has formatting inconsistencies. Before running `cargo fmt`:

```bash
# Add rustfmt component if not present
rustup component add rustfmt --toolchain nightly

# Check formatting (will show many diffs in referential.rs and midi.rs)
cargo fmt --check

# Auto-fix formatting
cargo fmt
```

**Known formatting issues**: The project uses 2-space indentation in some files and inconsistent import ordering.

### Linting

```bash
# Add clippy component if not present
rustup component add clippy --toolchain nightly

# Run clippy (currently shows 10 warnings)
cargo clippy
```

**Known clippy warnings**:
- Collapsible if statements (can use let_chains)
- Needless borrows in function calls
- Use of deprecated Into trait (should use From)

**Note**: These warnings are not build-breaking but should be fixed when modifying nearby code.

### Testing

```bash
cargo test
```

**Current state**: The project has NO unit tests. Do not expect any tests to run. Adding tests is acceptable but not required for most changes.

## Project Structure

### Root Directory Files

```
.
├── Cargo.toml           # Project manifest and dependencies
├── Cargo.lock           # Dependency lock file
├── config.yaml          # Runtime configuration (MIDI devices, bookmarks)
├── README.md            # User documentation
├── LICENSE              # GPL-3.0 license
├── colors.png           # Launchpad color reference image
├── pew.mp3              # Example sound file
└── src/                 # Source code directory
```

### Source Code Structure

```
src/
├── main.rs              # Application entry point (~250 lines)
├── config.rs            # Configuration struct and parsing (~82 lines)
├── audio.rs             # Audio device selection and playback (~33 lines)
├── midi.rs              # MIDI device handling and grid management (~134 lines)
├── referential.rs       # Page and note management (~175 lines)
└── launchpad/
    └── mod.rs           # Launchpad model definitions (~91 lines)
```

### Key Architecture Components

1. **main.rs**: Main event loop that:
   - Loads configuration from `config.yaml`
   - Sets up MIDI input/output connections
   - Sets up audio output devices (default + virtual)
   - Initializes the referential (page system) from the `pages/` folder
   - Processes MIDI note events and plays corresponding sounds
   - Manages hold-to-play modes (Normal, Stop, Pause)

2. **config.rs**: Defines `Config` struct with:
   - MIDI device names (input/output)
   - Audio device names (output/virtual)
   - 7 bookmark paths (bookmark_1 through bookmark_7)
   - Hold mode setting (Normal/Stop/Pause)

3. **audio.rs**: Handles:
   - Enumerating available audio output devices
   - Creating output streams for specific devices
   - Playing audio files with volume control

4. **midi.rs**: Manages:
   - MIDI device enumeration and connection
   - Grid refresh and clearing
   - Special action handlers (stop, end session)
   - Bookmark and page navigation button colors

5. **referential.rs**: Implements:
   - `Note`: A sound file mapping (note_id, file path, color)
   - `Page`: Collection of up to 64 notes
   - `Referential`: Page management (current page, bookmarks, navigation)

6. **launchpad/mod.rs**: Defines note mappings for different Launchpad models (MiniMk2, MiniMk3, X)

### Configuration Files

- **config.yaml**: Runtime configuration at project root (NOT in src/)
  - Required before running the application
  - Specifies MIDI device names (find via debug_mode: true)
  - Specifies audio output device names
  - Defines bookmark folder names

- **Cargo.toml**: Build configuration
  - Edition: 2024 (latest Rust edition)
  - Notable: Uses deprecated `serde_yaml` 0.9.34 (shows warning)

### Pages System

The application looks for a `pages/` directory (configurable via bookmarks). Each file in this directory becomes a page:

```
pages/
├── 0            # Page 0
├── 1            # Page 1
└── 2            # Page 2
```

**Page file format** (semicolon-separated):
```
11;pew.mp3;13
```
- Column 1: Note ID (11-89, excluding 19,29,39,49,59,69,79)
- Column 2: Sound file path (absolute or relative)
- Column 3: Color code (0-127)

## Common Issues and Workarounds

### Build Failures

1. **alsa-sys build failure**: Install `libasound2-dev` (see System Dependencies)
2. **"let_chains" feature error**: Switch to nightly toolchain (see Rust Toolchain)
3. **"cargo-fmt not installed"**: Run `rustup component add rustfmt --toolchain nightly`
4. **"cargo-clippy not installed"**: Run `rustup component add clippy --toolchain nightly`

### Runtime Issues

The application will fail at runtime if:
- `config.yaml` is missing or malformed
- MIDI devices specified in config are not connected
- Audio devices specified in config don't exist
- `pages/` directory is empty

**Debug mode**: Set `debug_mode: true` in `config.yaml` to see available MIDI and audio devices.

## Development Workflow

### Making Code Changes

1. **ALWAYS ensure nightly toolchain is active**: `rustup show`
2. **Make your changes** to source files
3. **Quick validation**: `cargo check` (~3-8 seconds)
4. **Full build**: `cargo build` (~3-8 seconds incremental)
5. **Format code**: `cargo fmt` (optional but recommended)
6. **Lint code**: `cargo clippy` (fix warnings in modified code)

### Binary Locations

- Debug: `target/debug/launch-soundpad` (~52 MB with debug symbols)
- Release: `target/release/launch-soundpad` (optimized binary)

### Testing Changes

Since there are no unit tests, manual testing requires:
1. A physical Launchpad MIDI controller
2. Configured `config.yaml` with valid device names
3. A `pages/` directory with valid page files
4. Audio output devices (regular + virtual like VB-Audio Cable)

**Note**: Most code changes can be validated through `cargo check` and `cargo build` without runtime testing.

## GitHub Actions / CI

**Current state**: No GitHub Actions workflows or CI pipelines are configured. All validation must be done locally.

## TODOs in Codebase

The codebase contains these known TODOs (do NOT fix unless specifically asked):
- `src/main.rs:39`: Load config file, generate default if not found
- `src/config.rs:27`: Create an init function for Config

## Important Notes

- **Trust these instructions**: They are validated against the actual codebase. Only search for additional information if something here is incomplete or incorrect.
- **Nightly toolchain is mandatory**: Do NOT attempt builds with stable Rust.
- **System dependencies are required**: Install libasound2-dev before any build attempts.
- **No tests exist**: Do not expect `cargo test` to run any tests.
- **Format before committing**: Run `cargo fmt` to maintain consistency.
- **The project uses GPL-3.0**: Any modifications must comply with this license.
