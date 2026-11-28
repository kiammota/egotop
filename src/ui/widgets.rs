use crate::command::CommandLineState;
use crate::config::{Align, BorderStyle, Config, GraphStyle, ViewMode};
use crate::system::{SystemSnapshot, format_bytes, uptime::format_uptime};
use crate::ui::borders::BorderChars;
use crate::ui::components::{
    ComponentAnimState, ComponentAnimationStore, blit_isolated_canvas, render_animated_to_canvas,
};
use crate::ui::container::{
    BackgroundFill, ComponentCategory, ComponentKind, ContainerBorderConfig, ContainerConfig,
    ContainerGradient, GradientDirection,
};
use crate::ui::layout::{ComputedLayout, PlacedContainer};
use crate::ui::theme::{Theme, blend_color, parse_color};
use crate::ui::typography::{
    IconSet, RevealState, render_figlet_lines, resolve_logo_lines, slice_marquee_lines,
};
use chrono::Local;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Sparkline, Widget, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct RenderContext<'a> {
    pub config: &'a Config,
    pub theme: &'a Theme,
    pub snapshot: &'a SystemSnapshot,
    pub reveal: &'a RevealState,
    pub view_mode: ViewMode,
    pub paused: bool,
    pub show_help: bool,
    pub fps: f32,
    pub status_toast: Option<&'a str>,
    pub transparent_bg: bool,
    pub config_error_count: usize,
}

impl<'a> RenderContext<'a> {
    pub fn panel_bg(&self) -> Color {
        self.theme.panel_background
    }

    pub fn panel_style(&self) -> Style {
        Style::default()
            .fg(self.theme.text)
            .bg(self.theme.panel_background)
    }

    pub fn border_style(&self, override_color: Option<&str>) -> Style {
        let color = override_color
            .map(|c| parse_color(c, self.theme.border))
            .unwrap_or(self.theme.border);
        Style::default()
            .fg(color)
            .bg(self.theme.panel_background)
    }
}

pub fn render_containers(
    frame: &mut Frame,
    layout: &ComputedLayout,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    let dest = frame.buffer_mut();
    for container in &layout.containers {
        render_placed_container(dest, container, anim_store, ctx);
    }
}

fn render_placed_container(
    dest: &mut Buffer,
    placed: &PlacedContainer,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    let outer = placed.outer_rect.intersection(*dest.area());
    if outer.width == 0 || outer.height == 0 {
        return;
    }

    let mut cont = placed.config.clone();
    if cont.name == "status_spinner" {
        if !ctx.config.footer.enabled {
            return;
        }
        cont.content.text = Some(ctx.config.footer.text.clone());
        cont.align = ctx.config.footer.align;
        if let Some(col) = &ctx.config.footer.color {
            cont.color = Some(col.clone());
        }
        let anim = cont.animation.get_or_insert_with(Default::default);
        anim.style = ctx
            .config
            .footer
            .icon
            .clone()
            .or_else(|| ctx.config.footer.animation.clone());
        anim.fps = Some(ctx.config.footer.fps);
    }

    let style_override = ctx.config.container_styles.get(&placed.name);
    if let Some(ov) = style_override {
        if let Some(fg) = &ov.foreground {
            cont.color = Some(fg.clone());
        }
        if let Some(c) = &ov.border_color {
            cont.border.color = Some(c.clone());
        }
        if let Some(st) = ov.border_style {
            cont.border.style = Some(st);
            cont.border.enabled = st != BorderStyle::None;
        }
    }

    let eff_grad = style_override
        .and_then(|s| s.gradient.as_ref())
        .or(cont.gradient.as_ref());
    let has_grad = eff_grad.is_some();
    let opacity = style_override
        .and_then(|s| s.opacity)
        .or(cont.opacity)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let solid_panel_bg = match ctx.panel_bg() {
        Color::Reset if ctx.theme.background == Color::Reset => Color::Black,
        Color::Reset => ctx.theme.background,
        color => color,
    };
    let bg = match style_override.map(|s| &s.background) {
        Some(BackgroundFill::Transparent) => Color::Reset,
        Some(BackgroundFill::Solid(c)) => {
            apply_color_opacity(parse_color(c, ctx.theme.panel_background), opacity)
        }
        Some(BackgroundFill::Inherit) | None => {
            if cont.transparent {
                Color::Reset
            } else {
                let base = cont
                    .background
                    .as_deref()
                    .map(|c| parse_color(c, solid_panel_bg))
                    .unwrap_or(solid_panel_bg);
                apply_color_opacity(base, opacity)
            }
        }
    };

    let mut border = cont.border.clone();
    if border.title.is_none() {
        border.title = cont.content.title.clone();
    }
    let resolved_style = border.style.unwrap_or(ctx.config.theme.border_type);
    let show_border = border.enabled && resolved_style != BorderStyle::None;

    // Paint the container's outer surface (background, gradient, border/title) inside an isolated buffer.
    let local_outer = Rect::new(0, 0, outer.width, outer.height);
    let mut outer_buf = Buffer::empty(local_outer);

    if let Some(grad) = eff_grad {
        render_gradient_bg_with_opacity(
            &mut outer_buf,
            local_outer,
            grad,
            ctx.theme.panel_background,
            opacity,
        );
    }

    if show_border && outer.width >= 2 && outer.height >= 2 {
        render_border_block_to_buf(&mut outer_buf, local_outer, &border, bg, !has_grad, ctx);
    } else {
        if !has_grad && bg != Color::Reset {
            Block::default()
                .style(Style::default().fg(ctx.theme.text).bg(bg))
                .render(local_outer, &mut outer_buf);
        }
        render_borderless_title_to_buf(&mut outer_buf, local_outer, border.title.as_deref(), ctx);
    }

    blit_buffer_strict(dest, outer, &outer_buf);

    let inner = placed.inner_rect.intersection(outer);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Render all container content strictly into its own isolated [0..inner.w, 0..inner.h) canvas.
    blit_isolated_canvas(dest, inner, |canvas_buf, local_inner| {
        if bg != Color::Reset {
            Block::default().style(Style::default().bg(bg)).render(local_inner, canvas_buf);
        }
        if let Some(overlay) = anim_store.canvas_overlay(&cont.name) {
            overlay.instance.render(canvas_buf, local_inner, ctx.theme);
        }
        if cont.inline_lua.is_some() || cont.kind == ComponentKind::Canvas {
            if anim_store.canvas_overlay(&cont.name).is_none() {
                render_animated_to_canvas(
                    canvas_buf,
                    local_inner,
                    &cont,
                    anim_store,
                    ctx.config,
                    ctx.theme,
                );
            }
        } else {
            match cont.category {
                ComponentCategory::Static => {
                    render_static_to_canvas(canvas_buf, local_inner, &cont, anim_store, ctx)
                }
                ComponentCategory::Dynamic => {
                    render_dynamic_to_canvas(canvas_buf, local_inner, &cont, ctx)
                }
                ComponentCategory::Animated => render_animated_to_canvas(
                    canvas_buf,
                    local_inner,
                    &cont,
                    anim_store,
                    ctx.config,
                    ctx.theme,
                ),
            }
        }
    });
}

