# Jedit

**Jedit** is a command-line tool to view and edit large JSON file directly within your terminal.

![screenshot](docs/screenshot.png)

## Installation

To install Jedit, ensure you have [Rust](https://www.rust-lang.org/tools/install) installed, then run:

```bash
git clone https://github.com/aguss787/jedit.git
cd jedit
cargo build --release
```

## Usage

```bash
jedit --help
```

## Keybind

| Key               | Action                 |
| ----------------- | ---------------------- |
| q                 | Exit                   |
| k / Up            | Up                     |
| j / Down          | Down                   |
| l / Enter / Space | Expand                 |
| Ctrl + u          | Up 10                  |
| Ctrl + d          | Down 10                |
| h                 | Close                  |
| p                 | Toggle preview         |
| e                 | Edit value             |
| w                 | Save                   |
| K                 | Preview up             |
| J                 | Preview down           |
| Ctrl + U          | Preview up 5           |
| Ctrl + D          | Preview down 5         |
| H                 | Preview left           |
| L                 | Preview right          |
| Ctrl + Left       | Preview window bigger  |
| Ctrl + Right      | Preview window smaller |

## Missing feature

- [ ] Custom keybind
- [ ] Search
- [ ] Inline key operation
  - [ ] Add new key
  - [ ] Delete key
  - [ ] Rename key
- [ ] Prettier error message
