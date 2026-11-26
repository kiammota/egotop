use crate::animation::{BackgroundEffect, create_effect};
use crate::config::{Config, EffectKind, ViewMode, load_from_path};
use crate::system::{SystemCollector, SystemSnapshot};
use crate::terminal::TerminalGuard;
use crate::terminal::input::{AppAction, poll_action};
use crate::terminal::renderer::draw_frame;
use crate::ui::RevealState;
use crate::ui::theme::Theme;
use crate::ui::widgets::RenderContext;
use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct App {
    config: Config,
    config_path: PathBuf,
    theme: Theme,
    collector: SystemCollector,
    snapshot: SystemSnapshot,
    effect: Box<dyn BackgroundEffect>,
    reveal: RevealState,
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
        let theme = Theme::from_config(&config.theme);
        let mut collector = SystemCollector::new();
        let snapshot = collector.collect(width, height);
        let effect = create_effect(config.background.effect, width, height, &config.background);
        let active_page = config.general.default_page.max(1);
        let view_mode = config.general.view_mode;
        let fps = config.general.animation_fps as f32;

        Self {
            config,
            config_path,
            theme,
            collector,
            snapshot,
            effect,
            reveal: RevealState::new(1.1),
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

            if now.duration_since(last_sys_refresh) >= sys_interval {
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
            last_frame = now;
            self.fps = (self.fps * 0.85) + ((1.0 / dt) * 0.15);

            if !self.paused {
                self.effect.update(dt, &self.config.background);
                self.reveal.advance(dt);
            }

            let toast_text = self.status_toast.as_ref().map(|(s, _)| s.as_str());
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
            };

            guard.terminal.draw(|frame| {
                draw_frame(frame, &self.config, self.effect.as_ref(), &ctx);
            })?;

            // Non-blocking input wait up to remaining frame budget
            let elapsed = Instant::now().duration_since(now);
            let wait = frame_budget.saturating_sub(elapsed).min(Duration::from_millis(50));
            if let Some(action) = poll_action(wait)? {
                self.handle_action(action, &mut guard, &mut last_sys_refresh)?;
            }
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        action: AppAction,
        guard: &mut TerminalGuard,
        last_sys_refresh: &mut Instant,
    ) -> Result<()> {
        match action {
            AppAction::Quit => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.should_quit = true;
                }
            }
            AppAction::Refresh => {
                let sz = guard.terminal.size()?;
                self.snapshot = self.collector.collect(sz.width, sz.height);
                *last_sys_refresh = Instant::now();
                self.set_toast("Dados do sistema atualizados");
            }
            AppAction::TogglePause => {
                self.paused = !self.paused;
                self.set_toast(if self.paused {
                    "Animações pausadas"
                } else {
                    "Animações retomadas"
                });
            }
            AppAction::ToggleHelp => {
                self.show_help = !self.show_help;
            }
            AppAction::ReloadConfig => {
                match load_from_path(&self.config_path) {
                    Ok(new_cfg) => {
                        let sz = guard.terminal.size()?;
                        self.theme = Theme::from_config(&new_cfg.theme);
                        self.effect = create_effect(
                            new_cfg.background.effect,
                            sz.width,
                            sz.height,
                            &new_cfg.background,
                        );
                        self.config = new_cfg;
                        self.set_toast("Configuração recarregada");
                    }
                    Err(err) => {
                        self.set_toast(format!("Erro no TOML: {err}"));
                    }
                }
            }
            AppAction::CycleViewMode => {
                self.view_mode = self.view_mode.next();
                self.set_toast(format!("Modo: {}", self.view_mode.label()));
            }
            AppAction::CycleEffect => {
                let next = self.config.background.effect.next();
                self.set_effect(next, guard)?;
                self.set_toast(format!("Efeito: {}", next.label()));
            }
            AppAction::SetPage(page) => {
                if (1..=3).contains(&page) {
                    self.active_page = page;
                    self.set_toast(format!("Página {page}"));
                }
            }
            AppAction::Resize(w, h) => {
                self.effect.resize(w, h, &self.config.background);
                self.snapshot = self.collector.collect(w, h);
            }
        }
        Ok(())
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