fn blit_buffer_strict(dest: &mut Buffer, target: Rect, src: &Buffer) {
    let clipped = target.intersection(*dest.area());
    let max_w = clipped.width.min(src.area().width);
    let max_h = clipped.height.min(src.area().height);
    for dy in 0..max_h {
        for dx in 0..max_w {
            let s = &src[(dx, dy)];
            if s.symbol() == " " && s.bg == Color::Reset {
                continue;
            }
            if let Some(d) = dest.cell_mut((clipped.x + dx, clipped.y + dy)) {
                let preserved_bg = if s.bg == Color::Reset { d.bg } else { s.bg };
                *d = s.clone();
                d.set_bg(preserved_bg);
            }
        }
    }
}

pub fn apply_color_opacity(c: Color, opacity: f32) -> Color {
    let op = opacity.clamp(0.0, 1.0);
    if op >= 0.999 {
        return c;
    }
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * op).round() as u8,
            ((g as f32) * op).round() as u8,
            ((b as f32) * op).round() as u8,
        ),
        other => other,
    }
}

pub fn render_gradient_bg_with_opacity(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    grad: &ContainerGradient,
    fallback: Color,
    opacity: f32,
) {
    if grad.colors.len() < 2 || area.width == 0 || area.height == 0 {
        return;
    }
    let parsed: Vec<Color> = grad
        .colors
        .iter()
        .map(|c| parse_color(c, fallback))
        .collect();
    let segments = parsed.len() - 1;

    for dy in 0..area.height {
        for dx in 0..area.width {
            let t = match grad.direction {
                GradientDirection::Horizontal => {
                    if area.width <= 1 {
                        0.0
                    } else {
                        dx as f32 / (area.width - 1) as f32
                    }
                }
                GradientDirection::Vertical => {
                    if area.height <= 1 {
                        0.0
                    } else {
                        dy as f32 / (area.height - 1) as f32
                    }
                }
            };
            let scaled = (t.clamp(0.0, 1.0)) * segments as f32;
            let idx = (scaled.floor() as usize).min(segments - 1);
            let local_t = scaled - idx as f32;
            let color = apply_color_opacity(blend_color(parsed[idx], parsed[idx + 1], local_t), opacity);
            if let Some(cell) = buf.cell_mut((area.x + dx, area.y + dy)) {
                cell.set_bg(color);
            }
        }
    }
}

pub(crate) fn render_border_block_to_buf(
    buf: &mut Buffer,
    area: Rect,
    border: &ContainerBorderConfig,
    bg: Color,
    clear_if_opaque: bool,
    ctx: &RenderContext,
) {
    let border_color = border
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.border))
        .unwrap_or(ctx.theme.border);
    let border_chars_owned;
    let border_set = if let Some(explicit_style) = border.style {
        border_chars_owned = BorderChars::from_style(explicit_style);
        border_chars_owned.as_ratatui_set()
    } else {
        ctx.theme.borders.as_ratatui_set()
    };
    let mut b_style = Style::default().fg(border_color);
    let mut blk_style = Style::default().fg(ctx.theme.text);
    if clear_if_opaque {
        b_style = b_style.bg(bg);
        blk_style = blk_style.bg(bg);
    }
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_set(border_set)
        .border_style(b_style)
        .style(blk_style);

    if let Some(title) = &border.title
        && !title.is_empty()
    {
        let title_lower = title.to_lowercase();
        block = block.title(Span::styled(
            format!(" {title_lower} "),
            Style::default()
                .fg(ctx.theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if clear_if_opaque && bg != Color::Reset {
        Clear.render(area, buf);
    }
    block.render(area, buf);
}

fn render_borderless_title_to_buf(
    buf: &mut Buffer,
    area: Rect,
    title: Option<&str>,
    ctx: &RenderContext,
) {
    let Some(title) = title.filter(|t| !t.is_empty()) else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }

    let (title_x, max_w) = if area.width >= 3 {
        (area.x.saturating_add(1), area.width.saturating_sub(2))
    } else {
        (area.x, area.width)
    };
    let title_lower = title.to_lowercase();
    let title_area = Rect::new(title_x, area.y, max_w, 1);
    let span = Span::styled(
        format!(" {title_lower} "),
        Style::default()
            .fg(ctx.theme.primary)
            .add_modifier(Modifier::BOLD),
    );
    Paragraph::new(Line::from(span)).render(title_area, buf);
}

fn render_static_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    match cont.kind {
        ComponentKind::Greeting => render_greeting_to_canvas(buf, area, cont, anim_store, ctx),
        ComponentKind::AsciiLogo => render_logo_to_canvas(buf, area, cont, ctx),
        ComponentKind::Separator => render_separator_to_canvas(buf, area, cont, ctx),
        _ => render_text_to_canvas(buf, area, cont, ctx),
    }
}

fn render_dynamic_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    match cont.kind {
        ComponentKind::Header => render_header_to_canvas(buf, area, ctx),
        ComponentKind::System => render_system_to_canvas(buf, area, cont, ctx),
        ComponentKind::Hardware => render_hardware_to_canvas(buf, area, ctx),
        ComponentKind::Environment => render_environment_to_canvas(buf, area, ctx),
        ComponentKind::Terminal => render_terminal_to_canvas(buf, area, ctx),
        ComponentKind::Storage => render_storage_to_canvas(buf, area, ctx),
        ComponentKind::Activity => render_activity_to_canvas(buf, area, cont, ctx),
        ComponentKind::Session => render_session_to_canvas(buf, area, ctx),
        ComponentKind::Cpu => render_cpu_to_canvas(buf, area, ctx),
        ComponentKind::Memory => render_memory_to_canvas(buf, area, ctx),
        ComponentKind::Temperature => render_temperature_to_canvas(buf, area, ctx),
        ComponentKind::Processes => render_processes_to_canvas(buf, area, ctx),
        ComponentKind::Network => render_network_to_canvas(buf, area, ctx),
        ComponentKind::Uptime => render_uptime_to_canvas(buf, area, ctx),
        ComponentKind::Graph => render_graph_to_canvas(buf, area, cont, ctx),
        _ => render_text_to_canvas(buf, area, cont, ctx),
    }
}

