use crate::animation::{AnimationCatalog, BackgroundEffect, TransitionPlayer, create_effect};
use crate::command::{
    AnimationAction, CommandLineState, CommandRegistry, CompletionContext, LayoutSelection,
    ParsedCommand, WindowCommand, WriteTarget, parse_command,
};
use crate::config::{
    Config, EffectKind, StartupMode, StartupTrigger, ThemeEntry, ViewMode, load_all_themes,
    load_from_path, load_layout_names, load_or_create_window_config, save_config_to_path,
    save_window_config,
};
use crate::system::{SystemCollector, SystemSnapshot};
use crate::terminal::TerminalGuard;
use crate::terminal::input::{InputEvent, poll_event};
use crate::terminal::renderer::{CursorStartupState, draw_frame};
use crate::ui::RevealState;
use crate::ui::borders::parse_border_style;
use crate::ui::customizer::{
    CustomizerState, CustomizerTab, cycle_anchor, cycle_border_style, cycle_layout_mode,
    layout_items_summary, toggle_widget_enabled, window_items_summary,
};
use crate::ui::theme::Theme;
use crate::ui::widgets::{RenderContext, window_config_button_rect, window_help_button_rect};
use crate::window::{PositionMode, WindowBackgroundMode, WindowConfig, WindowManager};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct App {
    config: Config,
    config_path: PathBuf,
    windows: WindowManager,
    themes_map: BTreeMap<String, ThemeEntry>,
    theme_list: Vec<ThemeEntry>,
    theme: Theme,
    animations: AnimationCatalog,
    layout_names: Vec<String>,
    transition: TransitionPlayer,
    startup_cursor: CursorStartupState,
    registry: CommandRegistry,
    collector: SystemCollector,
    snapshot: SystemSnapshot,
    effect: Box<dyn BackgroundEffect>,
    reveal: RevealState,
    command_line: CommandLineState,
    customizer: CustomizerState,
    active_page: u8,
    view_mode: ViewMode,
    paused: bool,
    show_help: bool,
    should_quit: bool,
    fps: f32,
    status_toast: Option<(String, Instant)>,
}

