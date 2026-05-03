# NeoFM (nfm)
<img width="2160" height="1440" alt="nfm" src="https://github.com/user-attachments/assets/a958c539-a9f4-4063-9171-3047be98cab5" />


NeoFM is a modern, fast, and feature-rich TUI (Text User Interface) file manager and text editor built in Rust. It runs seamlessly on both Linux and Windows.

## Features

- **TUI File Manager**: Fast, responsive, keyboard-driven navigation.
- **Built-in Editor**: Edit text files right inside the app, with multi-tab support.
- **Full-screen Tabs**: Open multiple directories and files simultaneously in separate tabs.
- **"This PC" Overview**: Easily access drives and mount points.
- **Configurable via TOML**: Customizable themes, editor settings, and file browser behaviors.
- **Rich Action Menu**: Zip/unzip, open terminals, bulk actions via a handy context menu (`o`).

## Keys

### File Browser
- `Arrow Keys`: Navigate items (Left to go up, Right to enter directory).
- `Enter`: Open file in editor / enter directory.
- `Tab`: Switch tabs.
- `Ctrl+T`: New tab.
- `Ctrl+W`: Close tab.
- `Space`: Select/deselect for bulk actions.
- `c` / `Ctrl+C`: Copy selected.
- `v` / `Ctrl+V`: Paste.
- `Del` / `D`: Delete (with confirmation).
- `n` / `m`: New file / New folder.
- `r`: Rename.
- `o`: Open Action Menu.
- `q` / `Esc`: Quit / Cancel.

### Editor
- `Arrow Keys`: Move cursor.
- `Shift+Arrow`: Jump to start/end of line or file.
- `Ctrl+S`: Save.
- `Ctrl+X`: Clear all text.
- `Esc`: Close editor and return.

## Building from source

### Prerequisites
- Rust and Cargo (`rustup default stable`)

### Linux & Windows
```sh
cargo build --release
```
The binary will be located in `target/release/nfm`.

## Configuration

A default config is generated on the first run at:
- **Linux**: `~/.config/com.NeoFM.nfm/config.toml`
- **Windows**: `C:\Users\Username\AppData\Roaming\NeoFM\nfm\config\config.toml`

See the `config.example.toml` in the repository for all available options.

## License
MIT