fn to_alignment(align: Align) -> Alignment {
    match align {
        Align::Left => Alignment::Left,
        Align::Center => Alignment::Center,
        Align::Right => Alignment::Right,
    }
}

fn render_text_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let text = cont
        .content
        .text
        .as_deref()
        .unwrap_or(&ctx.config.general.message);
    let fg = cont
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.text))
        .unwrap_or(ctx.theme.text);
    let bg = cont
        .background
        .as_deref()
        .map(|c| parse_color(c, ctx.panel_bg()))
        .unwrap_or_else(|| ctx.panel_bg());

    let mut span_style = Style::default().fg(fg);
    if cont.background.is_some() && bg != Color::Reset {
        span_style = span_style.bg(bg);
    }
    let lines: Vec<Line> = text
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), span_style)))
        .collect();
    Paragraph::new(lines)
        .alignment(to_alignment(cont.align))
        .wrap(Wrap { trim: true })
        .render(area, buf);
}

fn render_separator_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let ch = cont
        .content
        .separator_char
        .as_deref()
        .and_then(|s| s.chars().next())
        .unwrap_or('─');
    let fg = cont
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.border))
        .unwrap_or(ctx.theme.border);
    let bg = ctx.panel_bg();

    let style = Style::default().fg(fg).bg(bg);
    if area.height == 1 {
        let line: String = std::iter::repeat_n(ch, area.width as usize).collect();
        buf.set_stringn(area.x, area.y, &line, area.width as usize, style);
    } else {
        let x = area.x + area.width / 2;
        for y in area.y..area.bottom() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    }
}

fn render_logo_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let logo_lines = if let Some(custom) = &cont.content.text {
        custom.lines().map(ToString::to_string).collect()
    } else {
        resolve_logo_lines(&ctx.config.typography, ctx.snapshot)
    };
    let fg = cont
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.secondary))
        .unwrap_or(ctx.theme.secondary);
    let lines: Vec<Line> = logo_lines
        .into_iter()
        .map(|l| {
            Line::from(Span::styled(
                l,
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    Paragraph::new(center_vertically(lines, area.height))
        .alignment(to_alignment(cont.align))
        .render(area, buf);
}

fn render_greeting_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let user = ctx
        .config
        .general
        .user_override
        .clone()
        .or_else(|| ctx.snapshot.os.username.clone())
        .unwrap_or_else(|| "user".to_string());
    let host = ctx.snapshot.os.hostname.as_deref().unwrap_or("linux");
    let distro = ctx.snapshot.os.distro_name.as_deref().unwrap_or("Linux");
    let wm = ctx
        .snapshot
        .session
        .window_manager
        .as_deref()
        .unwrap_or("Terminal");

    let raw_template = cont
        .content
        .text
        .as_deref()
        .unwrap_or(&ctx.config.general.greeting);
    let greeting_text = raw_template
        .replace("{user}", &user.to_uppercase())
        .replace("{host}", host)
        .replace("{distro}", distro)
        .replace("{wm}", wm);

    let text_area = area;

    let font_name = cont
        .font
        .as_deref()
        .unwrap_or(&ctx.config.typography.font);

    let raw_figlet = if text_area.height >= 5 {
        render_figlet_lines(
            &greeting_text,
            font_name,
            ctx.config.typography.ascii_fallback,
        )
    } else {
        vec![greeting_text.clone()]
    };

    let widest = raw_figlet.iter().map(|l| l.width()).max().unwrap_or(0);
    let is_marquee = widest > text_area.width as usize && text_area.width > 0;
    let figlet_lines = if is_marquee {
        let offset = match anim_store.get(&cont.name) {
            Some(ComponentAnimState::Marquee(st)) => st.offset_cols.floor() as usize,
            _ => 0,
        };
        slice_marquee_lines(&raw_figlet, text_area.width as usize, offset, 6)
    } else {
        raw_figlet
    };

    let accent_color = cont
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.primary))
        .unwrap_or(ctx.theme.primary);

    let mut lines: Vec<Line<'static>> = figlet_lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let shown = if is_marquee { l } else { ctx.reveal.mask_line(&l, i) };
            Line::from(Span::styled(
                shown,
                Style::default()
                    .fg(accent_color)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    if !ctx.config.general.message.trim().is_empty() && lines.len() < text_area.height as usize {
        if lines.len() + 2 <= text_area.height as usize {
            lines.push(Line::from(""));
        }
        let msg = ctx.reveal.mask_line(&ctx.config.general.message, lines.len());
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(ctx.theme.accent),
        )));
    }

    let align = if is_marquee {
        Alignment::Left
    } else {
        to_alignment(cont.align)
    };
    Paragraph::new(center_vertically(lines, text_area.height))
        .alignment(align)
        .render(text_area, buf);
}

