# Borders

Egotop renders every bordered UI element—the main window, header, system panels, modals, and the command-line autocomplete popup—through a single centralized `BorderChars` structure (`src/ui/borders.rs`) stored on the active `Theme`.

## Border Properties

Every border consists of 11 configurable single-cell characters plus a preset `style`:

| Property | Default (`rounded`) | Purpose |
|---|---|---|
| `style` | `"rounded"` | Applies a full preset (`rounded`, `plain`, `double`, `thick`) |
| `top_left` | `"╭"` | Top-left corner |
| `top_right` | `"╮"` | Top-right corner |
| `bottom_left` | `"╰"` | Bottom-left corner |
| `bottom_right` | `"╯"` | Bottom-right corner |
| `horizontal` | `"─"` | Top and bottom horizontal edges |
| `vertical` | `"│"` | Left and right vertical edges |
| `left_separator` | `"├"` | Left junction separator |
| `right_separator` | `"┤"` | Right junction separator |
| `top_separator` | `"┬"` | Top junction separator |
| `bottom_separator` | `"┴"` | Bottom junction separator |
| `intersection` | `"┼"` | Cross intersection |

### Built-in Presets

- `rounded`: `╭ ╮ ╰ ╯ ─ │ ├ ┤ ┬ ┴ ┼`
- `plain` (also accepted as `solid`): `┌ ┐ └ ┘ ─ │ ├ ┤ ┬ ┴ ┼`
- `double`: `╔ ╗ ╚ ╝ ═ ║ ╠ ╣ ╦ ╩ ╬`
- `thick`: `┏ ┓ ┗ ┛ ━ ┃ ┣ ┫ ┳ ┻ ╋`

## Runtime Customization

Use `:set` in the command line (`:`) to inspect and modify borders live without restarting:

```text
:set border.style double
:set border.horizontal ━
:set border.vertical ┃
:set border.top_left ╔
:set border.bottom_right ╝
```

As soon as you press `Enter`, all panels, windows, and the autocomplete popup reflect the updated border characters immediately.

### Autocomplete Support

Typing `:set border.` triggers contextual completion listing every property (`border.style`, `border.horizontal`, `border.vertical`, `border.top_left`, etc.). Typing `:set border.style ` suggests `rounded`, `plain`, `double`, and `thick`. Typing `:set border.horizontal ` suggests common single-width box-drawing characters.

## Character Validation

Every individual border character is validated before updating the active border set (`validate_border_char`):
- Must contain **exactly 1 Unicode scalar character** (empty strings and multi-character strings are rejected).
- Control characters are rejected.
- Terminal display width (`unicode-width`) **must equal 1** — zero-width combining marks and double-width characters/emojis (e.g. `🔥`, `❌`) are rejected so borders never misalign panels.

If validation fails, Egotop displays an English error message in the bottom status line and preserves the previous border configuration unchanged.

## Persistent Border Configuration

Runtime `:set border.*` changes are temporary until saved:
- Run `:write` (or `:w`, `:write config`, `:write theme`) to write `[theme.borders]` into `~/.config/egotop/egotop.toml`.
- You can also configure borders directly in `~/.config/egotop/egotop.toml` or inside a theme file under `~/.config/egotop/themes/*.toml`:

```toml
[theme]
name = "matrix"
border_type = "rounded"

[theme.borders]
top_left = "┏"
top_right = "┓"
bottom_left = "┗"
bottom_right = "┛"
horizontal = "━"
vertical = "┃"
left_separator = "┣"
right_separator = "┫"
top_separator = "┳"
bottom_separator = "┻"
intersection = "╋"
```
