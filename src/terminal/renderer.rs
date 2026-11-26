use crate::animation::{BackgroundEffect, TransitionPlayer};
use crate::command::CommandLineState;
use crate::config::{Config, StepTarget, ThemeEntry};
use crate::ui::customizer::{CustomizerState, render_customizer, render_help_overlay};
use crate::ui::layout::compute_layout;
use crate::ui::theme::parse_color;
use crate::ui::widgets::{RenderContext, render_command_line, render_dashboard, render_window_frame};
use crate::window::WindowManager;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Paragraph};

#[derive(Debug, Clone)]
pub struct CursorStartupState {
    pub pending: bool,
    pub pos: Option<(u16, u16)>,
    pub blink_on: bool,
    pub blink_elapsed_ms: u64,
}

impl CursorStartupState {
    pub fn new(enabled: bool) -> Self {
        Self {
            pending: enabled,
            pos: None,
            blink_on: true,
            blink_elapsed_ms: 0,
        }
    }

    pub fn resolved_pos(&self, area: Rect) -> (u16, u16) {
        self.pos
            .map(|(x, y)| {
                (
                    x.clamp(area.x, area.right().saturating_sub(1)),
                    y.clamp(area.y, area.bottom().saturating_sub(1)),
                )
            })
            .unwrap_or((area.x + area.width / 2, area.y + area.height / 2))
    }

    pub fn update_blink(&mut self, dt_ms: u64, blink_enabled: bool, interval_ms: u64) {
        if !self.pending || !blink_enabled {
            self.blink_on = true;
            return;
        }
        let interval = interval_ms.max(50);
        self.blink_elapsed_ms = self.blink_elapsed_ms.saturating_add(dt_ms);
        if self.blink_elapsed_ms >= interval {
            self.blink_elapsed_ms %= interval;
            self.blink_on = !self.blink_on;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_frame(
    frame: &mut Frame,
    config: &Config,
    effect: &dyn BackgroundEffect,
    transition: &TransitionPlayer,
    windows: &mut WindowManager,
    customizer: &mut CustomizerState,
    command_line: &CommandLineState,
    startup: &CursorStartupState,
    themes: &[ThemeEntry],
    animations: &crate::animation::AnimationCatalog,
    ctx: &RenderContext,
) {
    let area = frame.area();

    let base_bg = if ctx.window_transparent && !config.background.enabled {
        Style::default().fg(ctx.theme.text).bg(Color::Reset)
    } else {
        ctx.theme.base_style()
    };
    frame.render_widget(Block::default().style(base_bg), area);

    if startup.pending {
        render_cursor_startup(frame, area, config, startup, ctx);
        return;
    }

    // Draw background animation directly onto the buffer
    effect.render(frame.buffer_mut(), area, &config.background, ctx.theme);

    let scene_target = transition.package.definition.target;
    if transition.has_scene_tracks() && scene_target == StepTarget::Background {
        transition.render_scene(frame.buffer_mut(), area, ctx.theme);
    }

    // Resolve and render windows in z-index order
    windows.resolve_all(area);
    for win in windows.windows_sorted() {
        if !win.config.visible {
            continue;
        }
        render_window_frame(frame, win, ctx);
        let inner = win.inner_area();
        if inner.width >= 10 && inner.height >= 4 {
            let layout = compute_layout(inner, config, ctx.active_page, ctx.view_mode);
            render_dashboard(frame, &layout, ctx);
        }
        transition.apply_to_buffer(frame.buffer_mut(), win.resolved_rect, ctx.theme);
        if transition.has_scene_tracks()
            && matches!(scene_target, StepTarget::Window | StepTarget::Content)
        {
            let scene_area = if scene_target == StepTarget::Content {
                inner
            } else {
                win.resolved_rect
            };
            transition.render_scene(frame.buffer_mut(), scene_area, ctx.theme);
        }
    }

    if transition.has_scene_tracks() && matches!(scene_target, StepTarget::Screen | StepTarget::Overlay)
    {
        transition.render_scene(frame.buffer_mut(), area, ctx.theme);
    }

    if customizer.open {
        render_customizer(
            frame,
            area,
            customizer,
            config,
            &windows.main_window().config,
            themes,
            animations,
            effect,
            ctx.theme,
        );
    } else if ctx.show_help {
        render_help_overlay(frame, area, ctx.theme);
    }

    render_command_line(
        frame,
        area,
        windows.main_window().resolved_rect,
        command_line,
        ctx,
    );
}

fn render_cursor_startup(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    startup: &CursorStartupState,
    ctx: &RenderContext,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if !startup.blink_on {
        return;
    }
    let (cx, cy) = startup.resolved_pos(area);
    let cursor_color = parse_color(&config.startup.cursor.color, ctx.theme.primary);
    let glyph = config
        .startup
        .cursor
        .character
        .chars()
        .next()
        .unwrap_or('▌')
        .to_string();

    let cell_rect = Rect::new(cx, cy, 1, 1);
    frame.render_widget(
        Paragraph::new(glyph).style(
            Style::default()
                .fg(cursor_color)
                .bg(ctx.theme.background)
                .add_modifier(Modifier::BOLD),
        ),
        cell_rect,
    );
}