fn render_header_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let now = Local::now();
    let icons = IconSet::from_config(&ctx.config.typography);

    let brand_label = format!(" {} ", ctx.config.general.name.to_lowercase());
    let brand_w = brand_label.width() as u16;
    let brand_span = Span::styled(
        brand_label,
        Style::default()
            .fg(ctx.theme.background)
            .bg(ctx.theme.primary)
            .add_modifier(Modifier::BOLD),
    );

    let mut left_spans = Vec::new();
    if ctx.config.header.show_indicators {
        let anim_label = if ctx.paused {
            "paused".to_string()
        } else {
            ctx.config.background.active_effect_label()
        };
        let anim_color = if ctx.paused {
            ctx.theme.accent
        } else {
            ctx.theme.secondary
        };
        left_spans.push(Span::styled(
            format!("[bg: {anim_label}] "),
            Style::default().fg(anim_color),
        ));
        left_spans.push(Span::styled(
            format!("[{}]", ctx.view_mode.label().to_lowercase()),
            Style::default().fg(ctx.theme.muted),
        ));
        if ctx.config.header.show_fps {
            left_spans.push(Span::styled(
                format!(" {:.0}fps", ctx.fps),
                Style::default().fg(ctx.theme.muted),
            ));
        }
        if ctx.config_error_count > 0 {
            left_spans.push(Span::styled(
                format!(" [{} err • :errors]", ctx.config_error_count),
                Style::default()
                    .fg(ctx.theme.danger)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    let mut right_parts = Vec::new();
    if ctx.config.header.show_date {
        right_parts.push(now.format(&ctx.config.general.date_format).to_string());
    }
    if ctx.config.header.show_clock {
        right_parts.push(format!(
            "{} {}",
            icons.clock(),
            now.format(&ctx.config.general.time_format)
        ));
    }
    let right_text = right_parts.join("  ");

    let eff_brand_w = brand_w.min(area.width);
    let ideal_center_x = area.x + area.width.saturating_sub(eff_brand_w) / 2;
    let max_brand_x = area.right().saturating_sub(eff_brand_w);
    let brand_x = ideal_center_x.clamp(area.x, max_brand_x.max(area.x));

    let left_w = brand_x.saturating_sub(area.x);
    let left_rect = Rect::new(area.x, area.y, left_w, area.height);
    let center_rect = Rect::new(brand_x, area.y, eff_brand_w, area.height);
    let right_x = brand_x.saturating_add(eff_brand_w);
    let right_w = area.right().saturating_sub(right_x);
    let right_rect = Rect::new(right_x, area.y, right_w, area.height);

    if left_rect.width > 0 {
        Paragraph::new(Line::from(left_spans))
            .style(Style::default())
            .render(left_rect, buf);
    }
    if center_rect.width > 0 {
        Paragraph::new(Line::from(brand_span))
            .alignment(Alignment::Center)
            .style(Style::default())
            .render(center_rect, buf);
    }
    if right_rect.width > 0 {
        Paragraph::new(Line::from(Span::styled(
            right_text,
            Style::default()
                .fg(ctx.theme.accent)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right)
        .style(Style::default())
        .render(right_rect, buf);
    }
}

fn render_system_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let os = &ctx.snapshot.os;
    let sess = &ctx.snapshot.session;
    let hide = ctx.config.general.hide_unavailable;

    let pkgs_str = if os.packages.is_empty() {
        None
    } else {
        Some(
            os.packages
                .iter()
                .map(|(mgr, cnt)| format!("{cnt} ({mgr})"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    let os_arch = match (&os.distro_name, &os.arch) {
        (Some(d), Some(a)) => Some(format!("{d} {a}")),
        (Some(d), None) => Some(d.clone()),
        _ => None,
    };

    let candidates = [
        ("distro", format!("{} OS", icons.os()), os_arch),
        ("host_model", format!("{} Host", icons.host()), os.host_model.clone()),
        ("hostname", "  Hostname".to_string(), os.hostname.clone()),
        ("kernel", format!("{} Kernel", icons.kernel()), os.kernel.clone()),
        ("packages", "  Packages".to_string(), pkgs_str),
        ("locale", "  Locale".to_string(), os.locale.clone()),
        ("shell", format!("{} Shell", icons.shell()), os.shell.clone()),
        ("terminal", format!("{} Terminal", icons.terminal()), os.terminal.clone()),
        (
            "wm",
            format!("{} DE/WM", icons.wm()),
            sess.window_manager
                .clone()
                .or_else(|| sess.desktop_env.clone()),
        ),
        (
            "resolution",
            format!("{} Term Size", icons.terminal()),
            Some(format!("{}x{}", sess.terminal_cols, sess.terminal_rows)),
        ),
    ];

    let mut rows: Vec<(String, String)> = Vec::new();
    for (key, label, value) in candidates {
        if let Some(allowed) = &cont.content.fields
            && !allowed.iter().any(|f| f.eq_ignore_ascii_case(key))
        {
            continue;
        }
        match value {
            Some(val) => rows.push((label, val)),
            None if !hide => rows.push((label, "N/A".to_string())),
            None => {}
        }
    }

    let label_w = compute_label_width(rows.iter().map(|(l, _)| l.as_str()));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();

    Paragraph::new(lines).render(area, buf);
}

enum HardwareRow {
    Kv(String, String),
    Bar(String, f32, String),
}

fn render_hardware_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let hw = &ctx.snapshot.hardware;
    let hide = ctx.config.general.hide_unavailable;

    let mut rows: Vec<HardwareRow> = Vec::new();

    if let Some(m) = hw.cpu.model.as_deref() {
        rows.push(HardwareRow::Kv(
            format!("{} CPU", icons.cpu()),
            m.to_string(),
        ));
    } else if !hide {
        rows.push(HardwareRow::Kv(
            format!("{} CPU", icons.cpu()),
            "N/A".to_string(),
        ));
    }

    let cpu_usage_row = match (hw.cpu.usage_percent, hw.cpu.temp_celsius, hw.cpu.freq_ghz) {
        (Some(pct), Some(temp), Some(ghz)) => Some(HardwareRow::Bar(
            "  CPU Usage".to_string(),
            pct,
            format!("{pct:.1}% ({}c @ {ghz:.2}GHz, {temp:.0}°C)", hw.cpu.cores),
        )),
        (Some(pct), Some(temp), None) => Some(HardwareRow::Bar(
            "  CPU Usage".to_string(),
            pct,
            format!("{pct:.1}% ({temp:.0}°C)"),
        )),
        (Some(pct), None, Some(ghz)) => Some(HardwareRow::Bar(
            "  CPU Usage".to_string(),
            pct,
            format!("{pct:.1}% ({}c @ {ghz:.2}GHz)", hw.cpu.cores),
        )),
        (Some(pct), None, None) => Some(HardwareRow::Bar(
            "  CPU Usage".to_string(),
            pct,
            format!("{pct:.1}% ({} cores)", hw.cpu.cores),
        )),
        (None, Some(temp), _) => Some(HardwareRow::Kv(
            "  CPU Temp".to_string(),
            format!("{temp:.1}°C"),
        )),
        (None, None, _) if !hide => {
            Some(HardwareRow::Kv("  CPU Usage".to_string(), "N/A".to_string()))
        }
        (None, None, _) => None,
    };
    if let Some(row) = cpu_usage_row {
        rows.push(row);
    }

    if let (Some(used), Some(total), Some(pct)) = (
        hw.memory.used_bytes,
        hw.memory.total_bytes,
        hw.memory.usage_percent(),
    ) {
        let info = format!("{} / {} ({pct:.0}%)", format_bytes(used), format_bytes(total));
        rows.push(HardwareRow::Bar(
            format!("{} RAM", icons.memory()),
            pct,
            info,
        ));
    } else if !hide {
        rows.push(HardwareRow::Kv(
            format!("{} RAM", icons.memory()),
            "N/A".to_string(),
        ));
    }

    if let (Some(s_used), Some(s_total)) = (hw.memory.swap_used_bytes, hw.memory.swap_total_bytes) {
        let pct = ((s_used as f64 / s_total.max(1) as f64) * 100.0) as f32;
        rows.push(HardwareRow::Kv(
            "  Swap".to_string(),
            format!(
                "{} / {} ({pct:.0}%)",
                format_bytes(s_used),
                format_bytes(s_total)
            ),
        ));
    }

    if let Some(disk) = hw.disks.first() {
        let pct = disk.usage_percent();
        let info = format!(
            "{} / {} ({pct:.0}%)",
            format_bytes(disk.used_bytes),
            format_bytes(disk.total_bytes)
        );
        rows.push(HardwareRow::Bar(
            format!("{} Disk ({})", icons.disk(), disk.mount_point),
            pct,
            info,
        ));
    } else if !hide {
        rows.push(HardwareRow::Kv(
            format!("{} Disk", icons.disk()),
            "N/A".to_string(),
        ));
    }

    if let Some(gpu) = hw.gpus.first() {
        let mut gpu_str = gpu.name.clone().unwrap_or_else(|| "GPU".to_string());
        if let Some(disc) = gpu.is_discrete {
            gpu_str.push_str(if disc { " [Discrete]" } else { " [Integrated]" });
        }
        if let Some(busy) = gpu.busy_percent {
            gpu_str.push_str(&format!(" • {busy:.0}%"));
        }
        if let (Some(v_used), Some(v_total)) = (gpu.vram_used_bytes, gpu.vram_total_bytes) {
            gpu_str.push_str(&format!(
                " • VRAM {}/{}",
                format_bytes(v_used),
                format_bytes(v_total)
            ));
        }
        rows.push(HardwareRow::Kv(format!("{} GPU", icons.gpu()), gpu_str));
    } else if !hide {
        rows.push(HardwareRow::Kv(
            format!("{} GPU", icons.gpu()),
            "N/A".to_string(),
        ));
    }

    let label_w = compute_label_width(rows.iter().map(|r| match r {
        HardwareRow::Kv(l, _) | HardwareRow::Bar(l, _, _) => l.as_str(),
    }));
    let bar_width = compute_bar_width(area.width, label_w);

    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|row| match row {
            HardwareRow::Kv(l, v) => kv_line_aligned(&l, &v, label_w, area.width, ctx.theme),
            HardwareRow::Bar(l, pct, suf) => {
                bar_line_aligned(&l, pct, &suf, label_w, bar_width, area.width, ctx.theme)
            }
        })
        .collect();

    Paragraph::new(lines).render(area, buf);
}

fn render_activity_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let act = &ctx.snapshot.activity;
    let icons = IconSet::from_config(&ctx.config.typography);

    let load_str = match (act.load_1m, act.load_5m, act.load_15m) {
        (Some(a), Some(b), Some(c)) => format!("{a:.2} / {b:.2} / {c:.2}"),
        _ => "N/A".to_string(),
    };
    let procs_str = act
        .process_count
        .map(|p| p.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let threads_str = match (act.runnable_threads, act.thread_count) {
        (Some(run), Some(tot)) => format!("{tot} ({run} active)"),
        (None, Some(tot)) => tot.to_string(),
        _ => "N/A".to_string(),
    };

    let info_rows = vec![
        (format!("{} Load (1/5/15)", icons.activity()), load_str),
        ("  Processes".to_string(), procs_str),
        ("  Threads".to_string(), threads_str),
    ];
    let label_w = compute_label_width(info_rows.iter().map(|(l, _)| l.as_str()));
    let info_lines: Vec<Line<'static>> = info_rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();

    let graph_style = cont.graph_style.unwrap_or(GraphStyle::Sparkline);
    if area.height >= 6 && graph_style == GraphStyle::Sparkline {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(2),
            ])
            .split(area);

        Paragraph::new(info_lines).render(chunks[0], buf);
        Paragraph::new(Line::from(Span::styled(
            "cpu % / ram % history:",
            Style::default().fg(ctx.theme.muted),
        )))
        .render(chunks[1], buf);

        let graph_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        let cpu_data: Vec<u64> = ctx.snapshot.cpu_history.iter().copied().collect();
        let mem_data: Vec<u64> = ctx.snapshot.mem_history.iter().copied().collect();

        Sparkline::default()
            .data(&cpu_data)
            .max(100)
            .style(Style::default().fg(ctx.theme.primary))
            .render(graph_cols[0], buf);
        Sparkline::default()
            .data(&mem_data)
            .max(100)
            .style(Style::default().fg(ctx.theme.secondary))
            .render(graph_cols[1], buf);
    } else {
        Paragraph::new(info_lines).render(area, buf);
    }
}

fn render_session_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let act = &ctx.snapshot.activity;
    let sess = &ctx.snapshot.session;
    let hide = ctx.config.general.hide_unavailable;

    let uptime_str = act.uptime_secs.map(format_uptime);
    let boot_str = act
        .boot_time
        .map(|bt| bt.format("%d/%m %H:%M:%S").to_string());

    let now = Local::now();
    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(up) = uptime_str {
        rows.push((format!("{} Uptime", icons.uptime()), up));
    } else if !hide {
        rows.push((format!("{} Uptime", icons.uptime()), "N/A".to_string()));
    }

    if let Some(boot) = boot_str {
        rows.push((format!("{} Boot", icons.clock()), boot));
    } else if !hide {
        rows.push((format!("{} Boot", icons.clock()), "N/A".to_string()));
    }

    rows.push((
        "  Time".to_string(),
        now.format(&ctx.config.general.time_format).to_string(),
    ));

    if let Some(disp) = &sess.display {
        let kind = sess.session_type.as_deref().unwrap_or("Display");
        rows.push((format!("{} {kind}", icons.wm()), disp.clone()));
    }

    if sess.is_hyprland {
        rows.push(("  Compositor".to_string(), "Hyprland active".to_string()));
    }

    let label_w = compute_label_width(rows.iter().map(|(l, _)| l.as_str()));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();

    Paragraph::new(lines).render(area, buf);
}

fn render_cpu_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let cpu = &ctx.snapshot.hardware.cpu;
    let mut rows: Vec<HardwareRow> = Vec::new();
    if let Some(model) = &cpu.model {
        rows.push(HardwareRow::Kv("CPU".to_string(), model.clone()));
    }
    if let Some(pct) = cpu.usage_percent {
        rows.push(HardwareRow::Bar(
            "Total".to_string(),
            pct,
            format!("{pct:.1}% ({} cores)", cpu.cores),
        ));
    }
    let max_cores = (area.height as usize).saturating_sub(rows.len());
    for (idx, pct) in cpu.per_core_percent.iter().take(max_cores).enumerate() {
        rows.push(HardwareRow::Bar(
            format!("Core {idx}"),
            *pct,
            format!("{pct:.0}%"),
        ));
    }
    let label_w = compute_label_width(rows.iter().map(|r| match r {
        HardwareRow::Kv(l, _) | HardwareRow::Bar(l, _, _) => l.as_str(),
    }));
    let bar_width = compute_bar_width(area.width, label_w);
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|r| match r {
            HardwareRow::Kv(l, v) => kv_line_aligned(&l, &v, label_w, area.width, ctx.theme),
            HardwareRow::Bar(l, pct, suf) => {
                bar_line_aligned(&l, pct, &suf, label_w, bar_width, area.width, ctx.theme)
            }
        })
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_memory_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let mem = &ctx.snapshot.hardware.memory;
    let mut rows: Vec<HardwareRow> = Vec::new();
    if let (Some(used), Some(total), Some(pct)) =
        (mem.used_bytes, mem.total_bytes, mem.usage_percent())
    {
        rows.push(HardwareRow::Bar(
            "RAM".to_string(),
            pct,
            format!("{} / {} ({pct:.0}%)", format_bytes(used), format_bytes(total)),
        ));
    }
    if let Some(avail) = mem.available_bytes {
        rows.push(HardwareRow::Kv(
            "Available".to_string(),
            format_bytes(avail),
        ));
    }
    if let (Some(su), Some(st)) = (mem.swap_used_bytes, mem.swap_total_bytes) {
        let pct = (su as f64 / st.max(1) as f64 * 100.0) as f32;
        rows.push(HardwareRow::Bar(
            "Swap".to_string(),
            pct,
            format!("{} / {}", format_bytes(su), format_bytes(st)),
        ));
    }
    let label_w = compute_label_width(rows.iter().map(|r| match r {
        HardwareRow::Kv(l, _) | HardwareRow::Bar(l, _, _) => l.as_str(),
    }));
    let bar_width = compute_bar_width(area.width, label_w);
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|r| match r {
            HardwareRow::Kv(l, v) => kv_line_aligned(&l, &v, label_w, area.width, ctx.theme),
            HardwareRow::Bar(l, pct, suf) => {
                bar_line_aligned(&l, pct, &suf, label_w, bar_width, area.width, ctx.theme)
            }
        })
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_temperature_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let temp = ctx.snapshot.hardware.cpu.temp_celsius;
    let val = match temp {
        Some(t) => format!("{t:.1} °C"),
        None => "Unavailable".to_string(),
    };
    let line = kv_line_aligned("CPU Temp", &val, 8, area.width, ctx.theme);
    Paragraph::new(vec![line]).render(area, buf);
}

