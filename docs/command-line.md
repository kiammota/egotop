# Command-line and Autocomplete

Egotop includes a Neovim-style command line hidden until you press `:`.

## How Autocomplete Works

1. Press `:` to open the command prompt. Suggestions appear automatically in a discrete popup directly above the prompt, styled with the active theme and central border characters.
2. **Immediate `Tab` completion (no `Enter` required):**
   - Pressing `Tab` completes the current token immediately in the buffer and highlights the active suggestion.
   - Pressing `Tab` again advances to the next suggestion; `Shift+Tab` steps backward. Repeated `Tab`/`Shift+Tab` presses replace only the active token without duplicating text.
   - Pressing `Space` commits the active token, exits the cycle, inserts a space, and fetches contextual suggestions for the next argument.
   - Pressing `Enter` always executes the command currently in the buffer.
   - Pressing `Backspace`, typing characters, or moving with `ArrowLeft` / `ArrowRight` updates suggestions dynamically for the current cursor position.
   - Pressing `Esc` closes the autocomplete popup if open, or cancels command mode if already closed.

Example flow:
```text
:color          # popup shows: colorscheme
<Tab>           # buffer becomes: :colorscheme
<Space>         # buffer becomes: :colorscheme  | popup shows: matrix, bamboo, nord, ...
<Tab>           # buffer becomes: :colorscheme matrix
<Tab>           # buffer becomes: :colorscheme bamboo
<Shift+Tab>     # buffer becomes: :colorscheme matrix
<Enter>         # executes :colorscheme matrix immediately
```

## Command Reference

All commands are registered in `CommandRegistry` (`src/command/registry.rs`), parsed by `parse_command` (`src/command/parser.rs`), and executed by `App` (`src/app.rs`).

### `:colorscheme`

- **Aliases:** `:theme`
- **Description:** Apply a colorscheme in runtime or open the interactive theme picker.
- **Syntax:** `:colorscheme [name]`
- **Arguments:** `name` (optional) — built-in or user theme name from `~/.config/egotop/themes/*.toml`.
- **Examples:**
  ```text
  :colorscheme
  :colorscheme bamboo
  :colorscheme nord
  ```
- **Expected behavior:** Without arguments, opens the **Themes** tab of the customizer. With `name`, switches theme immediately in memory (use `:write` to persist).

### `:set`

- **Aliases:** none
- **Description:** Update runtime theme brightness or border characters/style immediately.
- **Syntax:** `:set <property> <value>`
- **Arguments:**
  - `property`: `background`, `border.style`, or `border.<char_field>` (`horizontal`, `vertical`, `top_left`, `top_right`, `bottom_left`, `bottom_right`, `left_separator`, `right_separator`, `top_separator`, `bottom_separator`, `intersection`).
  - `value`: `dark` | `light` for `background`; `rounded` | `plain` (`solid`) | `double` | `thick` for `border.style`; a single width-1 Unicode character for `border.<char_field>`.
- **Examples:**
  ```text
  :set background light
  :set border.style double
  :set border.horizontal ━
  :set border.top_left ╔
  ```
- **Expected behavior:** Updates all bordered UI components immediately in runtime. Invalid characters or values display an English error and preserve the prior state.

### `:animation`

- **Aliases:** `:anim`
- **Description:** Play, pause, resume, stop, restart, speed-scale, or select bundled and community animations.
- **Syntax:** `:animation [play|set|pause|resume|stop|restart|speed|list] [arg]`
- **Arguments:**
  - `list` or omitted: opens the **Animations** tab in the visual customizer.
  - `play <name>`: plays the specified transition or scene animation immediately (also applies declared `[[backgrounds]]`).
  - `set <name>` or `<name>`: sets `general.startup_animation` in runtime and plays it.
  - `pause` / `resume` / `stop` / `restart`: non-blocking playback controls for the active timeline.
  - `speed <factor>`: scales playback speed (`0.05..=10.0`).
- **Examples:**
  ```text
  :animation play shape-morph
  :animation pause
  :animation resume
  :animation speed 1.5
  :animation stop
  :animation set cyber_boot
  ```
- **Expected behavior:** Runs the selected transition or multi-layer scene without blocking TUI input (press `Esc` or `:animation stop` to dismiss an active foreground loop; use `:write` to persist `startup_animation`).

