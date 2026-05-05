# NeoFM (nfm)

<img width="1730" height="1065" alt="nfm0" src="https://github.com/user-attachments/assets/88cb3856-0c4e-4006-94c2-d50a7018184c" />
<img width="1745" height="1071" alt="nfm1" src="https://github.com/user-attachments/assets/7bbbe3f8-b8cc-4b3a-af32-128a46a57f9b" />
<img width="1742" height="1077" alt="nfm2" src="https://github.com/user-attachments/assets/02b5d44a-53f9-4175-b224-80bd791a43bc" />


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