fn render_processes_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let act = &ctx.snapshot.activity;
    let rows = vec![
        (
            "Processes",
            act.process_count
                .map(|p| p.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
        ),
        (
            "Threads",
            act.thread_count
                .map(|t| t.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
        ),
        (
            "Runnable",
            act.runnable_threads
                .map(|r| r.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
        ),
    ];
    let label_w = compute_label_width(rows.iter().map(|(l, _)| *l));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(l, &v, label_w, area.width, ctx.theme))
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_network_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let net = &ctx.snapshot.network;
    let mut rows = vec![
        (
            "RX Rate".to_string(),
            format!("{}/s", format_bytes(net.total_rx_bps)),
        ),
        (
            "TX Rate".to_string(),
            format!("{}/s", format_bytes(net.total_tx_bps)),
        ),
    ];
    for iface in net.interfaces.iter().take(3) {
        let ip = iface.ipv4.as_deref().unwrap_or("no-ip");
        rows.push((
            iface.name.clone(),
            format!(
                "{ip} (↓{} ↑{})",
                format_bytes(iface.rx_bytes),
                format_bytes(iface.tx_bytes)
            ),
        ));
    }
    let label_w = compute_label_width(rows.iter().map(|(l, _)| l.as_str()));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_environment_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let os = &ctx.snapshot.os;
    let sess = &ctx.snapshot.session;
    let icons = IconSet::from_config(&ctx.config.typography);
    let mut rows: Vec<(String, String)> = Vec::new();
    rows.push((
        format!("{} WM/DE", icons.wm()),
        sess.window_manager
            .as_deref()
            .or(sess.desktop_env.as_deref())
            .unwrap_or("N/A")
            .to_string(),
    ));
    rows.push((
        "  Protocol".to_string(),
        sess.session_type.as_deref().unwrap_or("N/A").to_string(),
    ));
    if let Some(disp) = os.displays.first() {
        let kind = if disp.is_internal { "Internal" } else { "External" };
        let res = disp.resolution.as_deref().unwrap_or("?");
        rows.push((
            "  Display".to_string(),
            format!("{} {res} ({kind})", disp.name),
        ));
    }
    if let Some(theme) = &os.gtk_theme {
        rows.push(("  GTK Theme".to_string(), theme.clone()));
    }
    if let Some(ic) = &os.icons {
        rows.push(("  Icons".to_string(), ic.clone()));
    }
    if let Some(font) = &os.ui_font {
        rows.push(("  UI Font".to_string(), font.clone()));
    }
    if let Some(cursor) = &os.cursor_theme {
        rows.push(("  Cursor".to_string(), cursor.clone()));
    }
    let label_w = compute_label_width(rows.iter().map(|(l, _)| l.as_str()));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_terminal_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let os = &ctx.snapshot.os;
    let sess = &ctx.snapshot.session;
    let icons = IconSet::from_config(&ctx.config.typography);
    let rows = vec![
        (
            format!("{} Terminal", icons.terminal()),
            os.terminal.as_deref().unwrap_or("N/A").to_string(),
        ),
        (
            format!("{} Shell", icons.shell()),
            os.shell.as_deref().unwrap_or("N/A").to_string(),
        ),
        (
            "  Font".to_string(),
            os.terminal_font.as_deref().unwrap_or("N/A").to_string(),
        ),
        (
            "  Size".to_string(),
            format!("{}x{}", sess.terminal_cols, sess.terminal_rows),
        ),
    ];
    let label_w = compute_label_width(rows.iter().map(|(l, _)| l.as_str()));
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(l, v)| kv_line_aligned(&l, &v, label_w, area.width, ctx.theme))
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_storage_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let disks = &ctx.snapshot.hardware.disks;
    let mut rows: Vec<HardwareRow> = Vec::new();
    for disk in disks {
        let pct = disk.usage_percent();
        let info = format!(
            "{} / {} ({pct:.0}%) [{}]",
            format_bytes(disk.used_bytes),
            format_bytes(disk.total_bytes),
            disk.fs_type
        );
        rows.push(HardwareRow::Bar(
            format!("{} {}", icons.disk(), disk.mount_point),
            pct,
            info,
        ));
    }
    if rows.is_empty() {
        rows.push(HardwareRow::Kv(
            format!("{} Disk", icons.disk()),
            "N/A".to_string(),
        ));
    }
    let label_w = compute_label_width(rows.iter().map(|r| match r {
        HardwareRow::Kv(l, _) | HardwareRow::Bar(l, _, _) => l.as_str(),
    }));
    let bar_width = compute_bar_width(area.width, label_w);
    let lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|row| match row {
            HardwareRow::Kv(l, v) => kv_line_aligned(&l, &v, label_w, area.width, ctx.theme),
            HardwareRow::Bar(l, pct, suf) => {
                bar_line_aligned(&l, pct, &suf, label_w, bar_width, area.width, ctx.theme)
            }
        })
        .collect();
    Paragraph::new(lines).render(area, buf);
}