### `:background`

- **Aliases:** `:bg`
- **Description:** Replace, stack, pause, resume, speed-scale, or stop animated background layers while keeping the dashboard interactive.
- **Syntax:** `:background <set|add|stop|pause|resume|speed|list> [arg]`
- **Arguments:**
  - `set <name>`: replaces the active background stack with a preset (`matrix_rain`, `starfield`, `particles`, `waves`, `noise`, `gradient`, `floating_chars`, `custom_ascii`, `scanlines`, `none`) or the `[[backgrounds]]` stack of an animation package (e.g. `aurora-horizon`).
  - `add <name>`: stacks an additional animated background layer on top of existing layers.
  - `stop`: stops and clears all active background layers.
  - `pause` / `resume`: pauses or resumes background updates.
  - `speed <factor>`: scales background speed (`0.05..=10.0`).
  - `list`: shows active background layer count and available effect presets.
- **Examples:**
  ```text
  :background set starfield
  :background add noise
  :background speed 1.5
  :background pause
  :background resume
  :background stop
  ```
- **Expected behavior:** Updates `BackgroundPipeline` live behind all windows and foreground scenes without restarting Egotop.

### `:layout`

- **Aliases:** none
- **Description:** Switch layout positioning mode or view density, or open the Layout customizer tab.
- **Syntax:** `:layout [load|mode] [name]`
- **Arguments:** `name` (optional) — `responsive`, `free`, `compact`, `minimal`, or `dashboard`.
  *(Known limitation: `~/.config/egotop/layouts/*.toml` filenames are discovered for completion suggestions, while only the five built-in layout/view modes above are applied at execution time.)*
- **Examples:**
  ```text
  :layout
  :layout free
  :layout load compact
  ```
- **Expected behavior:** Updates the active layout or view mode immediately.

### `:window`

- **Aliases:** `:win`
- **Description:** Configure main window placement, borders, background transparency, dragging, resizing, and padding.
- **Syntax:** `:window [subcommand] [value]`
- **Subcommands:**
  - omitted: opens the **Window** tab in the customizer.
  - `center`: anchors window to `center` with zero offset.
  - `anchor <top-left|top-center|top-right|center-left|center|center-right|bottom-left|bottom-center|bottom-right>`
  - `border [on|off|rounded|plain|solid|double|thick]`
  - `bg <solid|transparent>`
  - `drag [on|off|toggle]`
  - `resize [on|off|toggle]`
  - `padding <0..10>`
  - `show` | `hide` | `toggle`
  - `reset`: restores default `WindowConfig`.
- **Examples:**
  ```text
  :window anchor top-right
  :window bg transparent
  :window border double
  :window padding 2
  ```
- **Expected behavior:** Updates `WindowManager` immediately and clamps geometry to the terminal viewport.

### `:effect`

- **Aliases:** `:fx`
- **Description:** Switch or cycle the ambient background effect.
- **Syntax:** `:effect [matrix|water|particles|scanlines|none]`
- **Examples:**
  ```text
  :effect water
  :effect none
  ```
- **Expected behavior:** Switches the active `BackgroundEffect` without restarting.

### `:write`

- **Aliases:** `:w`
- **Description:** Persist current runtime changes to `~/.config/egotop/`.
- **Syntax:** `:write [all|config|window|theme]`
- **Arguments:** target defaults to `all`.
- **Examples:**
  ```text
  :write
  :w
  :write window
  :write config
  ```
- **Expected behavior:** Writes `egotop.toml` and/or `window.toml` cleanly using pretty TOML serialization.

### `:reload`

- **Aliases:** `:r`
- **Description:** Reload configuration, themes, community animations, layout list, and window settings from disk.
- **Syntax:** `:reload`
- **Expected behavior:** Re-reads `~/.config/egotop/` files. On invalid TOML, reports the parse/validation error in English and keeps the running UI intact.

### `:help`

- **Aliases:** `:h`
- **Description:** Open the Help overlay.
- **Syntax:** `:help`

### `:quit`

- **Aliases:** `:q`
- **Description:** Exit Egotop cleanly and restore raw mode, mouse capture, cursor visibility, colors, and alternate screen.
- **Syntax:** `:quit` or `:q`
