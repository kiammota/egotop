use crate::animation::{
    AnimationCatalog, BackgroundEffect, BackgroundLayerKind, BackgroundLayerSpec,
    BackgroundPipeline, TransitionPlayer,
};
use crate::command::{
    AnimationAction, BackgroundCommand, BorderCommand, CanvasCommand, IntroCommand, CommandLineState, CommandRegistry,
    CompletionContext, ContainerCommand, ContainerProperty, LayoutSelection, ParsedCommand,
    WriteTarget, parse_command,
};
use crate::config::{
    BorderStyle, Config, EffectKind, StartupMode, StartupTrigger, ThemeEntry, ViewMode,
    save_config_to_path,
};
use crate::lua::LuaAnimationRuntime;
use crate::runtime::{
    ConfigurationRuntime, LayoutPreset, ManagedAppTarget, RuntimeCommit, RuntimePaths,
    RuntimeResourceManager,
};
use crate::system::{SystemCollector, SystemSnapshot};
use crate::terminal::TerminalGuard;
use crate::terminal::diagnostics::TerminalDiagnostics;
use crate::terminal::input::{InputEvent, poll_event};
use crate::terminal::renderer::{CursorStartupState, draw_frame_with_lua};
use crate::ui::borders::parse_border_style;
use crate::ui::customizer::{
    CustomizerState, CustomizerTab, DiagnosticsOverlayState, DiagnosticsViewMode,
    collect_component_ids, layout_items_summary, render_diagnostics_overlay,
    toggle_component_visible,
};
use crate::ui::theme::Theme;
use crate::ui::widgets::RenderContext;
use crate::ui::{
    ComponentAnimationStore, ComponentKind, ContainerBorderConfig, ContainerConfig,
    ContainerGradient, ContainerSize, ContainerStyle, LayoutRegion, RevealState,
    collect_container_names, compute_dashboard_layout, has_container, remove_container,
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
    lua_anim: Option<LuaAnimationRuntime>,
    anim_store: ComponentAnimationStore,
    reveal: RevealState,
    command_line: CommandLineState,
    customizer: CustomizerState,
    diagnostics_ui: DiagnosticsOverlayState,
    term_diagnostics: TerminalDiagnostics,
    runtime: ConfigurationRuntime,
    view_mode: ViewMode,
    paused: bool,
    show_help: bool,
    should_quit: bool,
    fps: f32,
    status_toast: Option<(String, Instant)>,
    command_preview_backup: Option<CommandPreviewBackup>,
}

#[derive(Clone)]
struct CommandPreviewBackup {
    config: Config,
    theme: Theme,
    view_mode: ViewMode,
    transition_active_id: String,
    transition_finished: bool,
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
        let runtime = ConfigurationRuntime::start(
            RuntimePaths::default_with_config(config_path.clone()),
            config,
            true,
        );
        let lkg = runtime.state().clone();
        let mut config = lkg.config;
        let themes_map = lkg.themes;
        let mut theme = resolve_active_theme(&config, &themes_map);
        let term_diagnostics = TerminalDiagnostics::detect(width, height);
        if term_diagnostics.should_use_ascii_fallback() {
            config.typography.ascii_fallback = true;
        }
        if term_diagnostics.should_adapt_colors() {
            adapt_theme_colors(&mut theme);
        }
        let theme_list: Vec<ThemeEntry> = themes_map.values().cloned().collect();

        let animations = lkg.animations;
        let layouts_map = lkg.layouts;
        let layout_names: Vec<String> = layouts_map.keys().cloned().collect();
        let mut container_ids = Vec::new();
        collect_container_names(&config.ui.containers, &mut container_ids);

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

        let lua_bundle = if config_path.extension().and_then(|e| e.to_str()) == Some("lua")
            && config_path.is_file()
        {
            crate::lua::load_lua_config_from_path(&config_path).ok()
        } else {
            None
        };
        let lua_specs = lua_bundle
            .as_ref()
            .map(|b| b.effect_specs.clone())
            .unwrap_or_default();
        let lua_anim = if !wait_init
            && !config.general.startup_animation.trim().is_empty()
            && !config
                .general
                .startup_animation
                .eq_ignore_ascii_case("none")
        {
            Some(LuaAnimationRuntime::start(
                &config.general.startup_animation,
                width,
                height,
                config.general.animation_fps,
            ))
        } else {
            None
        };