fn render_uptime_to_canvas(buf: &mut Buffer, area: Rect, ctx: &RenderContext) {
    render_session_to_canvas(buf, area, ctx);
}

fn render_graph_to_canvas(
    buf: &mut Buffer,
    area: Rect,
    cont: &ContainerConfig,
    ctx: &RenderContext,
) {
    let metric = cont
        .content
        .metric
        .as_deref()
        .unwrap_or("cpu")
        .to_ascii_lowercase();
    let (data, max_val, color) = match metric.as_str() {
        "mem" | "memory" | "ram" => (
            ctx.snapshot.mem_history.iter().copied().collect::<Vec<_>>(),
            100,
            ctx.theme.secondary,
        ),
        "net" | "network" | "rx" => (
            ctx.snapshot
                .net_rx_history
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            ctx.snapshot
                .net_rx_history
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .max(1),
            ctx.theme.accent,
        ),
        _ => (
            ctx.snapshot.cpu_history.iter().copied().collect::<Vec<_>>(),
            100,
            ctx.theme.primary,
        ),
    };
    Sparkline::default()
        .data(&data)
        .max(max_val)
        .style(Style::default().fg(color))
        .render(area, buf);
}

fn center_vertically(lines: Vec<Line<'static>>, available_height: u16) -> Vec<Line<'static>> {
    let total = lines.len();
    let avail = available_height as usize;
    if total == 0 || total >= avail {
        return lines;
    }
    let top_pad = (avail - total) / 2;
    let mut padded = Vec::with_capacity(top_pad + total);
    for _ in 0..top_pad {
        padded.push(Line::from(""));
    }
    padded.extend(lines);
    padded
}

fn compute_label_width<'a>(labels: impl Iterator<Item = &'a str>) -> usize {
    labels
        .map(|l| l.width())
        .max()
        .unwrap_or(10)
        .clamp(7, 18)
}