impl App {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        initial_toast: Option<String>,
        width: u16,
        height: u16,
    ) -> Self {
        let themes_map = load_all_themes();
        let theme = resolve_active_theme(&config, &themes_map);
        let theme_list: Vec<ThemeEntry> = themes_map.values().cloned().collect();

        let win_cfg = load_or_create_window_config();
        let mut windows = WindowManager::new(win_cfg);
        windows.resolve_all(Rect::new(0, 0, width, height));

        let animations = AnimationCatalog::load();
        let layout_names = load_layout_names();

        let is_cursor_startup = config.startup.mode == StartupMode::Cursor;
        let transition = if is_cursor_startup {
            TransitionPlayer::new("none", &animations, 0)
        } else {
            TransitionPlayer::new(
                &config.general.startup_animation,
                &animations,
                config.general.startup_duration_ms,
            )
        };

        let mut collector = SystemCollector::new();
        let snapshot = collector.collect(width, height);
        let effect = create_effect(config.background.effect, width, height, &config.background);
        let active_page = config.general.default_page.max(1);
        let view_mode = config.general.view_mode;
        let fps = config.general.animation_fps as f32;

        Self {
            config,
            config_path,
            windows,
            themes_map,
            theme_list,
            theme,
            animations,
            layout_names,
            transition,
            startup_cursor: CursorStartupState::new(is_cursor_startup),
            registry: CommandRegistry::new(),
            collector,
            snapshot,
            effect,
            reveal: if is_cursor_startup {
                RevealState::complete()
            } else {
                RevealState::new(0.65)
            },
            command_line: CommandLineState::default(),
            customizer: CustomizerState::default(),
            active_page,
            view_mode,
            paused: false,
            show_help: false,
            should_quit: false,
            fps,
            status_toast: initial_toast.map(|m| (m, Instant::now() + Duration::from_secs(4))),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut guard = TerminalGuard::enter()?;
        let initial_size = guard.terminal.size()?;
        let viewport = Rect::new(0, 0, initial_size.width, initial_size.height);
        self.windows.resolve_all(viewport);
        self.effect
            .resize(initial_size.width, initial_size.height, &self.config.background);
        self.snapshot = self
            .collector
            .collect(initial_size.width, initial_size.height);

        let mut last_frame = Instant::now();
        let mut last_sys_refresh = Instant::now();

        while !self.should_quit {
            let now = Instant::now();
            let frame_budget = Duration::from_secs_f64(
                1.0 / (self.config.general.animation_fps.clamp(1, 120) as f64),
            );
            let sys_interval = Duration::from_millis(self.config.general.refresh_rate_ms.max(100));

            if !self.startup_cursor.pending
                && now.duration_since(last_sys_refresh) >= sys_interval
            {
                let sz = guard.terminal.size()?;
                self.snapshot = self.collector.collect(sz.width, sz.height);
                last_sys_refresh = now;
            }

            if let Some((_, expires_at)) = &self.status_toast
                && now >= *expires_at
            {
                self.status_toast = None;
            }

            let dt = now.duration_since(last_frame).as_secs_f32().clamp(0.001, 0.25);
            let dt_ms = now.duration_since(last_frame).as_millis() as u64;
            last_frame = now;
            self.fps = (self.fps * 0.85) + ((1.0 / dt) * 0.15);

            if self.startup_cursor.pending {
                self.startup_cursor.update_blink(
                    dt_ms,
                    self.config.startup.cursor.blink,
                    self.config.startup.cursor.blink_interval,
                );
            } else if !self.paused {
                self.effect.update(dt, &self.config.background);
                self.transition.update(dt);
                self.reveal.advance(dt);
            }

            let toast_text = self.status_toast.as_ref().map(|(s, _)| s.as_str());
            let window_transparent =
                self.windows.main_window().config.background.mode == WindowBackgroundMode::Transparent;
            let ctx = RenderContext {
                config: &self.config,
                theme: &self.theme,
                snapshot: &self.snapshot,
                reveal: &self.reveal,
                active_page: self.active_page,
                view_mode: self.view_mode,
                paused: self.paused,
                show_help: self.show_help,
                fps: self.fps,
                status_toast: toast_text,
                window_transparent,
            };

            let config_ref = &self.config;
            let effect_ref = self.effect.as_ref();
            let transition_ref = &self.transition;
            let windows_ref = &mut self.windows;
            let customizer_ref = &mut self.customizer;
            let cmd_ref = &self.command_line;
            let startup_ref = &self.startup_cursor;
            let themes_ref = &self.theme_list;
            let anim_ref = &self.animations;

            guard.terminal.draw(|frame| {
                draw_frame(
                    frame,
                    config_ref,
                    effect_ref,
                    transition_ref,
                    windows_ref,
                    customizer_ref,
                    cmd_ref,
                    startup_ref,
                    themes_ref,
                    anim_ref,
                    &ctx,
                );
            })?;

            let elapsed = Instant::now().duration_since(now);
            let wait = frame_budget.saturating_sub(elapsed).min(Duration::from_millis(50));
            if let Some(ev) = poll_event(wait)? {
                self.handle_input_event(ev, &mut guard, &mut last_sys_refresh)?;
            }
        }

        Ok(())
    }

    fn completion_ctx(&self) -> CompletionContext<'_> {
        CompletionContext {
            themes: &self.theme_list,
            animations: &self.animations,
            layouts: &self.layout_names,
        }
    }

    fn trigger_cursor_startup(&mut self, origin: (u16, u16), viewport: Rect) {
        self.startup_cursor.pending = false;
        self.windows.resolve_all(viewport);
        self.snapshot = self.collector.collect(viewport.width, viewport.height);
        self.transition.restart_from_origin(
            &self.config.startup.animation,
            &self.animations,
            self.config.general.startup_duration_ms,
            origin,
        );
        self.reveal = RevealState::new(0.65);
    }

    fn handle_input_event(
        &mut self,
        ev: InputEvent,
        guard: &mut TerminalGuard,
        last_sys_refresh: &mut Instant,
    ) -> Result<()> {
        let sz = guard.terminal.size()?;
        let viewport = Rect::new(0, 0, sz.width, sz.height);

        if self.startup_cursor.pending {
            match ev {
                InputEvent::Resize(w, h) => {
                    let vp = Rect::new(0, 0, w, h);
                    self.windows.resolve_all(vp);
                    self.effect.resize(w, h, &self.config.background);
                }
                InputEvent::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                            self.startup_cursor.pos = Some((mouse.column, mouse.row));
                        }
                        MouseEventKind::Down(MouseButton::Left)
                            if matches!(
                                self.config.startup.trigger,
                                StartupTrigger::Mouse | StartupTrigger::Any
                            ) =>
                        {
                            self.startup_cursor.pos = Some((mouse.column, mouse.row));
                            self.trigger_cursor_startup((mouse.column, mouse.row), viewport);
                        }
                        _ => {}
                    }
                }
                InputEvent::Key(key) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                        || matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc)
                    {
                        self.should_quit = true;
                        return Ok(());
                    }
                    if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                        let origin = self.startup_cursor.resolved_pos(viewport);
                        self.trigger_cursor_startup(origin, viewport);
                    }
                }
            }
            return Ok(());
        }

        match ev {
            InputEvent::Resize(w, h) => {
                let vp = Rect::new(0, 0, w, h);
                self.windows.resolve_all(vp);
                self.effect.resize(w, h, &self.config.background);
                self.snapshot = self.collector.collect(w, h);
            }
            InputEvent::Mouse(mouse) => {
                self.handle_mouse(mouse, viewport, guard)?;
            }
            InputEvent::Key(key) => {
                if self.command_line.active {
                    self.handle_command_key(key, guard, viewport)?;
                } else if self.customizer.open {
                    self.handle_customizer_key(key, guard, viewport)?;
                } else {
                    self.handle_normal_key(key, guard, viewport, last_sys_refresh)?;
                }
            }
        }
        Ok(())
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        viewport: Rect,
        guard: &TerminalGuard,
    ) -> Result<()> {
        if self.customizer.open {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let col = mouse.column;
                let row = mouse.row;
                let m = self.customizer.modal_rect;
                if col < m.x || col >= m.right() || row < m.y || row >= m.bottom() {
                    self.customizer.close();
                    return Ok(());
                }
                for (idx, tr) in self.customizer.tab_rects.iter().enumerate() {
                    if col >= tr.x && col < tr.right() && row == tr.y {
                        self.customizer.tab = CustomizerTab::all()[idx];
                        self.customizer.selected = 0;
                        return Ok(());
                    }
                }
                let cr = self.customizer.content_rect;
                if col >= cr.x && col < cr.right() && row >= cr.y && row < cr.bottom() {
                    let clicked_idx = (row - cr.y) as usize;
                    let len = self.customizer_list_len();
                    if clicked_idx < len {
                        self.customizer.selected = clicked_idx;
                        self.activate_customizer_selection(false, guard)?;
                    }
                }
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let win_rect = self.windows.main_window().resolved_rect;
                let cfg_btn = window_config_button_rect(win_rect);
                if cfg_btn.width > 0
                    && mouse.column >= cfg_btn.x
                    && mouse.column < cfg_btn.right()
                    && mouse.row == cfg_btn.y
                {
                    self.customizer.open_tab(CustomizerTab::Themes);
                    return Ok(());
                }
                let help_btn = window_help_button_rect(win_rect);
                if help_btn.width > 0
                    && mouse.column >= help_btn.x
                    && mouse.column < help_btn.right()
                    && mouse.row == help_btn.y
                {
                    self.show_help = !self.show_help;
                    return Ok(());
                }
                self.windows
                    .begin_mouse_down(mouse.column, mouse.row, viewport);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.windows
                    .update_mouse_drag(mouse.column, mouse.row, viewport);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.windows.end_mouse_drag();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_key(
        &mut self,
        key: KeyEvent,
        guard: &TerminalGuard,
        viewport: Rect,
    ) -> Result<()> {
        let mut cmd_state = std::mem::take(&mut self.command_line);
        let ctx = self.completion_ctx();

        let mut execute_str: Option<String> = None;
        match key.code {
            KeyCode::Esc => {
                if cmd_state.popup_open {
                    cmd_state.close_popup();
                } else {
                    cmd_state.cancel();
                }
            }
            KeyCode::Enter => {
                let cmd = cmd_state.submit();
                if !cmd.is_empty() {
                    execute_str = Some(cmd);
                }
            }
            KeyCode::Tab => cmd_state.next_suggestion(&self.registry, &ctx),
            KeyCode::BackTab => cmd_state.prev_suggestion(&self.registry, &ctx),
            KeyCode::Up => cmd_state.handle_up(&self.registry, &ctx),
            KeyCode::Down => cmd_state.handle_down(&self.registry, &ctx),
            KeyCode::Left => cmd_state.move_left(&self.registry, &ctx),
            KeyCode::Right => cmd_state.move_right(&self.registry, &ctx),
            KeyCode::Home => cmd_state.move_home(&self.registry, &ctx),
            KeyCode::End => cmd_state.move_end(&self.registry, &ctx),
            KeyCode::Backspace => cmd_state.backspace(&self.registry, &ctx),
            KeyCode::Delete => cmd_state.delete(&self.registry, &ctx),
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                cmd_state.history_prev(&self.registry, &ctx);
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                cmd_state.history_next(&self.registry, &ctx);
            }
            KeyCode::Char(c) => cmd_state.insert_char(c, &self.registry, &ctx),
            _ => {}
        }

        self.command_line = cmd_state;
        if let Some(cmd) = execute_str {
            self.execute_command_str(&cmd, guard, viewport)?;
        }
        Ok(())
    }

    fn handle_customizer_key(
        &mut self,
        key: KeyEvent,
        guard: &TerminalGuard,
        _viewport: Rect,
    ) -> Result<()> {
        let len = self.customizer_list_len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.customizer.close(),
            KeyCode::Tab | KeyCode::Right => {
                self.customizer.tab = self.customizer.tab.next();
                self.customizer.selected = 0;
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.customizer.tab = self.customizer.tab.prev();
                self.customizer.selected = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.customizer.move_up(len),
            KeyCode::Down | KeyCode::Char('j') => self.customizer.move_down(len),
            KeyCode::Char('p') if self.customizer.tab == CustomizerTab::Animations => {
                self.activate_customizer_selection(true, guard)?;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.activate_customizer_selection(false, guard)?;
            }
            KeyCode::Char('s') => {
                self.save_active_tab_settings();
            }
            _ => {}
        }
        Ok(())
    }

    fn customizer_list_len(&self) -> usize {
        match self.customizer.tab {
            CustomizerTab::Themes => self.theme_list.len(),
            CustomizerTab::Animations => self.animations.entries.len(),
            CustomizerTab::Window => window_items_summary(&self.windows.main_window().config).len(),
            CustomizerTab::Layout => layout_items_summary(&self.config).len(),
        }
    }

    fn activate_customizer_selection(
        &mut self,
        preview_only: bool,
        guard: &TerminalGuard,
    ) -> Result<()> {
        let idx = self.customizer.selected;
        match self.customizer.tab {
            CustomizerTab::Themes => {
                if let Some(entry) = self.theme_list.get(idx).cloned() {
                    self.apply_theme_entry(&entry);
                    self.set_toast(format!("Colorscheme '{}' applied (runtime)", entry.name));
                }
            }
            CustomizerTab::Animations => {
                if let Some(entry) = self.animations.entries.get(idx).cloned() {
                    if preview_only {
                        self.transition.restart(
                            &entry.id,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        self.reveal = RevealState::new(0.65);
                        self.customizer.close();
                        self.set_toast(format!("Previewing '{}'", entry.display_name));
                    } else {
                        self.config.general.startup_animation = entry.id.clone();
                        self.transition.restart(
                            &entry.id,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        self.set_toast(format!(
                            "Startup animation set to '{}' (use :write to persist)",
                            entry.display_name
                        ));
                    }
                }
            }
            CustomizerTab::Window => {
                let sz = guard.terminal.size()?;
                let vp = Rect::new(0, 0, sz.width, sz.height);
                {
                    let win = self.windows.main_window_mut();
                    match idx {
                        0 => {
                            win.config.position = PositionMode::Relative;
                            win.config.anchor = cycle_anchor(win.config.anchor);
                            win.config.offset_x = 0;
                            win.config.offset_y = 0;
                        }
                        1 => {
                            cycle_border_style(&mut win.config);
                            let style = win.config.border.style;
                            self.theme.set_border_style(style);
                        }
                        2 => {
                            win.config.background.mode = match win.config.background.mode {
                                WindowBackgroundMode::Solid => WindowBackgroundMode::Transparent,
                                WindowBackgroundMode::Transparent => WindowBackgroundMode::Solid,
                            };
                        }
                        3 => {
                            win.config.padding = (win.config.padding + 1) % 5;
                        }
                        4 => win.config.draggable = !win.config.draggable,
                        5 => win.config.resizable = !win.config.resizable,
                        6 => win.config = WindowConfig::default(),
                        7 => {
                            let cfg_clone = win.config.clone();
                            match save_window_config(&cfg_clone) {
                                Ok(()) => self.set_toast("Saved window settings to window.toml"),
                                Err(e) => self.set_toast(format!("Save error: {e}")),
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                self.windows.resolve_all(vp);
            }
            CustomizerTab::Layout => match idx {
                0 => self.config.layout.mode = cycle_layout_mode(self.config.layout.mode),
                1 => self.view_mode = self.view_mode.next(),
                2 => {
                    let next = self.config.background.effect.next();
                    self.set_effect(next, guard)?;
                }
                last if last == layout_items_summary(&self.config).len() - 1 => {
                    self.config.general.view_mode = self.view_mode;
                    match save_config_to_path(&self.config_path, &self.config) {
                        Ok(()) => self.set_toast("Saved configuration to egotop.toml"),
                        Err(e) => self.set_toast(format!("Save error: {e}")),
                    }
                }
                widget_idx => {
                    let w_pos = widget_idx.saturating_sub(3);
                    if let Some(id) = self.config.widgets.get(w_pos).map(|w| w.id) {
                        let now_on = toggle_widget_enabled(&mut self.config, id);
                        self.set_toast(format!(
                            "{}: {}",
                            id.default_title(),
                            if now_on { "visible" } else { "hidden" }
                        ));
                    }
                }
            },
        }
        Ok(())
    }

    fn save_active_tab_settings(&mut self) {
        match self.customizer.tab {
            CustomizerTab::Window => {
                match save_window_config(&self.windows.main_window().config) {
                    Ok(()) => self.set_toast("Saved window settings to ~/.config/egotop/window.toml"),
                    Err(e) => self.set_toast(format!("Save error: {e}")),
                }
            }
            _ => {
                self.save_main_config();
            }
        }
    }

    fn save_main_config(&mut self) {
        self.config.general.view_mode = self.view_mode;
        self.config.theme.borders = self.theme.borders.to_config();
        match save_config_to_path(&self.config_path, &self.config) {
            Ok(()) => self.set_toast("Saved configuration to ~/.config/egotop/egotop.toml"),
            Err(e) => self.set_toast(format!("Save error: {e}")),
        }
    }

    fn handle_normal_key(
        &mut self,
        key: KeyEvent,
        guard: &mut TerminalGuard,
        viewport: Rect,
        last_sys_refresh: &mut Instant,
    ) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        match key.code {
            KeyCode::Char(':') => {
                self.show_help = false;
                let mut cmd_state = std::mem::take(&mut self.command_line);
                let ctx = self.completion_ctx();
                cmd_state.open(&self.registry, &ctx);
                self.command_line = cmd_state;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::F(2) | KeyCode::Char('g') | KeyCode::Char('G') => {
                self.show_help = false;
                self.customizer.open_tab(CustomizerTab::Themes);
            }
            KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let sz = guard.terminal.size()?;
                self.snapshot = self.collector.collect(sz.width, sz.height);
                *last_sys_refresh = Instant::now();
                self.set_toast("System metrics refreshed");
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.paused = !self.paused;
                self.set_toast(if self.paused {
                    "Animations paused"
                } else {
                    "Animations resumed"
                });
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.reload_all(guard)?;
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.view_mode = self.view_mode.next();
                self.set_toast(format!("View mode: {}", self.view_mode.label()));
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                let next = self.config.background.effect.next();
                self.set_effect(next, guard)?;
                self.set_toast(format!("Background effect: {}", next.label()));
            }
            KeyCode::Char(d @ '1'..='3') => {
                self.active_page = (d as u8) - b'0';
                self.set_toast(format!("Page {}", self.active_page));
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.windows.resize_main_by(-2, 0, viewport);
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.windows.resize_main_by(2, 0, viewport);
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.windows.resize_main_by(0, -1, viewport);
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.windows.resize_main_by(0, 1, viewport);
            }
            KeyCode::Left => self.windows.nudge_main(-2, 0, viewport),
            KeyCode::Right => self.windows.nudge_main(2, 0, viewport),
            KeyCode::Up => self.windows.nudge_main(0, -1, viewport),
            KeyCode::Down => self.windows.nudge_main(0, 1, viewport),
            _ => {}
        }
        Ok(())
    }

    fn execute_command_str(
        &mut self,
        raw: &str,
        guard: &TerminalGuard,
        viewport: Rect,
    ) -> Result<()> {
        let parsed = match parse_command(raw) {
            Ok(cmd) => cmd,
            Err(msg) => {
                self.set_toast(msg);
                return Ok(());
            }
        };

        match parsed {
            ParsedCommand::Quit => self.should_quit = true,
            ParsedCommand::Help => self.show_help = true,
            ParsedCommand::Reload => self.reload_all(guard)?,
            ParsedCommand::Write(target) => self.execute_write(target),
            ParsedCommand::Colorscheme(None) => {
                self.customizer.open_tab(CustomizerTab::Themes);
            }
            ParsedCommand::Colorscheme(Some(name)) => {
                let key = name.trim().to_ascii_lowercase();
                if let Some(entry) = self.themes_map.get(&key).cloned() {
                    self.apply_theme_entry(&entry);
                    self.set_toast(format!(
                        "Colorscheme '{}' applied (use :write to persist)",
                        entry.name
                    ));
                } else {
                    self.set_toast(format!(
                        "Colorscheme '{name}' not found in built-ins or ~/.config/egotop/themes/"
                    ));
                }
            }
            ParsedCommand::Set { key, value } => {
                self.execute_set_option(&key, &value);
            }
            ParsedCommand::Animation { action, name } => match action {
                AnimationAction::OpenPicker => {
                    self.customizer.open_tab(CustomizerTab::Animations);
                }
                AnimationAction::Play => {
                    if let Some(anim_name) = name {
                        self.transition.restart(
                            &anim_name,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!("Playing animation '{anim_name}'"));
                    }
                }
                AnimationAction::SetStartup => {
                    if let Some(anim_name) = name {
                        self.config.general.startup_animation = anim_name.clone();
                        self.transition.restart(
                            &anim_name,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!(
                            "Startup animation set to '{anim_name}' (use :write to persist)"
                        ));
                    }
                }
            },
            ParsedCommand::Effect(None) => {
                let next = self.config.background.effect.next();
                self.set_effect(next, guard)?;
                self.set_toast(format!("Background effect: {}", next.label()));
            }
            ParsedCommand::Effect(Some(kind)) => {
                self.set_effect(kind, guard)?;
                self.set_toast(format!("Background effect: {}", kind.label()));
            }
            ParsedCommand::Layout(None) => {
                self.customizer.open_tab(CustomizerTab::Layout);
            }
            ParsedCommand::Layout(Some(sel)) => match sel {
                LayoutSelection::Mode(mode) => {
                    self.config.layout.mode = mode;
                    self.set_toast(format!("Layout mode: {mode:?}"));
                }
                LayoutSelection::View(view) => {
                    self.view_mode = view;
                    self.set_toast(format!("View mode: {}", view.label()));
                }
            },
            ParsedCommand::Window(wcmd) => {
                self.execute_window_command(wcmd, viewport);
            }
        }
        Ok(())
    }

    fn execute_set_option(&mut self, key: &str, value: &str) {
        let k = key.trim().to_ascii_lowercase();
        if k == "background" {
            match value.trim().to_ascii_lowercase().as_str() {
                "dark" => {
                    self.theme.set_brightness_mode(true);
                    self.set_toast("Background set to dark (runtime)");
                }
                "light" => {
                    self.theme.set_brightness_mode(false);
                    self.set_toast("Background set to light (runtime)");
                }
                other => self.set_toast(format!(
                    "Invalid background value '{other}' (expected dark or light)"
                )),
            }
            return;
        }

        if k == "border.style" {
            match parse_border_style(value) {
                Ok(style) => {
                    self.config.theme.border_type = style;
                    self.windows.main_window_mut().config.border.style = style;
                    self.theme.set_border_style(style);
                    self.set_toast(format!("Border style set to '{}'", style.label()));
                }
                Err(err) => self.set_toast(err),
            }
            return;
        }

        if let Some(prop) = k.strip_prefix("border.") {
            let mut next_borders = self.theme.borders.clone();
            match next_borders.set_property(prop, value) {
                Ok(()) => {
                    self.theme.borders = next_borders;
                    self.config.theme.borders = self.theme.borders.to_config();
                    self.set_toast(format!("Updated border.{prop} = {value} (runtime)"));
                }
                Err(err) => {
                    // Validation failed: preserve previous border configuration
                    self.set_toast(err);
                }
            }
            return;
        }

        self.set_toast(format!("Unknown property '{key}'"));
    }

    fn execute_write(&mut self, target: WriteTarget) {
        match target {
            WriteTarget::Config | WriteTarget::Theme => {
                self.save_main_config();
            }
            WriteTarget::Window => match save_window_config(&self.windows.main_window().config) {
                Ok(()) => self.set_toast("Saved window settings to ~/.config/egotop/window.toml"),
                Err(e) => self.set_toast(format!("Save error: {e}")),
            },
            WriteTarget::All => {
                self.config.general.view_mode = self.view_mode;
                self.config.theme.borders = self.theme.borders.to_config();
                let res_main = save_config_to_path(&self.config_path, &self.config);
                let res_win = save_window_config(&self.windows.main_window().config);
                match (res_main, res_win) {
                    (Ok(()), Ok(())) => {
                        self.set_toast("Wrote all settings to ~/.config/egotop/");
                    }
                    (Err(e), _) | (_, Err(e)) => self.set_toast(format!("Write error: {e}")),
                }
            }
        }
    }

    fn execute_window_command(&mut self, wcmd: WindowCommand, viewport: Rect) {
        if matches!(wcmd, WindowCommand::OpenInspector) {
            self.customizer.open_tab(CustomizerTab::Window);
            return;
        }
        let mut new_border_style = None;
        let toast_msg: Option<String> = {
            let win = self.windows.main_window_mut();
            match wcmd {
                WindowCommand::OpenInspector => None,
                WindowCommand::Center => {
                    win.config.position = PositionMode::Relative;
                    win.config.anchor = crate::config::Anchor::Center;
                    win.config.offset_x = 0;
                    win.config.offset_y = 0;
                    Some("Window centered".to_string())
                }
                WindowCommand::SetAnchor(a) => {
                    win.config.position = PositionMode::Relative;
                    win.config.anchor = a;
                    win.config.offset_x = 0;
                    win.config.offset_y = 0;
                    Some(format!("Window anchor set to {a:?}"))
                }
                WindowCommand::SetDraggable(opt) => {
                    win.config.draggable = opt.unwrap_or(!win.config.draggable);
                    Some(format!("Window draggable: {}", win.config.draggable))
                }
                WindowCommand::SetResizable(opt) => {
                    win.config.resizable = opt.unwrap_or(!win.config.resizable);
                    Some(format!("Window resizable: {}", win.config.resizable))
                }
                WindowCommand::ToggleBorder(opt) => {
                    win.config.border.enabled = opt.unwrap_or(!win.config.border.enabled);
                    Some(format!("Window border: {}", win.config.border.enabled))
                }
                WindowCommand::SetBorderStyle(style) => {
                    win.config.border.enabled = true;
                    win.config.border.style = style;
                    new_border_style = Some(style);
                    Some(format!("Border style set to '{}'", style.label()))
                }
                WindowCommand::SetBackground(mode) => {
                    win.config.background.mode = mode;
                    Some(format!("Window background: {mode:?}"))
                }
                WindowCommand::SetPadding(p) => {
                    win.config.padding = p;
                    Some(format!("Window padding set to {p}"))
                }
                WindowCommand::SetVisible(opt) => {
                    win.config.visible = opt.unwrap_or(!win.config.visible);
                    None
                }
                WindowCommand::Reset => {
                    win.config = WindowConfig::default();
                    Some("Window reset to defaults".to_string())
                }
            }
        };
        if let Some(style) = new_border_style {
            self.theme.set_border_style(style);
        }
        if let Some(msg) = toast_msg {
            self.set_toast(msg);
        }
        self.windows.resolve_all(viewport);
    }

    fn reload_all(&mut self, guard: &TerminalGuard) -> Result<()> {
        self.themes_map = load_all_themes();
        self.theme_list = self.themes_map.values().cloned().collect();
        self.animations = AnimationCatalog::load();
        self.layout_names = load_layout_names();

        match load_from_path(&self.config_path) {
            Ok(new_cfg) => {
                let sz = guard.terminal.size()?;
                self.theme = resolve_active_theme(&new_cfg, &self.themes_map);
                self.effect = create_effect(
                    new_cfg.background.effect,
                    sz.width,
                    sz.height,
                    &new_cfg.background,
                );
                self.transition.restart(
                    &new_cfg.general.startup_animation,
                    &self.animations,
                    new_cfg.general.startup_duration_ms,
                );
                let win_cfg = load_or_create_window_config();
                *self.windows.main_window_mut() = crate::window::WindowState::new(win_cfg);
                self.windows.resolve_all(Rect::new(0, 0, sz.width, sz.height));
                self.config = new_cfg;
                self.set_toast("Reloaded configuration, themes, and animations");
            }
            Err(err) => {
                self.set_toast(format!("Invalid configuration: {err}"));
            }
        }
        Ok(())
    }

    fn apply_theme_entry(&mut self, entry: &ThemeEntry) {
        self.theme = entry.theme.clone();
        self.config.theme.name = entry.name.clone();
        if entry.source_path.is_some() {
            self.config.theme.background.clear();
            self.config.theme.panel_background.clear();
            self.config.theme.primary.clear();
            self.config.theme.secondary.clear();
            self.config.theme.accent.clear();
            self.config.theme.text.clear();
            self.config.theme.muted.clear();
            self.config.theme.border.clear();
            self.config.theme.danger.clear();
            self.config.theme.borders = Default::default();
        }
    }

    fn set_effect(&mut self, kind: EffectKind, guard: &TerminalGuard) -> Result<()> {
        self.config.background.effect = kind;
        self.config.background.enabled = kind != EffectKind::None;
        match kind {
            EffectKind::Matrix => self.config.background.matrix.enabled = true,
            EffectKind::Water => self.config.background.water.enabled = true,
            EffectKind::Particles => self.config.background.particles.enabled = true,
            EffectKind::Scanlines => self.config.background.scanlines.enabled = true,
            EffectKind::None => {}
        }
        let sz = guard.terminal.size()?;
        self.effect = create_effect(kind, sz.width, sz.height, &self.config.background);
        Ok(())
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.status_toast = Some((msg.into(), Instant::now() + Duration::from_secs(3)));
    }
}

fn resolve_active_theme(cfg: &Config, themes_map: &BTreeMap<String, ThemeEntry>) -> Theme {
    let key = cfg.theme.name.trim().to_ascii_lowercase();
    if let Some(entry) = themes_map.get(&key)
        && entry.source_path.is_some()
    {
        let mut theme = entry.theme.clone();
        let _ = theme.borders.apply_overrides(&cfg.theme.borders);
        return theme;
    }
    Theme::from_config(&cfg.theme)
}
