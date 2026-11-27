use crate::animation::{AnimationCatalog, BackgroundEffect, TransitionPlayer};
use crate::command::CommandLineState;
use crate::config::{Config, StepTarget, ThemeEntry};
use crate::ui::components::ComponentAnimationStore;
use crate::ui::customizer::{CustomizerState, render_customizer, render_help_overlay};
use crate::ui::layout::compute_dashboard_layout;
use crate::ui::theme::parse_color;
use crate::ui::widgets::{RenderContext, render_command_line, render_container_tree};
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
    anim_store: &ComponentAnimationStore,
    customizer: &mut CustomizerState,
    command_line: &CommandLineState,
    startup: &CursorStartupState,
    themes: &[ThemeEntry],
    animations: &AnimationCatalog,
    ctx: &RenderContext,
) {
    let area = frame.area();

    let base_bg = if ctx.transparent_bg && !config.background.enabled {
        Style::default().fg(ctx.theme.text).bg(Color::Reset)
    } else {
        ctx.theme.base_style()
    };
    frame.render_widget(Block::default().style(base_bg), area);

    if startup.pending {
        render_cursor_startup(frame, area, config, startup, ctx);
        return;
    }

    effect.render(frame.buffer_mut(), area, &config.background, ctx.theme);

    let scene_target = transition.package.definition.target;
    if transition.has_scene_tracks() && scene_target == StepTarget::Background {
        transition.render_scene(frame.buffer_mut(), area, ctx.theme);
    }

    let layout = compute_dashboard_layout(area, config, ctx.view_mode);
    render_container_tree(frame, &layout, anim_store, ctx);

    transition.apply_to_buffer(frame.buffer_mut(), area, ctx.theme);
    if transition.has_scene_tracks() && !matches!(scene_target, StepTarget::Background) {
        transition.render_scene(frame.buffer_mut(), area, ctx.theme);
    }

    if customizer.open {
        render_customizer(
            frame,
            area,
            customizer,
            config,
            themes,
            animations,
            effect,
            ctx.theme,
        );
    } else if ctx.show_help {
        render_help_overlay(frame, area, ctx.theme);
    }

    render_command_line(frame, area, command_line, ctx);
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
    let cursor_color = parse_color(&config.startup.cursor.color, ctx.theme.primary);
    let glyph = config
        .startup
        .cursor
        .character
        .chars()
        .next()
        .unwrap_or('▌')
        .to_string();

    if config.init.wait_for_input {
        let cy = area.y + area.height / 2;
        let msg = config.init.message.trim();
        let cursor_str = if config.init.cursor && startup.blink_on {
            glyph
        } else {
            " ".to_string()
        };
        let line = ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!("{msg} "),
                Style::default().fg(ctx.theme.text).bg(ctx.theme.background),
            ),
            ratatui::text::Span::styled(
                cursor_str,
                Style::default()
                    .fg(cursor_color)
                    .bg(ctx.theme.background)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let row_rect = Rect::new(area.x, cy, area.width, 1);
        frame.render_widget(
            Paragraph::new(line).alignment(ratatui::layout::Alignment::Center),
            row_rect,
        );
        return;
    }

    if !startup.blink_on {
        return;
    }
    let (cx, cy) = startup.resolved_pos(area);
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