fn compute_bar_width(area_width: u16, label_w: usize) -> usize {
    area_width
        .saturating_sub(label_w as u16 + 32)
        .clamp(6, 16) as usize
}

fn pad_label(label: &str, target_width: usize) -> String {
    let w = label.width();
    if w >= target_width {
        label.to_string()
    } else {
        format!("{label}{}", " ".repeat(target_width - w))
    }
}

fn kv_line_aligned<'a>(
    label: &str,
    value: &str,
    label_width: usize,
    area_width: u16,
    theme: &Theme,
) -> Line<'a> {
    let lower_label = label.to_lowercase();
    let padded = pad_label(&lower_label, label_width);
    let prefix_w = padded.width() + 2;
    let max_val_w = (area_width as usize).saturating_sub(prefix_w);
    let val_str = if area_width == u16::MAX {
        value.to_string()
    } else if max_val_w > 0 {
        truncate_display_width(value, max_val_w)
    } else {
        String::new()
    };
    Line::from(vec![
        Span::styled(
            format!("{padded}  "),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(val_str, Style::default().fg(theme.text)),
    ])
}

fn bar_line_aligned<'a>(
    label: &str,
    pct: f32,
    suffix: &str,
    label_width: usize,
    bar_width: usize,
    area_width: u16,
    theme: &Theme,
) -> Line<'a> {
    let lower_label = label.to_lowercase();
    let padded = pad_label(&lower_label, label_width);
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * bar_width as f32).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar_color = if clamped >= 85.0 {
        theme.danger
    } else if clamped >= 65.0 {
        theme.accent
    } else {
        theme.primary
    };
    let prefix_w = padded.width() + 2 + bar_width + 4;
    let max_suf_w = (area_width as usize).saturating_sub(prefix_w);
    let suf_str = if area_width == u16::MAX {
        suffix.to_string()
    } else if max_suf_w > 0 {
        truncate_display_width(suffix, max_suf_w)
    } else {
        String::new()
    };

    Line::from(vec![
        Span::styled(
            format!("{padded}  "),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(theme.muted)),
        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
        Span::styled("░".repeat(empty), Style::default().fg(theme.muted)),
        Span::styled("]  ", Style::default().fg(theme.muted)),
        Span::styled(suf_str, Style::default().fg(theme.text)),
    ])
}

pub fn truncate_display_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let clean;
    let src = if s.contains(['\n', '\r', '\t']) {
        clean = s
            .chars()
            .map(|c| match c {
                '\n' | '\r' | '\t' => ' ',
                other => other,
            })
            .collect::<String>();
        clean.as_str()
    } else {
        s
    };

    if src.width() <= max_width {
        return src.to_string();
    }
    if max_width == 1 {
        return "…".to_string();
    }

    let limit = max_width - 1;
    let mut out = String::new();
    let mut used = 0usize;
    for ch in src.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > limit {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push('…');
    out
}

pub fn pad_display_width(s: &str, width: usize) -> String {
    let mut out = truncate_display_width(s, width);
    let w = out.width();
    if w < width {
        out.push_str(&" ".repeat(width - w));
    }
    out
}

pub fn allocate_three_column_widths(
    available_width: usize,
    preferred_name_width: usize,
    min_name_width: usize,
    preferred_tag_width: usize,
) -> (usize, usize, usize) {
    if available_width == 0 {
        return (0, 0, 0);
    }

    let min_name = min_name_width.min(available_width).max(1);
    let pref_name = preferred_name_width.max(min_name).min(available_width);

    if preferred_tag_width == 0 {
        let desc_budget = available_width.saturating_sub(pref_name);
        if desc_budget >= 4 {
            return (pref_name, 0, desc_budget);
        }
        if available_width >= min_name + 4 {
            let name_w = available_width.saturating_sub(4).min(pref_name);
            return (name_w, 0, available_width - name_w);
        }
        return (available_width, 0, 0);
    }

    if available_width >= pref_name + preferred_tag_width + 4 {
        let desc_w = available_width - pref_name - preferred_tag_width;
        return (pref_name, preferred_tag_width, desc_w);
    }

    if available_width >= min_name + preferred_tag_width + 4 {
        let name_w = available_width - preferred_tag_width - 4;
        let desc_w = available_width - name_w - preferred_tag_width;
        return (name_w, preferred_tag_width, desc_w);
    }

    if available_width >= min_name + preferred_tag_width {
        let name_w = available_width - preferred_tag_width;
        return (name_w, preferred_tag_width, 0);
    }

    (available_width, 0, 0)
}

