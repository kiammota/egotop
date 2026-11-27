use crate::animation::{
    AnimationCatalog, BackgroundEffect, BackgroundLayerKind, BackgroundLayerSpec,
    BackgroundPipeline, TransitionPlayer,
};
use crate::command::{
    AnimationAction, BackgroundCommand, CommandLineState, CommandRegistry, CompletionContext,
    ContainerCommand, ContainerProperty, LayoutSelection, ParsedCommand, WriteTarget,
    parse_command,
};
use crate::config::{
    Config, EffectKind, StartupMode, StartupTrigger, ThemeEntry, ViewMode, save_config_to_path,
};
use crate::runtime::{
    ConfigurationRuntime, LayoutPreset, ManagedAppTarget, RuntimeCommit, RuntimePaths,
    RuntimeResourceManager,
};
use crate::system::{SystemCollector, SystemSnapshot};
use crate::terminal::TerminalGuard;
use crate::terminal::input::{InputEvent, poll_event};
use crate::terminal::renderer::{CursorStartupState, draw_frame};
use crate::ui::borders::parse_border_style;
use crate::ui::customizer::{
    CustomizerState, CustomizerTab, DiagnosticsOverlayState, DiagnosticsViewMode,
    collect_component_ids, layout_items_summary, render_diagnostics_overlay,
    toggle_component_visible,
};
use crate::ui::theme::Theme;
use crate::ui::widgets::RenderContext;
use crate::ui::{
    ComponentAnimationStore, ContainerGradient, ContainerStyle, RevealState,
    collect_all_container_ids, compute_dashboard_layout, has_container_id,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct App {
    config: Config,
    config_path: PathBuf,
    themes_map: BTreeMap<String, ThemeEntry>,
    theme_list: Vec<ThemeEntry>,
    theme: Theme,
    animations: AnimationCatalog,
    layout_names: Vec<String>,
    container_ids: Vec<String>,
    layouts_map: BTreeMap<String, LayoutPreset>,
    transition: TransitionPlayer,
    startup_cursor: CursorStartupState,
    registry: CommandRegistry,
    collector: SystemCollector,
    snapshot: SystemSnapshot,
    effect: BackgroundPipeline,
    anim_store: ComponentAnimationStore,
    reveal: RevealState,
    command_line: CommandLineState,
    customizer: CustomizerState,
    diagnostics_ui: DiagnosticsOverlayState,
    runtime: ConfigurationRuntime,
    view_mode: ViewMode,
    paused: bool,
    show_help: bool,
    should_quit: bool,
    fps: f32,
    status_toast: Option<(String, Instant)>,
    theme_preview_backup: Option<(Theme, crate::config::ThemeConfig)>,
}

impl App {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        initial_toast: Option<String>,
        width: u16,
        height: u16,
    ) -> Self {
        let runtime = ConfigurationRuntime::start(
            RuntimePaths::default_with_config(config_path.clone()),
            config,
            true,
        );
        let lkg = runtime.state().clone();
        let config = lkg.config;
        let themes_map = lkg.themes;
        let theme = resolve_active_theme(&config, &themes_map);
        let theme_list: Vec<ThemeEntry> = themes_map.values().cloned().collect();

        let animations = lkg.animations;
        let layouts_map = lkg.layouts;
        let layout_names: Vec<String> = layouts_map.keys().cloned().collect();
        let mut container_ids = Vec::new();
        collect_all_container_ids(&config.ui.root, &mut container_ids);

        let is_cursor_startup = config.startup.mode == StartupMode::Cursor;
        let wait_init = is_cursor_startup || config.init.wait_for_input;
        let transition = if wait_init {
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
        let effect = BackgroundPipeline::from_config(&config.background, width, height);
        let view_mode = config.general.view_mode;
        let fps = config.general.animation_fps as f32;

        Self {
            config,
            config_path,
            themes_map,
            theme_list,
            theme,
            animations,
            layout_names,
            container_ids,
            layouts_map,
            transition,
            startup_cursor: CursorStartupState::new(wait_init),
            registry: CommandRegistry::new(),
            collector,
            snapshot,
            effect,
            anim_store: ComponentAnimationStore::new(),
            reveal: if is_cursor_startup {
                RevealState::complete()
            } else {
                RevealState::new(0.65)
            },
            command_line: CommandLineState::default(),
            customizer: CustomizerState::default(),
            diagnostics_ui: DiagnosticsOverlayState::default(),
            runtime,
            view_mode,
            paused: false,
            show_help: false,
            should_quit: false,
            fps,
            status_toast: initial_toast.map(|m| (m, Instant::now() + Duration::from_secs(4))),
            theme_preview_backup: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut guard = TerminalGuard::enter()?;
        let initial_size = guard.terminal.size()?;
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

            if let Some(commit) = self.runtime.poll() {
                self.apply_runtime_commit(&commit, &guard)?;
            }

            if self.startup_cursor.pending {
                self.startup_cursor.update_blink(
                    dt_ms,
                    self.config.startup.cursor.blink,
                    self.config.startup.cursor.blink_interval,
                );
            } else if !self.paused {
                let sz = guard.terminal.size()?;
                let vp = Rect::new(0, 0, sz.width, sz.height);
                let layout = compute_dashboard_layout(vp, &self.config, self.view_mode);
                self.effect.update(dt, &self.config.background);
                self.anim_store
                    .update_all(dt, &layout, &self.config, &self.animations);
                self.transition.update(dt);
                self.reveal.advance(dt);
            }

            let toast_text = self.status_toast.as_ref().map(|(s, _)| s.as_str());
            let transparent_bg = self.theme.background == ratatui::style::Color::Reset;
            let ctx = RenderContext {
                config: &self.config,
                theme: &self.theme,
                snapshot: &self.snapshot,
                reveal: &self.reveal,
                view_mode: self.view_mode,
                paused: self.paused,
                show_help: self.show_help,
                fps: self.fps,
                status_toast: toast_text,
                transparent_bg,
                config_error_count: self.runtime.diagnostics().error_count(),
            };

            let config_ref = &self.config;
            let effect_ref: &dyn BackgroundEffect = &self.effect;
            let transition_ref = &self.transition;
            let anim_store_ref = &self.anim_store;
            let customizer_ref = &mut self.customizer;
            let diagnostics_ui_ref = &mut self.diagnostics_ui;
            let diagnostics_ref = self.runtime.diagnostics();
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
                    anim_store_ref,
                    customizer_ref,
                    cmd_ref,
                    startup_ref,
                    themes_ref,
                    anim_ref,
                    &ctx,
                );
                if diagnostics_ui_ref.open {
                    render_diagnostics_overlay(
                        frame,
                        frame.area(),
                        diagnostics_ui_ref,
                        diagnostics_ref,
                        ctx.theme,
                    );
                }
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
            containers: &self.container_ids,
        }
    }

    fn trigger_cursor_startup(&mut self, origin: (u16, u16), viewport: Rect) {
        self.startup_cursor.pending = false;
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
                    if self.config.init.wait_for_input
                        || matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
                    {
                        let origin = self.startup_cursor.resolved_pos(viewport);
                        self.trigger_cursor_startup(origin, viewport);
                    }
                }
            }
            return Ok(());
        }

        match ev {
            InputEvent::Resize(w, h) => {
                self.effect.resize(w, h, &self.config.background);
                self.snapshot = self.collector.collect(w, h);
            }
            InputEvent::Mouse(mouse) => {
                self.handle_mouse(mouse, viewport, guard)?;
            }
            InputEvent::Key(key) => {
                if self.command_line.active {
                    self.handle_command_key(key, guard, viewport)?;
                } else if self.diagnostics_ui.open {
                    self.handle_diagnostics_key(key);
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
        _viewport: Rect,
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
        let mut preview_theme = false;
        let mut restore_preview = false;
        match key.code {
            KeyCode::Esc => {
                if cmd_state.popup_open {
                    cmd_state.close_popup();
                } else {
                    cmd_state.cancel();
                    restore_preview = true;
                }
            }
            KeyCode::Enter => {
                self.theme_preview_backup = None;
                let cmd = cmd_state.submit();
                if !cmd.is_empty() {
                    execute_str = Some(cmd);
                }
            }
            KeyCode::Tab => {
                cmd_state.next_suggestion(&self.registry, &ctx);
                preview_theme = true;
            }
            KeyCode::BackTab => {
                cmd_state.prev_suggestion(&self.registry, &ctx);
                preview_theme = true;
            }
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
        if restore_preview
            && let Some((prev_theme, prev_cfg)) = self.theme_preview_backup.take()
        {
            self.theme = prev_theme;
            self.config.theme = prev_cfg;
        }
        if preview_theme {
            self.maybe_preview_colorscheme_from_cmdline();
        }
        if let Some(cmd) = execute_str {
            self.execute_command_str(&cmd, guard, viewport)?;
        }
        Ok(())
    }

    fn maybe_preview_colorscheme_from_cmdline(&mut self) {
        let trimmed = self.command_line.buffer.trim_start();
        let mut parts = trimmed.split_whitespace();
        let Some(head) = parts.next() else {
            return;
        };
        if !matches!(head.to_ascii_lowercase().as_str(), "colorscheme" | "theme") {
            return;
        }
        let Some(arg) = parts.next() else {
            return;
        };
        let key = arg.trim().to_ascii_lowercase();
        if let Some(entry) = self.themes_map.get(&key).cloned() {
            if self.theme_preview_backup.is_none() {
                self.theme_preview_backup = Some((self.theme.clone(), self.config.theme.clone()));
            }
            self.apply_theme_entry(&entry);
        }
    }

    fn handle_diagnostics_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.diagnostics_ui.close(),
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                self.diagnostics_ui.toggle_mode();
            }
            KeyCode::Up | KeyCode::Char('k') => self.diagnostics_ui.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.diagnostics_ui.scroll_down(),
            _ => {}
        }
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
                        if !self.transition.package_backgrounds().is_empty() {
                            let sz = guard.terminal.size()?;
                            self.effect.set_specs(
                                self.transition.package_backgrounds(),
                                sz.width,
                                sz.height,
                                &self.config.background,
                                &self.transition.package.assets,
                            );
                        }
                        self.reveal = RevealState::new(0.65);
                        self.customizer.close();
                        self.set_toast(format!("Playing '{}'", entry.display_name));
                    } else {
                        self.config.general.startup_animation = entry.id.clone();
                        self.transition.restart(
                            &entry.id,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        if !self.transition.package_backgrounds().is_empty() {
                            let sz = guard.terminal.size()?;
                            self.effect.set_specs(
                                self.transition.package_backgrounds(),
                                sz.width,
                                sz.height,
                                &self.config.background,
                                &self.transition.package.assets,
                            );
                        }
                        self.set_toast(format!(
                            "Startup animation set to '{}' (use :write to persist)",
                            entry.display_name
                        ));
                    }
                }
            }
            CustomizerTab::Layout => match idx {
                0 => self.view_mode = self.view_mode.next(),
                1 => {
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
                comp_idx => {
                    let mut comps = Vec::new();
                    collect_component_ids(&self.config.ui.root, &mut comps);
                    if let Some((id, _)) = comps.get(comp_idx.saturating_sub(2)) {
                        let id = id.clone();
                        if let Some(now_on) = toggle_component_visible(&mut self.config.ui.root, &id) {
                            self.set_toast(format!(
                                "{id}: {}",
                                if now_on { "visible" } else { "hidden" }
                            ));
                        }
                    }
                }
            },
        }
        Ok(())
    }

    fn save_active_tab_settings(&mut self) {
        self.save_main_config();
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
        _viewport: Rect,
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
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                } else if self.transition.has_scene_tracks() && !self.transition.finished {
                    self.transition.stop();
                    self.set_toast("Animation stopped");
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
            _ => {}
        }
        Ok(())
    }

    fn execute_command_str(
        &mut self,
        raw: &str,
        guard: &TerminalGuard,
        _viewport: Rect,
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
            ParsedCommand::Messages => {
                self.customizer.close();
                self.show_help = false;
                self.diagnostics_ui.open_mode(DiagnosticsViewMode::Messages);
            }
            ParsedCommand::Errors => {
                self.customizer.close();
                self.show_help = false;
                self.diagnostics_ui.open_mode(DiagnosticsViewMode::Errors);
            }
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
                        if !self.transition.package_backgrounds().is_empty() {
                            let sz = guard.terminal.size()?;
                            self.effect.set_specs(
                                self.transition.package_backgrounds(),
                                sz.width,
                                sz.height,
                                &self.config.background,
                                &self.transition.package.assets,
                            );
                        }
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!("Playing animation '{}'", self.transition.active_id));
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
                        if !self.transition.package_backgrounds().is_empty() {
                            let sz = guard.terminal.size()?;
                            self.effect.set_specs(
                                self.transition.package_backgrounds(),
                                sz.width,
                                sz.height,
                                &self.config.background,
                                &self.transition.package.assets,
                            );
                        }
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!(
                            "Startup animation set to '{}' (use :write to persist)",
                            self.transition.active_id
                        ));
                    }
                }
                AnimationAction::Pause => {
                    self.transition.pause();
                    self.set_toast("Animation playback paused");
                }
                AnimationAction::Resume => {
                    self.transition.resume();
                    self.set_toast("Animation playback resumed");
                }
                AnimationAction::Stop => {
                    self.transition.stop();
                    self.set_toast("Animation playback stopped");
                }
                AnimationAction::Restart => {
                    self.transition.replay_current();
                    self.set_toast(format!("Restarted '{}'", self.transition.active_id));
                }
                AnimationAction::Speed => {
                    if let Some(val_str) = name
                        && let Ok(spd) = val_str.parse::<f32>()
                    {
                        self.transition.set_speed(spd);
                        self.set_toast(format!(
                            "Animation speed set to {:.2}x",
                            self.transition.speed()
                        ));
                    }
                }
            },
            ParsedCommand::Background(bcmd) => {
                self.execute_background_command(bcmd, guard)?;
            }
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
                LayoutSelection::View(view) => {
                    self.view_mode = view;
                    self.set_toast(format!("View mode: {}", view.label()));
                }
                LayoutSelection::Preset(name) => {
                    let key = name.trim().to_ascii_lowercase();
                    if let Some(preset) = self.layouts_map.get(&key).cloned() {
                        self.config.ui.root = preset.root;
                        if let Some(vm) = preset.view_mode {
                            self.view_mode = vm;
                        }
                        self.set_toast(format!("Applied layout preset '{}'", preset.name));
                    } else {
                        self.set_toast(format!(
                            "Layout preset '{name}' not found in ~/.config/egotop/layouts/"
                        ));
                    }
                }
            },
            ParsedCommand::Container(ccmd) => {
                self.execute_container_command(ccmd);
            }
        }
        Ok(())
    }

    fn execute_container_command(&mut self, cmd: ContainerCommand) {
        match cmd {
            ContainerCommand::List => {
                let mut ids = Vec::new();
                collect_all_container_ids(&self.config.ui.root, &mut ids);
                self.set_toast(format!("Containers: {}", ids.join(", ")));
            }
            ContainerCommand::Inspect(id) => {
                if !has_container_id(&self.config.ui.root, &id) {
                    self.set_toast(format!("Unknown container '{id}'"));
                    return;
                }
                let style = self
                    .config
                    .container_styles
                    .get(&id)
                    .cloned()
                    .unwrap_or_default();
                let grad = style
                    .gradient
                    .as_ref()
                    .map(|g| format!("{:?}({})", g.direction, g.colors.join("→")))
                    .unwrap_or_else(|| "none".to_string());
                self.set_toast(format!(
                    "{id}: bg={:?} fg={} border={} effects={:?} grad={grad}",
                    style.background,
                    style.foreground.as_deref().unwrap_or("inherit"),
                    style.border_color.as_deref().unwrap_or("inherit"),
                    style.effects
                ));
            }
            ContainerCommand::Set { id, property } => {
                if !has_container_id(&self.config.ui.root, &id) {
                    self.set_toast(format!("Unknown container '{id}'"));
                    return;
                }
                let entry: &mut ContainerStyle = self
                    .config
                    .container_styles
                    .entry(id.clone())
                    .or_default();
                match property {
                    ContainerProperty::Background(fill) => {
                        entry.background = fill;
                        entry.gradient = None;
                        self.set_toast(format!("Updated {id}.background"));
                    }
                    ContainerProperty::Foreground(fg) => {
                        entry.foreground = fg;
                        self.set_toast(format!("Updated {id}.foreground"));
                    }
                    ContainerProperty::BorderColor(c) => {
                        entry.border_color = c;
                        self.set_toast(format!("Updated {id}.border.color"));
                    }
                    ContainerProperty::BorderStyle(st) => {
                        entry.border_style = Some(st);
                        self.set_toast(format!("Updated {id}.border.style = {}", st.label()));
                    }
                    ContainerProperty::Effects(policy) => {
                        entry.effects = policy;
                        self.set_toast(format!("Updated {id}.effects = {policy:?}"));
                    }
                }
            }
            ContainerCommand::Gradient {
                id,
                direction,
                colors,
            } => {
                if !has_container_id(&self.config.ui.root, &id) {
                    self.set_toast(format!("Unknown container '{id}'"));
                    return;
                }
                let entry = self
                    .config
                    .container_styles
                    .entry(id.clone())
                    .or_default();
                entry.gradient = Some(ContainerGradient { direction, colors });
                self.set_toast(format!("Applied {direction:?} gradient to '{id}'"));
            }
            ContainerCommand::Reset(id) => {
                if !has_container_id(&self.config.ui.root, &id) {
                    self.set_toast(format!("Unknown container '{id}'"));
                    return;
                }
                self.config.container_styles.remove(&id);
                self.set_toast(format!("Reset style for '{id}'"));
            }
        }
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

        if k == "border.style" || k == "borders.style" {
            match parse_border_style(value) {
                Ok(style) => {
                    self.config.theme.border_type = style;
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
            WriteTarget::Config | WriteTarget::Theme | WriteTarget::All => {
                self.save_main_config();
            }
        }
    }

    fn reload_all(&mut self, guard: &TerminalGuard) -> Result<()> {
        let commit = self.runtime.reload_all();
        self.apply_runtime_commit(&commit, guard)
    }

    fn apply_runtime_commit(&mut self, commit: &RuntimeCommit, guard: &TerminalGuard) -> Result<()> {
        if commit.had_errors {
            self.set_toast(commit.summary.clone());
            return Ok(());
        }
        let sz = guard.terminal.size()?;
        let viewport = Rect::new(0, 0, sz.width, sz.height);
        let mut target = ManagedAppTarget {
            config: &mut self.config,
            theme: &mut self.theme,
            themes_map: &mut self.themes_map,
            theme_list: &mut self.theme_list,
            animations: &mut self.animations,
            layout_names: &mut self.layout_names,
            layouts_map: &mut self.layouts_map,
            background: &mut self.effect,
            transition: &mut self.transition,
            viewport,
        };
        RuntimeResourceManager::apply_commit(commit, self.runtime.state(), &mut target);
        if commit.config_updated
            || commit.themes_updated
            || commit.animations_updated
            || commit.layouts_updated
        {
            self.set_toast(commit.summary.clone());
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
        self.effect = BackgroundPipeline::from_config(&self.config.background, sz.width, sz.height);
        Ok(())
    }

    fn execute_background_command(
        &mut self,
        bcmd: BackgroundCommand,
        guard: &TerminalGuard,
    ) -> Result<()> {
        let sz = guard.terminal.size()?;
        match bcmd {
            BackgroundCommand::Stop => {
                self.effect.clear();
                self.config.background.enabled = false;
                self.config.background.effect = EffectKind::None;
                self.set_toast("Background stopped");
            }
            BackgroundCommand::Pause => {
                self.effect.pause();
                self.set_toast("Background paused");
            }
            BackgroundCommand::Resume => {
                self.effect.resume();
                self.set_toast("Background resumed");
            }
            BackgroundCommand::Speed(val_str) => {
                if let Ok(spd) = val_str.parse::<f32>() {
                    self.effect.set_speed(spd);
                    self.set_toast(format!("Background speed set to {:.2}x", self.effect.speed));
                }
            }
            BackgroundCommand::List => {
                self.set_toast(format!(
                    "Background layers active: {} (matrix_rain, starfield, particles, waves, noise, gradient, floating_chars, scanlines)",
                    self.effect.layer_count()
                ));
            }
            BackgroundCommand::Set(name) => {
                if let Some(kind) = BackgroundLayerKind::parse(&name) {
                    let spec = BackgroundLayerSpec {
                        effect: kind,
                        ..BackgroundLayerSpec::default()
                    };
                    self.config.background.enabled = kind != BackgroundLayerKind::None;
                    self.effect.set_single(
                        spec,
                        sz.width,
                        sz.height,
                        &self.config.background,
                        &BTreeMap::new(),
                    );
                    self.config.background.effect = self.effect.primary_effect_kind();
                    self.set_toast(format!("Background set to '{}'", kind.label()));
                } else {
                    let pkg = self.animations.resolve_package(&name);
                    if !pkg.definition.backgrounds.is_empty() {
                        self.config.background.enabled = true;
                        self.effect.set_specs(
                            &pkg.definition.backgrounds,
                            sz.width,
                            sz.height,
                            &self.config.background,
                            &pkg.assets,
                        );
                        self.set_toast(format!(
                            "Applied {} background layer(s) from '{}'",
                            self.effect.layer_count(),
                            pkg.definition.name
                        ));
                    } else {
                        self.set_toast(format!("Unknown background '{name}'"));
                    }
                }
            }
            BackgroundCommand::Add(name) => {
                if let Some(kind) = BackgroundLayerKind::parse(&name) {
                    let spec = BackgroundLayerSpec {
                        effect: kind,
                        z_index: self.effect.layer_count() as i32,
                        ..BackgroundLayerSpec::default()
                    };
                    self.config.background.enabled = true;
                    self.effect.add_layer(
                        spec,
                        sz.width,
                        sz.height,
                        &self.config.background,
                        &BTreeMap::new(),
                    );
                    self.set_toast(format!(
                        "Added background layer '{}' ({} active)",
                        kind.label(),
                        self.effect.layer_count()
                    ));
                } else {
                    self.set_toast(format!("Unknown background layer '{name}'"));
                }
            }
        }
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
