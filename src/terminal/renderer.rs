use crate::animation::{AnimationCatalog, BackgroundEffect, TransitionPlayer};
use crate::command::CommandLineState;
use crate::config::schema::BackgroundBaseType;
use crate::config::{BorderStyle, Config, ThemeEntry};
use crate::lua::{AnimationLayer, LuaAnimationRuntime};
use crate::ui::components::ComponentAnimationStore;
use crate::ui::customizer::{CustomizerState, render_customizer, render_help_overlay};
use crate::ui::layout::{ExclusionMask, compute_dashboard_layout};
use crate::ui::theme::parse_color;
use crate::ui::widgets::{
    RenderContext, apply_color_opacity, render_border_block_to_buf, render_command_line,
    render_containers, render_gradient_bg_with_opacity,
};
use ratatui::Frame;
use ratatui::buffer::Buffer;
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
    draw_frame_with_lua(
        frame,
        config,
        effect,
        transition,
        None,
        anim_store,
        customizer,
        command_line,
        startup,
        themes,
        animations,
        ctx,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_frame_with_lua(
    frame: &mut Frame,
    config: &Config,
    effect: &dyn BackgroundEffect,
    transition: &TransitionPlayer,
    lua_anim: Option<&LuaAnimationRuntime>,
    anim_store: &ComponentAnimationStore,
    customizer: &mut CustomizerState,
    command_line: &CommandLineState,
    startup: &CursorStartupState,
    themes: &[ThemeEntry],
    animations: &AnimationCatalog,
    ctx: &RenderContext,
) {
    let area = frame.area();
    frame.buffer_mut().reset();

    render_background_base(frame, area, config, ctx);

    let bg_area = render_background_canvas_border(frame, area, config, ctx);

    if startup.pending {
        render_cursor_startup(frame, bg_area, config, startup, ctx);
        return;
    }

    let layout = compute_dashboard_layout(area, config, ctx.view_mode);
    let mask = ExclusionMask::from_layout_with_styles(&layout, &config.container_styles);
    let intro_active = lua_anim.is_some_and(|anim| anim.is_intro() && !anim.is_finished());

    render_background_canvas_with_mask(frame, bg_area, &mask, |buf, a| {
        effect.render(buf, a, &config.background, ctx.theme);
    });

    if let Some(la) = lua_anim
        && la.layer() == AnimationLayer::Background
    {
        render_lua_animation_background(frame, bg_area, config, ctx, la);
    }

    if !intro_active && !transition.finished && transition.has_scene_tracks() {
        render_background_canvas_with_mask(frame, bg_area, &mask, |buf, a| {
            transition.render_scene(buf, a, ctx.theme);
        });
    }

    render_containers(frame, &layout, anim_store, ctx);

    if !intro_active && !transition.finished {
        transition.apply_to_buffer(frame.buffer_mut(), layout.area, ctx.theme);
    }

    if let Some(la) = lua_anim
        && la.layer() != AnimationLayer::Background
    {
        render_lua_animation_overlay(frame, layout.area, la, ctx.theme);
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

pub fn render_lua_animation_overlay(
    frame: &mut Frame,
    area: Rect,
    lua_anim: &LuaAnimationRuntime,
    theme: &crate::ui::theme::Theme,
) {
    if lua_anim.is_finished() || lua_anim.is_intro() {
        return;
    }
    match lua_anim.layer() {
        AnimationLayer::Background => {}
        AnimationLayer::Boxed => lua_anim.render_boxed(frame, area, theme),
        AnimationLayer::Overlay => lua_anim.render(frame.buffer_mut(), area),
    }
}

pub fn render_lua_animation_background(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    ctx: &RenderContext,
    lua_anim: &LuaAnimationRuntime,
) {
    if lua_anim.is_finished()
        || lua_anim.is_intro()
        || lua_anim.layer() != AnimationLayer::Background
    {
        return;
    }
    let layout = compute_dashboard_layout(area, config, ctx.view_mode);
    let mask = ExclusionMask::from_layout_with_styles(&layout, &config.container_styles);
    render_background_canvas_with_mask(frame, area, &mask, |buf, a| {
        lua_anim.render(buf, a);
    });
}

fn render_background_canvas_border(
    frame: &mut Frame,
    area: Rect,
    config: &Config,
    ctx: &RenderContext,
) -> Rect {
    let border = &config.background.border;
    let eff_style = border.style.unwrap_or(config.theme.border_type);
    if !border.enabled || eff_style == BorderStyle::None || area.width < 2 || area.height < 2 {
        return area;
    }
    let mut b_cfg = border.clone();
    b_cfg.style = Some(eff_style);
    render_border_block_to_buf(frame.buffer_mut(), area, &b_cfg, Color::Reset, false, ctx);
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

fn render_background_base(frame: &mut Frame, area: Rect, config: &Config, ctx: &RenderContext) {
    if config.background.transparent
        || config.background.base.kind == BackgroundBaseType::Transparent
        || (config.background.base.kind == BackgroundBaseType::Theme
            && ctx.theme.background == Color::Reset)
    {
        let base_bg = Style::default().fg(ctx.theme.text).bg(Color::Reset);
        frame.render_widget(Block::default().style(base_bg), area);
    } else if config.background.base.kind == BackgroundBaseType::Gradient
        && let Some(grad) = &config.background.base.gradient
    {
        frame.render_widget(
            Block::default().style(Style::default().fg(ctx.theme.text)),
            area,
        );
        render_gradient_bg_with_opacity(
            frame.buffer_mut(),
            area,
            grad,
            ctx.theme.background,
            config.background.base.opacity,
        );
    } else {
        let solid = if config.background.base.kind == BackgroundBaseType::Solid {
            config
                .background
                .base
                .color
                .as_deref()
                .map(|c| parse_color(c, ctx.theme.background))
                .unwrap_or(ctx.theme.background)
        } else {
            ctx.theme.background
        };
        let base_bg_color = apply_color_opacity(solid, config.background.base.opacity);
        let base_bg = Style::default().fg(ctx.theme.text).bg(base_bg_color);
        frame.render_widget(Block::default().style(base_bg), area);
    }
}

/// Renders the independent background layer onto its own isolated buffer and composites
/// only unmasked cells onto `frame` so background rendering never interferes with containers.
fn render_background_canvas_with_mask<F>(
    frame: &mut Frame,
    area: Rect,
    mask: &ExclusionMask,
    draw_fn: F,
) where
    F: FnOnce(&mut Buffer, Rect),
{
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut bg_surface = Buffer::empty(area);
    draw_fn(&mut bg_surface, area);

    let dest = frame.buffer_mut();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if mask.contains(x, y) {
                continue;
            }
            let src = &bg_surface[(x, y)];
            if src.symbol() == " " && src.bg == Color::Reset {
                continue;
            }
            if let Some(dst) = dest.cell_mut((x, y)) {
                *dst = src.clone();
            }
        }
    }
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
                Style::default().fg(ctx.theme.text).bg(Color::Reset),
            ),
            ratatui::text::Span::styled(
                cursor_str,
                Style::default()
                    .fg(cursor_color)
                    .bg(Color::Reset)
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
                .bg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        ),
        cell_rect,
    );
}