        let mut collector = SystemCollector::new();
        let snapshot = collector.collect(width, height);
        let effect = BackgroundPipeline::from_config_and_lua(
            &config.background,
            &lua_specs,
            width,
            height,
            Some(&theme),
        );
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
            lua_anim,
            anim_store: ComponentAnimationStore::new(),
            reveal: if is_cursor_startup {
                RevealState::complete()
            } else {
                RevealState::new(0.65)
            },
            command_line: CommandLineState::default(),
            customizer: CustomizerState::default(),
            diagnostics_ui: DiagnosticsOverlayState::default(),
            term_diagnostics: term_diagnostics.clone(),
            runtime,
            view_mode,
            paused: false,
            show_help: false,
            should_quit: false,
            fps,
            status_toast: initial_toast
                .or_else(|| {
                    (term_diagnostics.warning_count() + term_diagnostics.error_count() > 0)
                        .then(|| term_diagnostics.summary())
                })
                .map(|m| (m, Instant::now() + Duration::from_secs(4))),
            command_preview_backup: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut guard = TerminalGuard::enter()?;
        let initial_size = guard.terminal.size()?;
        self.effect
            .resize(initial_size.width, initial_size.height, &self.config.background);
        if let Some(la) = &mut self.lua_anim {
            la.resize(initial_size.width, initial_size.height);
        }

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
                if let Some(la) = &mut self.lua_anim {
                    la.update(dt);
                    self.reveal.progress = la.reveal_progress();
                    if la.is_finished() {
                        self.reveal = RevealState::complete();
                        self.lua_anim = None;
                    }
                } else {
                    self.reveal.advance(dt);
                }
            }

            let toast_text = self.status_toast.as_ref().map(|(s, _)| s.as_str());
            let transparent_bg = self.config.background.transparent
                || self.theme.background == ratatui::style::Color::Reset;
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
            let lua_anim_ref = self.lua_anim.as_ref();
            let anim_store_ref = &self.anim_store;
            let customizer_ref = &mut self.customizer;
            let diagnostics_ui_ref = &mut self.diagnostics_ui;
            let diagnostics_ref = self.runtime.diagnostics();
            let cmd_ref = &self.command_line;
            let startup_ref = &self.startup_cursor;
            let themes_ref = &self.theme_list;
            let anim_ref = &self.animations;

            guard.terminal.draw(|frame| {
                draw_frame_with_lua(
                    frame,
                    config_ref,
                    effect_ref,
                    transition_ref,
                    lua_anim_ref,
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
                if let Some(la) = &mut self.lua_anim {
                    la.resize(w, h);
                }
                self.snapshot = self.collector.collect(w, h);
                let layout = compute_dashboard_layout(
                    Rect::new(0, 0, w, h),
                    &self.config,
                    self.view_mode,
                );
                self.anim_store
                    .update_all(0.0, &layout, &self.config, &self.animations);
            }
            InputEvent::Mouse(mouse) => {
                self.handle_mouse(mouse, viewport, guard)?;
            }
            InputEvent::Key(key) => {
                if self.lua_anim.is_some()
                    && matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' '))
                    && !self.command_line.active
                {
                    if let Some(anim) = &mut self.lua_anim { anim.stop(); }
                    self.lua_anim = None;
                    self.reveal = RevealState::complete();
                    self.set_toast("Intro skipped");
                    return Ok(());
                }
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
        let mut preview_cmd = false;
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
                self.command_preview_backup = None;
                let cmd = cmd_state.submit();
                if !cmd.is_empty() {
                    execute_str = Some(cmd);
                }
            }
            KeyCode::Tab => {
                cmd_state.next_suggestion(&self.registry, &ctx);
                preview_cmd = true;
            }
            KeyCode::BackTab => {
                cmd_state.prev_suggestion(&self.registry, &ctx);
                preview_cmd = true;
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
        if restore_preview {
            self.restore_command_preview(viewport);
        }
        if preview_cmd {
            self.maybe_preview_command_from_cmdline(viewport);
        }
        if let Some(cmd) = execute_str {
            self.execute_command_str(&cmd, guard, viewport)?;
        }
        Ok(())
    }

    fn is_previewable_ui_command(parsed: &ParsedCommand) -> bool {
        match parsed {
            ParsedCommand::Colorscheme(Some(_))
            | ParsedCommand::Set { .. }
            | ParsedCommand::Border(_)
            | ParsedCommand::Layout(Some(_)) => true,
            ParsedCommand::Background(bcmd) => matches!(
                bcmd,
                BackgroundCommand::Set(_)
                    | BackgroundCommand::Stop
                    | BackgroundCommand::Pause
                    | BackgroundCommand::Resume
                    | BackgroundCommand::Toggle(_)
                    | BackgroundCommand::Speed(_)
            ),
            ParsedCommand::Animation { action, .. } => matches!(
                action,
                AnimationAction::Play
                    | AnimationAction::SetStartup
                    | AnimationAction::Pause
                    | AnimationAction::Resume
                    | AnimationAction::Stop
                    | AnimationAction::Speed
            ),
            ParsedCommand::Container(ccmd) => matches!(
                ccmd,
                ContainerCommand::Set { .. }
                    | ContainerCommand::Gradient { .. }
                    | ContainerCommand::Reset(_)
            ),
            _ => false,
        }
    }

    fn restore_from_backup(&mut self, backup: &CommandPreviewBackup, viewport: Rect) {
        let bg_changed = self.config.background != backup.config.background;
        self.config = backup.config.clone();
        self.theme = backup.theme.clone();
        self.view_mode = backup.view_mode;
        self.status_toast = backup.status_toast.clone();
        if bg_changed {
            self.effect = BackgroundPipeline::from_config_and_lua(
                &self.config.background,
                &[],
                viewport.width,
                viewport.height,
                Some(&self.theme),
            );
        }
        if self.transition.active_id != backup.transition_active_id
            || self.transition.finished != backup.transition_finished
        {
            if backup.transition_finished {
                self.transition.stop();
            } else {
                self.transition.restart(
                    &backup.transition_active_id,
                    &self.animations,
                    self.config.general.startup_duration_ms,
                );
            }
        }
    }

    fn restore_command_preview(&mut self, viewport: Rect) {
        if let Some(backup) = self.command_preview_backup.take() {
            self.restore_from_backup(&backup, viewport);
        }
    }

    fn maybe_preview_command_from_cmdline(&mut self, viewport: Rect) {
        let Ok(parsed) = parse_command(&self.command_line.buffer) else {
            return;
        };
        if !Self::is_previewable_ui_command(&parsed) {
            return;
        }
        if let Some(backup) = self.command_preview_backup.clone() {
            self.restore_from_backup(&backup, viewport);
        } else {
            self.command_preview_backup = Some(CommandPreviewBackup {
                config: self.config.clone(),
                theme: self.theme.clone(),
                view_mode: self.view_mode,
                transition_active_id: self.transition.active_id.clone(),
                transition_finished: self.transition.finished,
                status_toast: self.status_toast.clone(),
            });
        }
        self.apply_preview_command(parsed, viewport);
    }

    fn apply_preview_command(&mut self, parsed: ParsedCommand, viewport: Rect) {
        match parsed {
            ParsedCommand::Colorscheme(Some(name)) => {
                let key = name.trim().to_ascii_lowercase();
                if let Some(entry) = self.themes_map.get(&key).cloned() {
                    self.apply_theme_entry(&entry);
                }
            }
            ParsedCommand::Set { key, value } => {
                self.execute_set_option(&key, &value);
            }
            ParsedCommand::Border(bcmd) => {
                self.execute_border_command(bcmd);
            }
            ParsedCommand::Layout(Some(sel)) => match sel {
                LayoutSelection::View(view) => self.view_mode = view,
                LayoutSelection::Preset(name) => {
                    let key = name.trim().to_ascii_lowercase();
                    if let Some(preset) = self.layouts_map.get(&key).cloned() {
                        self.config.ui = preset.ui;
                        if let Some(vm) = preset.view_mode {
                            self.view_mode = vm;
                        }
                    }
                }
            },
            ParsedCommand::Background(bcmd) => {
                self.execute_background_command_with_size(bcmd, viewport.width, viewport.height);
            }
            ParsedCommand::Animation { action, name } => match action {
                AnimationAction::Play | AnimationAction::SetStartup => {
                    if let Some(anim_name) = name {
                        if action == AnimationAction::SetStartup {
                            self.config.general.startup_animation = anim_name.clone();
                        }
                        self.transition.restart(
                            &anim_name,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        if !self.transition.package_backgrounds().is_empty() {
                            self.effect.set_specs(
                                self.transition.package_backgrounds(),
                                viewport.width,
                                viewport.height,
                                &self.config.background,
                                &self.transition.package.assets,
                            );
                        }
                        self.reveal = RevealState::new(0.65);
                    }
                }
                AnimationAction::Pause => self.transition.pause(),
                AnimationAction::Resume => self.transition.resume(),
                AnimationAction::Stop => self.transition.stop(),
                AnimationAction::Speed => {
                    if let Some(val_str) = name
                        && let Ok(spd) = val_str.parse::<f32>()
                    {
                        self.transition.set_speed(spd);
                    }
                }
                _ => {}
            },
            ParsedCommand::Container(ccmd) => {
                self.execute_container_command(ccmd);
            }
            _ => {}
        }
        // Keep status_toast clean during live preview
        self.status_toast = self
            .command_preview_backup
            .as_ref()
            .and_then(|b| b.status_toast.clone());
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
                last if last == layout_items_summary(&self.config).len() - 1 => {
                    self.config.general.view_mode = self.view_mode;
                    match save_config_to_path(&self.config_path, &self.config) {
                        Ok(()) => self.set_toast("Saved configuration to egotop.toml"),
                        Err(e) => self.set_toast(format!("Save error: {e}")),
                    }
                }
                comp_idx => {
                    let mut comps = Vec::new();
                    collect_component_ids(&self.config.ui.containers, &mut comps);
                    if let Some((name, _)) = comps.get(comp_idx.saturating_sub(1)) {
                        let name = name.clone();
                        if let Some(now_on) =
                            toggle_component_visible(&mut self.config.ui.containers, &name)
                        {
                            self.set_toast(format!(
                                "{name}: {}",
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
            ParsedCommand::WriteQuit => {
                self.save_main_config();
                self.should_quit = true;
            }
            ParsedCommand::Help => self.show_help = true,
            ParsedCommand::Reload => self.reload_all(guard)?,
            ParsedCommand::Restore => self.restore_default_config(guard)?,
            ParsedCommand::Check => {
                let sz = guard.terminal.size()?;
                let diag = TerminalDiagnostics::detect(sz.width, sz.height);
                if diag.should_use_ascii_fallback() {
                    self.config.typography.ascii_fallback = true;
                }
                if diag.should_adapt_colors() {
                    adapt_theme_colors(&mut self.theme);
                }
                self.set_toast(diag.summary());
                self.term_diagnostics = diag;
            }
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
            ParsedCommand::Border(bcmd) => {
                self.execute_border_command(bcmd);
            }
            ParsedCommand::Animation { action, name } => match action {
                AnimationAction::OpenPicker => {
                    self.customizer.open_tab(CustomizerTab::Animations);
                }
                AnimationAction::Play => {
                    if let Some(anim_name) = name {
                        let sz = guard.terminal.size()?;
                        self.lua_anim = Some(LuaAnimationRuntime::start(
                            &anim_name,
                            sz.width,
                            sz.height,
                            self.config.general.animation_fps,
                        ));
                        self.transition.restart(
                            &anim_name,
                            &self.animations,
                            self.config.general.startup_duration_ms,
                        );
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!("Playing Lua animation '{anim_name}'"));
                    }
                }
                AnimationAction::SetStartup => {
                    if let Some(anim_name) = name {
                        self.config.general.startup_animation = anim_name.clone();
                        let sz = guard.terminal.size()?;
                        self.lua_anim = Some(LuaAnimationRuntime::start(
                            &anim_name,
                            sz.width,
                            sz.height,
                            self.config.general.animation_fps,
                        ));
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
                AnimationAction::Pause => {
                    self.transition.pause();
                    if let Some(la) = &mut self.lua_anim {
                        la.pause();
                    }
                    self.set_toast("Animation playback paused");
                }
                AnimationAction::Resume => {
                    self.transition.resume();
                    if let Some(la) = &mut self.lua_anim {
                        la.resume();
                    }
                    self.set_toast("Animation playback resumed");
                }
                AnimationAction::Stop => {
                    self.transition.stop();
                    if let Some(la) = &mut self.lua_anim {
                        la.stop();
                    }
                    self.lua_anim = None;
                    self.reveal = RevealState::complete();
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
            ParsedCommand::Intro(cmd) => match cmd {
                IntroCommand::Off => {
                    self.lua_anim = None;
                    self.transition.stop();
                    self.reveal = RevealState::complete();
                    self.config.general.startup_animation = "none".into();
                    self.set_toast("Intro disabled");
                }
                IntroCommand::Replay => {
                    let name = self.config.general.startup_animation.clone();
                    if name.eq_ignore_ascii_case("none") || name.is_empty() {
                        self.set_toast("No intro configured");
                    } else {
                        let sz = guard.terminal.size()?;
                        self.lua_anim = Some(LuaAnimationRuntime::start(&name, sz.width, sz.height, self.config.general.animation_fps));
                        self.reveal = RevealState::new(0.65);
                        self.set_toast(format!("Replaying intro '{name}'"));
                    }
                }
                IntroCommand::Play(name) | IntroCommand::Set(name) => {
                    match crate::lua::validate_script_for_slot(&name, crate::animation::AnimSlotKind::Intro) {
                        Ok(()) => {
                            self.config.general.startup_animation = name.clone();
                            let sz = guard.terminal.size()?;
                            self.lua_anim = Some(LuaAnimationRuntime::start(&name, sz.width, sz.height, self.config.general.animation_fps));
                            self.reveal = RevealState::new(0.65);
                            self.set_toast(format!("Playing intro '{name}'"));
                        }
                        Err(e) => self.set_toast(e),
                    }
                }
            },
            ParsedCommand::Canvas(cmd) => {
                let (id, script) = match cmd {
                    CanvasCommand::Set { container_id, script } => (container_id, Some(script)),
                    CanvasCommand::Off { container_id } => (container_id, None),
                    CanvasCommand::Pause { container_id } | CanvasCommand::Resume { container_id } => {
                        self.set_toast("Canvas pause/resume is not available for component animations yet");
                        (container_id, None)
                    }
                };
                if !has_container(&self.config.ui.containers, &id) {
                    self.set_toast(format!("Unknown container '{id}'"));
                } else if let Some(script) = script {
                    match crate::lua::validate_script_for_slot(&script, crate::animation::AnimSlotKind::Canvas) {
                        Ok(()) => {
                            if let Some(c) = self.config.ui.containers.iter_mut().find(|c| c.name == id) {
                                c.kind = ComponentKind::Canvas;
                                c.content.script = Some(script.clone());
                            }
                            self.set_toast(format!("Canvas '{script}' assigned to '{id}' (layout reload required)"));
                        }
                        Err(e) => self.set_toast(e),
                    }
                } else if let Some(c) = self.config.ui.containers.iter_mut().find(|c| c.name == id) {
                    c.content.script = None;
                    self.set_toast(format!("Canvas animation cleared from '{id}'"));
                }
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
                        self.config.ui = preset.ui;
                        if let Some(vm) = preset.view_mode {
                            self.view_mode = vm;
                        }
                        self.refresh_container_ids();
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
                let mut names = Vec::new();
                collect_container_names(&self.config.ui.containers, &mut names);
                self.set_toast(format!("Containers: {}", names.join(", ")));
            }
            ContainerCommand::Inspect(name) => {
                if !has_container(&self.config.ui.containers, &name) {
                    self.set_toast(format!("Unknown container '{name}'"));
                    return;
                }
                let style = self
                    .config
                    .container_styles
                    .get(&name)
                    .cloned()
                    .unwrap_or_default();
                let grad = style
                    .gradient
                    .as_ref()
                    .map(|g| format!("{:?}({})", g.direction, g.colors.join("→")))
                    .unwrap_or_else(|| "none".to_string());
                self.set_toast(format!(
                    "{name}: bg={:?} fg={} border={} effects={:?} grad={grad}",
                    style.background,
                    style.foreground.as_deref().unwrap_or("inherit"),
                    style.border_color.as_deref().unwrap_or("inherit"),
                    style.effects
                ));
            }
            ContainerCommand::Add { id, kind } => {
                let clean_name = id.trim().to_string();
                if clean_name.is_empty() {
                    self.set_toast("Container name cannot be empty");
                    return;
                }
                if has_container(&self.config.ui.containers, &clean_name) {
                    self.set_toast(format!("Container '{clean_name}' already exists"));
                    return;
                }
                let comp_kind = kind.unwrap_or_else(|| {
                    crate::command::parser::parse_component_kind(&clean_name)
                        .unwrap_or(ComponentKind::Text)
                });
                let new_container = ContainerConfig {
                    name: clean_name.clone(),
                    description: format!("{} container", comp_kind.label()),
                    visible: true,
                    region: LayoutRegion::Main,
                    category: comp_kind.expected_category(),
                    kind: comp_kind,
                    width: ContainerSize::Auto,
                    height: ContainerSize::Fill(1),
                    min_height: Some(6),
                    padding: 1,
                    border: ContainerBorderConfig::titled(clean_name.clone()),
                    ..ContainerConfig::default()
                };
                let insert_pos = self
                    .config
                    .ui
                    .containers
                    .iter()
                    .position(|c| c.region == LayoutRegion::Bottom || c.name == "status_spinner")
                    .unwrap_or(self.config.ui.containers.len());
                self.config.ui.containers.insert(insert_pos, new_container);
                self.refresh_container_ids();
                match save_config_to_path(&self.config_path, &self.config) {
                    Ok(()) => self.set_toast(format!("Added container '{clean_name}' and saved")),
                    Err(e) => self.set_toast(format!("Added '{clean_name}' (save error: {e})")),
                }
            }
            ContainerCommand::Remove(name) => {
                if !remove_container(&mut self.config.ui.containers, &name) {
                    self.set_toast(format!("Unknown container '{name}'"));
                    return;
                }
                self.config.container_styles.remove(&name);
                self.refresh_container_ids();
                match save_config_to_path(&self.config_path, &self.config) {
                    Ok(()) => self.set_toast(format!("Removed container '{name}' and saved")),
                    Err(e) => self.set_toast(format!("Removed '{name}' (save error: {e})")),
                }
            }
            ContainerCommand::Set { id, property } => {
                if !has_container(&self.config.ui.containers, &id) {
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
                        if let Some(c) = self
                            .config
                            .ui
                            .containers
                            .iter_mut()
                            .find(|c| c.name == id)
                        {
                            c.border.enabled = st != BorderStyle::None;
                            c.border.style = Some(st);
                        }
                        self.set_toast(format!("Updated {id}.border.style = {}", st.label()));
                    }
                    ContainerProperty::Effects(policy) => {
                        entry.effects = policy;
                        self.set_toast(format!("Updated {id}.effects = {policy:?}"));
                    }
                    ContainerProperty::Animation(anim_opt) => {
                        if let Some(ref anim_name) = anim_opt {
                            if crate::lua::resolve_canvas_module_name(anim_name).is_none() {
                                self.set_toast(format!(
                                    "Unknown canvas animation '{anim_name}'"
                                ));
                                return;
                            }
                            entry.animation = Some(anim_name.clone());
                            self.set_toast(format!(
                                "Assigned animation '{anim_name}' to '{id}' canvas"
                            ));
                        } else {
                            entry.animation = Some("none".to_string());
                            self.set_toast(format!("Cleared animation on '{id}' canvas"));
                        }
                    }
                }
            }
            ContainerCommand::Gradient {
                id,
                direction,
                colors,
            } => {
                if !has_container(&self.config.ui.containers, &id) {
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
                if !has_container(&self.config.ui.containers, &id) {
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
        if k == "background.transparent" {
            match parse_bool_flag(value) {
                Some(flag) => {
                    self.config.background.transparent = flag;
                    self.set_toast(format!(
                        "Background transparency set to {}",
                        if flag { "transparent" } else { "solid" }
                    ));
                }
                None => self.set_toast(format!(
                    "Invalid boolean '{value}' (expected true or false)"
                )),
            }
            return;
        }

        if k == "containers.transparent" {
            match parse_bool_flag(value) {
                Some(true) => {
                    self.config.theme.panel_background = "transparent".to_string();
                    self.theme.panel_background = ratatui::style::Color::Reset;
                    self.set_toast("All containers set to transparent (individual overrides preserved)");
                }
                Some(false) => {
                    let fallback = crate::ui::theme::preset_theme(&self.config.theme.name).panel_background;
                    let resolved = if fallback == ratatui::style::Color::Reset {
                        ratatui::style::Color::Rgb(8, 17, 12)
                    } else {
                        fallback
                    };
                    self.config.theme.panel_background.clear();
                    self.theme.panel_background = resolved;
                    self.set_toast("All containers set to solid (individual overrides preserved)");
                }
                None => self.set_toast(format!(
                    "Invalid boolean '{value}' (expected true or false)"
                )),
            }
            return;
        }

        if k == "containers.background" {
            let trimmed = value.trim();
            if trimmed.eq_ignore_ascii_case("transparent") || trimmed.eq_ignore_ascii_case("reset") {
                self.config.theme.panel_background = "transparent".to_string();
                self.theme.panel_background = ratatui::style::Color::Reset;
                self.set_toast("All containers background set to transparent");
            } else {
                let parsed = crate::ui::theme::parse_color(trimmed, self.theme.panel_background);
                self.config.theme.panel_background = trimmed.to_string();
                self.theme.panel_background = parsed;
                self.set_toast(format!("All containers background set to {trimmed}"));
            }
            return;
        }

        if k == "background" {
            match value.trim().to_ascii_lowercase().as_str() {
                "transparent" | "reset" => {
                    self.config.background.transparent = true;
                    self.set_toast("Background set to transparent (runtime)");
                }
                "solid" => {
                    self.config.background.transparent = false;
                    self.set_toast("Background set to solid (runtime)");
                }
                "dark" => {
                    self.config.background.transparent = false;
                    self.theme.set_brightness_mode(true);
                    self.set_toast("Background set to dark (runtime)");
                }
                "light" => {
                    self.config.background.transparent = false;
                    self.theme.set_brightness_mode(false);
                    self.set_toast("Background set to light (runtime)");
                }
                other => self.set_toast(format!(
                    "Invalid background value '{other}' (expected transparent, solid, dark, or light)"
                )),
            }
            return;
        }

        if k == "border.style" || k == "borders.style" {
            match parse_border_style(value) {
                Ok(style) => {
                    self.apply_global_border_style(style);
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

    fn apply_global_border_style(&mut self, style: BorderStyle) {
        self.config.theme.border_type = style;
        self.theme.set_border_style(style);
        self.config.theme.borders = self.theme.borders.to_config();
        for st in self.config.container_styles.values_mut() {
            st.border_style = None;
        }
        for c in &mut self.config.ui.containers {
            if c.border.enabled || c.border.style.is_some() {
                c.border.style = if style == BorderStyle::None {
                    None
                } else {
                    Some(style)
                };
            }
        }
        if self.config.background.border.enabled {
            self.config.background.border.style = Some(style);
        }
    }

    fn execute_border_command(&mut self, cmd: BorderCommand) {
        match cmd {
            BorderCommand::GlobalStyle(style) => {
                self.apply_global_border_style(style);
                self.set_toast(format!("Border style set to '{}'", style.label()));
            }
            BorderCommand::TargetStyle { id, style } => {
                if id.eq_ignore_ascii_case("background") {
                    self.config.background.border.enabled = style != BorderStyle::None;
                    self.config.background.border.style = Some(style);
                    self.set_toast(format!("Updated background.border.style = {}", style.label()));
                } else if has_container(&self.config.ui.containers, &id) {
                    let entry = self
                        .config
                        .container_styles
                        .entry(id.clone())
                        .or_default();
                    entry.border_style = Some(style);
                    if let Some(c) = self.config.ui.containers.iter_mut().find(|c| c.name == id) {
                        c.border.enabled = style != BorderStyle::None;
                        c.border.style = Some(style);
                    }
                    self.set_toast(format!("Updated {id}.border.style = {}", style.label()));
                } else {
                    self.set_toast(format!("Unknown canvas '{id}'"));
                }
            }
            BorderCommand::TargetColor { id, color } => {
                if id.eq_ignore_ascii_case("background") {
                    self.config.background.border.color = color;
                    self.set_toast("Updated background.border.color");
                } else if has_container(&self.config.ui.containers, &id) {
                    let entry = self
                        .config
                        .container_styles
                        .entry(id.clone())
                        .or_default();
                    entry.border_color = color.clone();
                    if let Some(c) = self.config.ui.containers.iter_mut().find(|c| c.name == id) {
                        c.border.color = color;
                    }
                    self.set_toast(format!("Updated {id}.border.color"));
                } else {
                    self.set_toast(format!("Unknown canvas '{id}'"));
                }
            }
            BorderCommand::CharProp { prop, value } => {
                self.execute_set_option(&format!("border.{prop}"), &value);
            }
        }
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

    fn refresh_container_ids(&mut self) {
        self.container_ids.clear();
        collect_container_names(&self.config.ui.containers, &mut self.container_ids);
    }

    fn restore_default_config(&mut self, guard: &TerminalGuard) -> Result<()> {
        self.config = Config::default();
        self.theme = Theme::from_config(&self.config.theme);
        self.view_mode = self.config.general.view_mode;
        self.refresh_container_ids();
        let sz = guard.terminal.size()?;
        self.effect =
            BackgroundPipeline::from_config(&self.config.background, sz.width, sz.height);
        match save_config_to_path(&self.config_path, &self.config) {
            Ok(()) => self.set_toast("Restored official default configuration"),
            Err(e) => self.set_toast(format!("Restored defaults (save error: {e})")),
        }
        Ok(())
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
        self.effect.set_theme(&self.theme);
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

    fn execute_background_command(
        &mut self,
        bcmd: BackgroundCommand,
        guard: &TerminalGuard,
    ) -> Result<()> {
        let sz = guard.terminal.size()?;
        self.execute_background_command_with_size(bcmd, sz.width, sz.height);
        Ok(())
    }

    fn execute_background_command_with_size(
        &mut self,
        bcmd: BackgroundCommand,
        width: u16,
        height: u16,
    ) {
        match bcmd {
            BackgroundCommand::Stop => {
                self.effect.clear();
                self.config.background.enabled = false;
                self.config.background.effect = EffectKind::None;
                self.config.background.layers.clear();
                self.config.background.preset = None;
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
            BackgroundCommand::Toggle(opt) => {
                let target = opt.unwrap_or(!self.config.background.enabled);
                self.config.background.enabled = target;
                if target {
                    if self.effect.layer_count() == 0 {
                        let fallback = self
                            .config
                            .background
                            .preset
                            .clone()
                            .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("none"))
                            .or_else(|| {
                                (self.config.background.effect != EffectKind::None)
                                    .then(|| self.config.background.effect.label().to_ascii_lowercase())
                            })
                            .or_else(|| crate::lua::list_all_canvas_animations().into_iter().next())
                            .unwrap_or_else(|| "particles".to_string());
                        let kind = BackgroundLayerKind::parse(&fallback)
                            .unwrap_or(BackgroundLayerKind::CustomAscii);
                        let spec = BackgroundLayerSpec {
                            effect: kind,
                            asset: Some(fallback.clone()),
                            ..BackgroundLayerSpec::default()
                        };
                        self.config.background.preset = Some(fallback);
                        self.config.background.layers = vec![spec.clone()];
                        self.effect.set_single_with_theme(
                            spec,
                            width,
                            height,
                            &self.config.background,
                            &BTreeMap::new(),
                            Some(&self.theme),
                        );
                        self.config.background.effect = self.effect.primary_effect_kind();
                    }
                    self.effect.resume();
                    self.set_toast("Background enabled");
                } else {
                    self.effect.pause();
                    self.set_toast("Background disabled");
                }
            }
            BackgroundCommand::Remove(target) => {
                let removed = self.effect.remove_layer(&target);
                if removed {
                    if let Ok(idx) = target.parse::<usize>() {
                        if idx > 0 && idx <= self.config.background.layers.len() {
                            self.config.background.layers.remove(idx - 1);
                        }
                    } else {
                        let target_clean = crate::lua::normalize_animation_alias(&target);
                        if let Some(pos) = self.config.background.layers.iter().position(|l| {
                            l.asset.as_deref().is_some_and(|a| {
                                crate::lua::normalize_animation_alias(a) == target_clean
                            }) || crate::lua::normalize_animation_alias(l.effect.label())
                                == target_clean
                        }) {
                            self.config.background.layers.remove(pos);
                        }
                    }
                    self.config.background.enabled = self.effect.layer_count() > 0;
                    if !self.config.background.enabled {
                        self.config.background.preset = None;
                        self.config.background.effect = EffectKind::None;
                    } else {
                        self.config.background.preset = self
                            .config
                            .background
                            .layers
                            .first()
                            .and_then(|l| l.asset.clone().or_else(|| Some(l.effect.label().to_string())));
                        self.config.background.effect = self.effect.primary_effect_kind();
                    }
                    self.set_toast(format!(
                        "Removed background layer '{target}' ({} active)",
                        self.effect.layer_count()
                    ));
                } else {
                    self.set_toast(format!("Background layer '{target}' not found"));
                }
            }
            BackgroundCommand::Speed(val_str) => {
                if let Ok(spd) = val_str.parse::<f32>() {
                    let clamped = spd.clamp(0.1, 10.0);
                    self.effect.set_speed(clamped);
                    self.config.background.speed = clamped;
                    self.set_toast(format!("Background speed set to {:.2}x", self.effect.speed));
                }
            }
            BackgroundCommand::List => {
                let avail = crate::lua::list_all_canvas_animations();
                self.set_toast(format!(
                    "Background layers active: {} (available: {})",
                    self.effect.layer_count(),
                    avail.join(", ")
                ));
            }
            BackgroundCommand::Set(name) => {
                let norm = name.trim().to_ascii_lowercase();
                if matches!(norm.as_str(), "none" | "off") {
                    self.effect.clear();
                    self.config.background.enabled = false;
                    self.config.background.preset = None;
                    self.config.background.layers.clear();
                    self.config.background.effect = EffectKind::None;
                    self.set_toast("Background set to 'none'");
                    return;
                }

                let lua_resolved = crate::lua::resolve_canvas_module_name(&name);
                let parsed_kind = BackgroundLayerKind::parse(&name);
                if lua_resolved.is_some() || parsed_kind.is_some() {
                    let kind = parsed_kind.unwrap_or(BackgroundLayerKind::CustomAscii);
                    let canon = crate::lua::normalize_animation_alias(&name);
                    let spec = BackgroundLayerSpec {
                        effect: kind,
                        speed: self.config.background.speed,
                        density: self.config.background.density,
                        asset: Some(canon.clone()),
                        ..BackgroundLayerSpec::default()
                    };
                    self.config.background.enabled = true;
                    self.config.background.preset = Some(canon.clone());
                    self.config.background.layers = vec![spec.clone()];
                    self.effect.set_single_with_theme(
                        spec,
                        width,
                        height,
                        &self.config.background,
                        &BTreeMap::new(),
                        Some(&self.theme),
                    );
                    self.config.background.effect = self.effect.primary_effect_kind();
                    self.set_toast(format!("Background set to '{canon}'"));
                } else {
                    let pkg = self.animations.resolve_package(&name);
                    if !pkg.definition.backgrounds.is_empty() {
                        self.config.background.enabled = true;
                        self.config.background.preset = Some(pkg.definition.name.clone());
                        self.config.background.layers = pkg.definition.backgrounds.clone();
                        self.effect.set_specs(
                            &pkg.definition.backgrounds,
                            width,
                            height,
                            &self.config.background,
                            &pkg.assets,
                        );
                        self.config.background.effect = self.effect.primary_effect_kind();
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
                let lua_resolved = crate::lua::resolve_canvas_module_name(&name);
                let parsed_kind = BackgroundLayerKind::parse(&name);
                if lua_resolved.is_some() || parsed_kind.is_some() {
                    let kind = parsed_kind.unwrap_or(BackgroundLayerKind::CustomAscii);
                    if kind == BackgroundLayerKind::None {
                        self.set_toast("Use ':background stop' to disable background layers");
                        return;
                    }
                    let canon = crate::lua::normalize_animation_alias(&name);
                    let spec = BackgroundLayerSpec {
                        effect: kind,
                        z_index: self.effect.layer_count() as i32,
                        speed: self.config.background.speed,
                        density: self.config.background.density,
                        asset: Some(canon.clone()),
                        ..BackgroundLayerSpec::default()
                    };
                    self.config.background.enabled = true;
                    if self.config.background.preset.is_none() {
                        self.config.background.preset = Some(canon.clone());
                    }
                    self.config.background.layers.push(spec.clone());
                    self.effect.add_layer_with_theme(
                        spec,
                        width,
                        height,
                        &self.config.background,
                        &BTreeMap::new(),
                        Some(&self.theme),
                    );
                    self.config.background.effect = self.effect.primary_effect_kind();
                    self.set_toast(format!(
                        "Added background layer '{canon}' ({} active)",
                        self.effect.layer_count()
                    ));
                } else {
                    self.set_toast(format!("Unknown background layer '{name}'"));
                }
            }
        }
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.status_toast = Some((msg.into(), Instant::now() + Duration::from_secs(3)));
    }
}

fn parse_bool_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn adapt_theme_colors(theme: &mut Theme) {
    use crate::ui::theme::adapt_color;
    theme.primary = adapt_color(theme.primary, false);
    theme.secondary = adapt_color(theme.secondary, false);
    theme.accent = adapt_color(theme.accent, false);
    theme.text = adapt_color(theme.text, false);
    theme.muted = adapt_color(theme.muted, false);
    theme.border = adapt_color(theme.border, false);
    theme.danger = adapt_color(theme.danger, false);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BorderStyle;
    use crate::ui::BackgroundFill;

    fn test_app() -> App {
        let tmp = std::env::temp_dir().join(format!(
            "egotop_preview_test_{}.toml",
            std::process::id()
        ));
        App::new(Config::default(), tmp, None, 100, 36)
    }

    #[test]
    fn command_tab_live_preview_and_esc_restore_for_ui_commands() {
        let mut app = test_app();
        let vp = Rect::new(0, 0, 100, 36);

        // 1. :background <Tab> previews effect and Esc restores original background
        let orig_layers_count = app.effect.layer_count();
        let mut cmd = std::mem::take(&mut app.command_line);
        {
            let ctx = app.completion_ctx();
            cmd.open(&app.registry, &ctx);
            for ch in "background set particles".chars() {
                cmd.insert_char(ch, &app.registry, &ctx);
            }
        }
        app.command_line = cmd;
        app.maybe_preview_command_from_cmdline(vp);
        assert!(app.config.background.enabled);
        app.restore_command_preview(vp);
        assert_eq!(app.effect.layer_count(), orig_layers_count);

        // 2. :layout <Tab> previews view mode and Esc restores original view mode
        let orig_view = app.view_mode;
        let mut cmd = std::mem::take(&mut app.command_line);
        {
            let ctx = app.completion_ctx();
            cmd.open(&app.registry, &ctx);
            for ch in "layout c".chars() {
                cmd.insert_char(ch, &app.registry, &ctx);
            }
            cmd.next_suggestion(&app.registry, &ctx);
            cmd.next_suggestion(&app.registry, &ctx);
        }
        app.command_line = cmd;
        app.maybe_preview_command_from_cmdline(vp);
        assert_eq!(app.view_mode, ViewMode::Compact);
        app.restore_command_preview(vp);
        assert_eq!(app.view_mode, orig_view);

        // 3. :set border.style <Tab> previews border style and Esc restores
        let orig_border = app.config.theme.border_type;
        app.command_line.buffer = "set border.style none".to_string();
        app.maybe_preview_command_from_cmdline(vp);
        assert_eq!(app.config.theme.border_type, BorderStyle::None);
        app.restore_command_preview(vp);
        assert_eq!(app.config.theme.border_type, orig_border);

        // 4. :container set <id> background <color> previews and Esc restores
        assert!(!app.config.container_styles.contains_key("system"));
        app.command_line.buffer = "container set system background #112233".to_string();
        app.maybe_preview_command_from_cmdline(vp);
        assert!(app.config.container_styles.contains_key("system"));
        app.restore_command_preview(vp);
        assert!(!app.config.container_styles.contains_key("system"));

        // 5. :colorscheme <name> previews and Esc restores
        let orig_theme_name = app.config.theme.name.clone();
        app.command_line.buffer = "colorscheme nord".to_string();
        app.maybe_preview_command_from_cmdline(vp);
        assert_eq!(app.config.theme.name, "nord");
        app.restore_command_preview(vp);
        assert_eq!(app.config.theme.name, orig_theme_name);
    }

    #[test]
    fn background_and_container_transparency_are_independent_and_preserve_overrides() {
        let mut app = test_app();

        // 1. Background transparency is independent of visual effects
        assert!(!app.config.background.transparent);
        let orig_effect = app.config.background.effect;
        app.execute_set_option("background.transparent", "true");
        assert!(app.config.background.transparent);
        assert_eq!(app.config.background.effect, orig_effect);

        app.execute_set_option("background", "solid");
        assert!(!app.config.background.transparent);
        assert_eq!(app.config.background.effect, orig_effect);

        // 2. Individual container override + global container update preserves individual override
        app.execute_container_command(ContainerCommand::Set {
            id: "system".to_string(),
            property: ContainerProperty::Background(BackgroundFill::Solid("#112233".to_string())),
        });
        app.execute_set_option("containers.transparent", "true");
        assert_eq!(app.theme.panel_background, ratatui::style::Color::Reset);
        assert_eq!(
            app.config.container_styles.get("system").unwrap().background,
            BackgroundFill::Solid("#112233".to_string())
        );

        app.execute_set_option("containers.background", "#223344");
        assert_eq!(
            app.theme.panel_background,
            ratatui::style::Color::Rgb(0x22, 0x33, 0x44)
        );
        assert_eq!(
            app.config.container_styles.get("system").unwrap().background,
            BackgroundFill::Solid("#112233".to_string())
        );
    }

    #[test]
    fn runtime_borders_background_all_animations_and_container_assignment_work() {
        let mut app = test_app();

        // 1. Global border change updates theme AND container border configs + live preview/Esc
        let orig_border = app.config.theme.border_type;
        app.execute_border_command(BorderCommand::GlobalStyle(BorderStyle::Double));
        assert_eq!(app.config.theme.border_type, BorderStyle::Double);
        assert_eq!(app.theme.borders.top_left, "╔");
        assert_eq!(
            app.config
                .ui
                .containers
                .iter()
                .find(|c| c.name == "system")
                .unwrap()
                .border
                .style,
            Some(BorderStyle::Double)
        );

        // Per-canvas border style & background border style update only target canvas
        app.execute_border_command(BorderCommand::TargetStyle {
            id: "system".to_string(),
            style: BorderStyle::Thick,
        });
        assert_eq!(
            app.config.container_styles.get("system").unwrap().border_style,
            Some(BorderStyle::Thick)
        );
        app.execute_border_command(BorderCommand::TargetStyle {
            id: "background".to_string(),
            style: BorderStyle::Ascii,
        });
        assert!(app.config.background.border.enabled);
        assert_eq!(app.config.background.border.style, Some(BorderStyle::Ascii));

        // Reset back to original
        app.execute_border_command(BorderCommand::GlobalStyle(orig_border));
        assert_eq!(app.config.theme.border_type, orig_border);

        // `:border` preview with Tab + Esc rollback
        app.command_line.buffer = "border double".to_string();
        app.maybe_preview_command_from_cmdline(Rect::new(0, 0, 100, 36));
        assert_eq!(app.config.theme.border_type, BorderStyle::Double);
        app.restore_command_preview(Rect::new(0, 0, 100, 36));
        assert_eq!(app.config.theme.border_type, orig_border);

        // 2. Switching Background across ALL animations updates the single global BackgroundPipeline
        for anim in [
            "matrix",
            "water",
            "particles",
            "scanlines",
            "starfield",
            "noise",
            "floating_chars",
            "christmas_tree",
        ] {
            app.execute_background_command_with_size(
                BackgroundCommand::Set(anim.to_string()),
                80,
                24,
            );
            assert!(app.config.background.enabled);
            assert_eq!(app.effect.layer_count(), 1, "Failed to switch bg to '{anim}'");
            assert_eq!(app.config.background.active_effect_label(), anim);
        }

        // Stacking multiple layers on the single Background Canvas + removing one
        app.execute_background_command_with_size(
            BackgroundCommand::Add("starfield".to_string()),
            80,
            24,
        );
        assert_eq!(app.effect.layer_count(), 2);
        app.execute_background_command_with_size(
            BackgroundCommand::Remove("starfield".to_string()),
            80,
            24,
        );
        assert_eq!(app.effect.layer_count(), 1);

        // 3. Assigning animation to an isolated Container Canvas
        app.execute_container_command(ContainerCommand::Set {
            id: "system".to_string(),
            property: ContainerProperty::Animation(Some("water".to_string())),
        });
        assert_eq!(
            app.config
                .container_styles
                .get("system")
                .unwrap()
                .animation
                .as_deref(),
            Some("water")
        );
        assert!(!app.config.container_styles.contains_key("hardware"));
    }
}
