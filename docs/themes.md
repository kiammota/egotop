# Themes

Egotop manages colorschemes through the Theme Manager (`src/config/theme_loader.rs` and `src/ui/theme.rs`).

## Built-in Colorschemes

Egotop includes five built-in presets:

- `matrix` (default): dark green canvas (`#050A07`), neon green primary (`#39FF88`), cyan secondary (`#00D9FF`), amber accent (`#FFC857`)
- `cyberpunk`: deep purple canvas (`#0A0612`), cyan primary (`#00FFD5`), magenta secondary (`#FF2A85`), yellow accent (`#FFDD00`)
- `nord`: polar night (`#2E3440`), frost primary (`#88C0D0`), aurora green secondary (`#A3BE8C`), warm yellow accent (`#EBCB8B`)
- `amber`: retro monochrome terminal (`#0C0802`), amber primary (`#FFB000`), bright amber secondary (`#FFD65A`)
- `dracula`: dark slate (`#1E1F29`), green primary (`#50FA7B`), cyan secondary (`#8BE9FD`), purple border (`#BD93F9`)

## Creating a Custom Theme

Create any `*.toml` file inside `~/.config/egotop/themes/`. For example, `~/.config/egotop/themes/bamboo.toml`:

```toml
[theme]
name = "bamboo"
background = "#0B100B"
panel_background = "#111A12"
primary = "#A8C686"
secondary = "#7FAF8A"
accent = "#D6B878"
text = "#E4E8DC"
muted = "#82917C"
border = "#526B45"
danger = "#D98282"
border_type = "solid"

[theme.borders]
top_left = "┌"
top_right = "┐"
bottom_left = "└"
bottom_right = "┘"
horizontal = "─"
vertical = "│"
```

Both wrapped (`[theme] ...`) and flat top-level theme TOML files are accepted. If `name` is omitted or empty, the filename stem is used as the theme name.

### Color Value Formats

Every color field accepts:
- 6-digit hex RGB: `"#RRGGBB"` (case-insensitive, e.g. `"#39FF88"`)
- Named terminal colors: `"black"`, `"red"`, `"green"`, `"yellow"`, `"blue"`, `"magenta"`, `"cyan"`, `"gray"`, `"darkgray"`, `"lightred"`, `"lightgreen"`, `"lightyellow"`, `"lightblue"`, `"lightmagenta"`, `"lightcyan"`, `"white"`
- Terminal default / cell transparency: `"reset"`, `"default"`, `"transparent"`

### Border Styles in Themes

`border_type` accepts `"rounded"`, `"plain"` (or alias `"solid"`), `"double"`, and `"thick"`. Optional `[theme.borders]` overrides individual border characters (see [borders.md](borders.md)).

## Loading and Switching Themes at Runtime

- At startup and on `:reload`, Egotop scans `~/.config/egotop/themes/*.toml` once and caches all discovered themes in memory.
- Switching via command line:
  ```text
  :colorscheme bamboo
  ```
  Type `:colorscheme ` and press `Tab` / `Shift+Tab` to cycle through all built-in and user themes registered in the Theme Manager.
- Switching via visual customizer: click `[⚙]` on the window border or press `F2` / `g`, select a theme under **Themes**, and press `Enter`.
- Runtime colorscheme changes are temporary until persisted with `:write` (or `s` in the customizer).

## Validation and Troubleshooting

- **Color validation**: When validating `egotop.toml` (`egotop --check-config` or `:reload`), invalid color strings produce `Configuration validation error in '<field>': invalid color '...'`.
- **User theme files**: Invalid TOML files inside `~/.config/egotop/themes/` are skipped during directory scanning so they never crash startup. If a theme does not appear in `:colorscheme` autocomplete:
  1. Confirm the file ends with `.toml` and lives inside `~/.config/egotop/themes/`.
  2. Check that TOML syntax is valid (`[theme]` header or flat keys).
  3. Run `:reload` inside Egotop or restart to re-scan the directory.