pub fn render_command_line(
    frame: &mut Frame,
    screen: Rect,
    cmd_state: &CommandLineState,
    ctx: &RenderContext,
) {
    if screen.height == 0 || screen.width == 0 {
        return;
    }

    let cmd_rect = Rect::new(screen.x, screen.bottom().saturating_sub(1), screen.width, 1);
    let base_bg = if ctx.transparent_bg {
        Color::Reset
    } else {
        ctx.theme.background
    };
    let bar_style = Style::default().fg(ctx.theme.text).bg(base_bg);

    // Integrate the command line directly on the absolute bottom row of the terminal
    // with no container, window, borders, title, or panel background.
    let buf = frame.buffer_mut();
    for x in cmd_rect.x..cmd_rect.right() {
        if let Some(cell) = buf.cell_mut((x, cmd_rect.y)) {
            cell.set_symbol(" ");
            cell.set_style(bar_style);
        }
    }

    if cmd_state.active {
        if cmd_state.popup_open && !cmd_state.suggestions.is_empty() && screen.height >= 2 {
            render_vim_wildmenu(frame, screen, cmd_rect.y, cmd_state, ctx, base_bg);
        }

        let cursor = cmd_state.cursor.min(cmd_state.buffer.len());
        let before = &cmd_state.buffer[..cursor];
        let (cursor_ch, after) = if cursor < cmd_state.buffer.len() {
            let ch = cmd_state.buffer[cursor..].chars().next().unwrap_or(' ');
            let next = cursor + ch.len_utf8();
            (ch.to_string(), &cmd_state.buffer[next..])
        } else {
            (" ".to_string(), "")
        };

        let cursor_fg = if base_bg == Color::Reset {
            Color::Black
        } else {
            base_bg
        };

        let line = Line::from(vec![
            Span::styled(
                ":",
                Style::default()
                    .fg(ctx.theme.primary)
                    .bg(base_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(before.to_string(), bar_style),
            Span::styled(
                cursor_ch,
                Style::default()
                    .fg(cursor_fg)
                    .bg(ctx.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after.to_string(), bar_style),
        ]);
        frame.render_widget(Paragraph::new(line).style(bar_style), cmd_rect);
    } else if let Some(msg) = ctx.status_toast {
        let msg_text = truncate_display_width(msg, cmd_rect.width as usize);
        let line = Line::from(Span::styled(
            msg_text,
            Style::default().fg(ctx.theme.accent).bg(base_bg),
        ));
        frame.render_widget(Paragraph::new(line).style(bar_style), cmd_rect);
    }
}

pub fn render_command_line_with_footer(
    frame: &mut Frame,
    screen: Rect,
    _footer_rect: Option<Rect>,
    cmd_state: &CommandLineState,
    ctx: &RenderContext,
) {
    render_command_line(frame, screen, cmd_state, ctx);
}

fn render_vim_wildmenu(
    frame: &mut Frame,
    screen: Rect,
    cmd_y: u16,
    cmd_state: &CommandLineState,
    ctx: &RenderContext,
    base_bg: Color,
) {
    if cmd_y <= screen.y || screen.width < 4 {
        return;
    }

    let wild_y = cmd_y - 1;
    let wild_rect = Rect::new(screen.x, wild_y, screen.width, 1);
    let strip_style = Style::default().fg(ctx.theme.text).bg(base_bg);

    let buf = frame.buffer_mut();
    for x in wild_rect.x..wild_rect.right() {
        if let Some(cell) = buf.cell_mut((x, wild_rect.y)) {
            cell.set_symbol(" ");
            cell.set_style(strip_style);
        }
    }

    let sel = cmd_state.selected_suggestion;
    let total = cmd_state.suggestions.len();
    let max_w = wild_rect.width as usize;

    // Determine starting index so the selected suggestion stays visible in the horizontal wildmenu.
    let mut start_idx = 0usize;
    if let Some(sel_idx) = sel {
        let mut width_from_start = 0usize;
        for i in 0..=sel_idx.min(total.saturating_sub(1)) {
            width_from_start += cmd_state.suggestions[i].label.width() + 2;
        }
        while width_from_start > max_w.saturating_sub(4) && start_idx < sel_idx {
            width_from_start =
                width_from_start.saturating_sub(cmd_state.suggestions[start_idx].label.width() + 2);
            start_idx += 1;
        }
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used_w = 0usize;
    if start_idx > 0 {
        spans.push(Span::styled(
            "< ",
            Style::default().fg(ctx.theme.muted).bg(base_bg),
        ));
        used_w += 2;
    }

    let hi_fg = if base_bg == Color::Reset {
        Color::Black
    } else {
        base_bg
    };

    let mut clipped_right = false;
    for (idx, item) in cmd_state.suggestions.iter().enumerate().skip(start_idx) {
        let token = format!(" {} ", item.label);
        let tw = token.width();
        if used_w + tw + 2 > max_w && !spans.is_empty() {
            clipped_right = idx < total;
            break;
        }
        let is_selected = sel == Some(idx);
        let style = if is_selected {
            Style::default()
                .fg(hi_fg)
                .bg(ctx.theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(ctx.theme.primary).bg(base_bg)
        };
        spans.push(Span::styled(token, style));
        used_w += tw;
    }

    if clipped_right && used_w + 2 <= max_w {
        spans.push(Span::styled(
            " >",
            Style::default().fg(ctx.theme.muted).bg(base_bg),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(strip_style), wild_rect);

    // Optional borderless single-line description/arg hint directly above wildmenu for the selected item.
    if let Some(sel_idx) = sel
        && let Some(item) = cmd_state.suggestions.get(sel_idx)
        && wild_y > screen.y
    {
        let hint_y = wild_y - 1;
        let hint_rect = Rect::new(screen.x, hint_y, screen.width, 1);
        let buf = frame.buffer_mut();
        for x in hint_rect.x..hint_rect.right() {
            if let Some(cell) = buf.cell_mut((x, hint_rect.y)) {
                cell.set_symbol(" ");
                cell.set_style(strip_style);
            }
        }
        let hint_text = match item.expected_arg.as_deref() {
            Some(arg) if !arg.is_empty() => format!(" {} {} - {}", item.label, arg, item.description),
            _ => format!(" {} - {}", item.label, item.description),
        };
        let truncated = truncate_display_width(&hint_text, hint_rect.width as usize);
        let hint_line = Line::from(Span::styled(
            truncated,
            Style::default().fg(ctx.theme.muted).bg(base_bg),
        ));
        frame.render_widget(Paragraph::new(hint_line).style(strip_style), hint_rect);
    }
}
