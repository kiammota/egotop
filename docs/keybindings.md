# Keybindings

Egotop routes keyboard and mouse events by active application context so inputs never conflict between normal viewing, command-line editing, overlays, and cursor startup mode.

> **Known limitation:** Keybindings are currently built into Egotop (`src/app.rs`). Custom keybinding remapping via a TOML configuration file is not implemented in this version.

## Contexts

### 1. Normal Dashboard Mode

| Input | Action |
|---|---|
| `:` | Open the Neovim-style command line and autocomplete popup |
| `q`, `Esc`, `Ctrl+C` | Close Help modal if open, otherwise quit and restore terminal |
| `h` or `?` | Toggle the Help overlay |
| `F2` or `g` | Open the interactive visual Customizer panel |
| `r` | Refresh system metrics immediately |
| `p` | Pause or resume background and startup animations |
| `c` | Reload configuration, themes, animations, and window config from `~/.config/egotop/` |
| `m` | Cycle view mode (`Dashboard` → `Compact` → `Minimal`) |
| `1`, `2`, `3` | Switch active dashboard page |
| `ArrowUp` / `ArrowDown` / `ArrowLeft` / `ArrowRight` | Nudge main window position (when `draggable = true`) |
| `Shift+Arrow` | Resize main window (when `resizable = true`) |
| Mouse drag on top window border | Move main window (when `draggable = true`) |
| Mouse drag on bottom-right corner (`◢`) | Resize main window (when `resizable = true`) |
| Mouse click on `[⚙]` / `[?]` | Open Customizer / toggle Help |

### 2. Command-line Mode (`:`)

| Input | Action |
|---|---|
| Printable characters | Insert character at cursor and refresh contextual suggestions |
| `Tab` | Select next suggestion and immediately complete the active token in the command buffer |
| `Shift+Tab` | Select previous suggestion and immediately update the active token |
| `Space` | Commit the current token, exit any active completion cycle, insert a space, and request next-argument suggestions |
| `Enter` | Execute the current command |
| `Esc` | Close the autocomplete popup if open; press `Esc` again to cancel command mode |
| `Backspace` / `Delete` | Delete character before/at cursor and refresh suggestions |
| `ArrowLeft` / `ArrowRight` / `Home` / `End` | Move edit cursor and refresh suggestions at cursor context |
| `ArrowUp` / `ArrowDown` | Cycle suggestions when typing; recall command history when the buffer is empty or popup is closed |
| `Ctrl+P` / `Ctrl+N` | Recall previous / next command from session history |

### 3. Visual Customizer Modal (`F2` / `g` / `[⚙]`)

| Input | Action |
|---|---|
| `Tab` / `ArrowRight` | Next tab (`Themes` → `Animations` → `Window` → `Layout`) |
| `Shift+Tab` / `ArrowLeft` | Previous tab |
| `ArrowUp` / `k`, `ArrowDown` / `j` | Move selection |
| `Enter` / `Space` | Apply theme, set startup animation, or toggle window/layout setting |
| `p` (Animations tab) | Live-preview the selected animation on the main window |
| `s` | Persist the active tab's settings to `~/.config/egotop/` |
| `Esc` / `q` | Close customizer modal |
| Mouse click | Click tabs to switch, click list items to apply/toggle, click outside modal to close |

### 4. Cursor Startup Mode (`--startup cursor` or `[startup].mode = "cursor"`)

| Input | Action |
|---|---|
| Mouse move | Updates cursor coordinates |
| Left mouse click | Triggers startup transition (`cursor-expand` by default) from the exact click coordinates (when `trigger` is `"mouse"` or `"any"`) |
| `Enter` or `Space` | Triggers startup transition from the current cursor position |
| `q`, `Esc`, `Ctrl+C` | Quit immediately and restore terminal |
