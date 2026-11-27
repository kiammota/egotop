use crate::command::CommandLineState;
use crate::config::{Align, BorderStyle, Config, GraphStyle, ViewMode};
use crate::system::{SystemSnapshot, format_bytes, uptime::format_uptime};
use crate::ui::components::{
    ComponentAnimState, ComponentAnimationStore, render_animated_component, render_clipped,
};
use crate::ui::container::{
    ComponentCategory, ComponentConfig, ComponentKind, ContainerBorderConfig,
};
use crate::ui::layout::{ComputedLayout, PlacedComponent, PlacedContainer};
use crate::ui::theme::{Theme, parse_color};
use crate::ui::typography::{
    IconSet, RevealState, render_figlet_lines, resolve_logo_lines, slice_marquee_lines,
};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Sparkline, Wrap};
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
        if self.transparent_bg {
            Color::Reset
        } else {
            self.theme.panel_background
        }
    }

    pub fn panel_style(&self) -> Style {
        Style::default().fg(self.theme.text).bg(self.panel_bg())
    }

    pub fn border_style(&self, override_color: Option<&str>) -> Style {
        let color = override_color
            .map(|c| parse_color(c, self.theme.border))
            .unwrap_or(self.theme.border);
        Style::default().fg(color).bg(self.panel_bg())
    }
}

pub fn render_container_tree(
    frame: &mut Frame,
    layout: &ComputedLayout,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    for container in &layout.containers {
        render_container_box(frame, container, ctx);
    }

    for component in &layout.components {
        render_placed_component(frame, component, anim_store, ctx);
    }
}

fn render_container_box(frame: &mut Frame, container: &PlacedContainer, ctx: &RenderContext) {
    let area = container.outer_rect.intersection(frame.area());
    if area.width == 0 || area.height == 0 {
        return;
    }

    let bg = container
        .background
        .as_deref()
        .map(|c| parse_color(c, ctx.panel_bg()))
        .unwrap_or_else(|| ctx.panel_bg());

    let show_border = container.border.enabled
        && container.border.style != BorderStyle::None
        && ctx.config.theme.border_type != BorderStyle::None;
    if show_border && area.width >= 2 && area.height >= 2 {
        render_border_block(frame, area, &container.border, bg, ctx);
    } else if container.background.is_some() && bg != Color::Reset {
        frame.render_widget(Block::default().style(Style::default().bg(bg)), area);
    }
}

fn render_placed_component(
    frame: &mut Frame,
    placed: &PlacedComponent,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    let outer = placed.outer_rect.intersection(frame.area());
    if outer.width == 0 || outer.height == 0 {
        return;
    }

    let comp = &placed.config;
    let bg = comp
        .visual
        .background
        .as_deref()
        .map(|c| parse_color(c, ctx.panel_bg()))
        .unwrap_or_else(|| ctx.panel_bg());

    let show_border = comp.visual.border.enabled
        && comp.visual.border.style != BorderStyle::None
        && ctx.config.theme.border_type != BorderStyle::None;
    if show_border && outer.width >= 2 && outer.height >= 2 {
        let mut border = comp.visual.border.clone();
        if border.title.is_none() {
            border.title = comp.content.title.clone();
        }
        render_border_block(frame, outer, &border, bg, ctx);
    } else if bg != Color::Reset && !ctx.transparent_bg {
        frame.render_widget(
            Block::default().style(Style::default().fg(ctx.theme.text).bg(bg)),
            outer,
        );
    }

    let inner = placed.inner_rect.intersection(outer);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    match comp.category {
        ComponentCategory::Static => render_static_component(frame, inner, comp, anim_store, ctx),
        ComponentCategory::Dynamic => render_dynamic_component(frame, inner, comp, ctx),
        ComponentCategory::Animated => {
            render_animated_component(frame, inner, comp, anim_store, ctx.config, ctx.theme);
        }
    }
}

fn render_border_block(
    frame: &mut Frame,
    area: Rect,
    border: &ContainerBorderConfig,
    bg: Color,
    ctx: &RenderContext,
) {
    let border_color = border
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.border))
        .unwrap_or(ctx.theme.border);
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_set(ctx.theme.borders.as_ratatui_set())
        .border_style(Style::default().fg(border_color).bg(bg))
        .style(Style::default().fg(ctx.theme.text).bg(bg));

    if let Some(title) = &border.title
        && !title.is_empty()
    {
        block = block.title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(ctx.theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if bg != Color::Reset && !ctx.transparent_bg {
        frame.render_widget(Clear, area);
    }
    frame.render_widget(block, area);
}

fn render_static_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    anim_store: &ComponentAnimationStore,
    ctx: &RenderContext,
) {
    match comp.kind {
        ComponentKind::Greeting => render_greeting_component(frame, area, comp, anim_store, ctx),
        ComponentKind::AsciiLogo => render_logo_component(frame, area, comp, ctx),
        ComponentKind::Separator => render_separator_component(frame, area, comp, ctx),
        _ => render_text_component(frame, area, comp, ctx),
    }
}

fn render_dynamic_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    match comp.kind {
        ComponentKind::Header => render_header_component(frame, area, ctx),
        ComponentKind::System => render_system_component(frame, area, comp, ctx),
        ComponentKind::Hardware => render_hardware_component(frame, area, ctx),
        ComponentKind::Activity => render_activity_component(frame, area, comp, ctx),
        ComponentKind::Session => render_session_component(frame, area, ctx),
        ComponentKind::Cpu => render_cpu_component(frame, area, ctx),
        ComponentKind::Memory => render_memory_component(frame, area, ctx),
        ComponentKind::Temperature => render_temperature_component(frame, area, ctx),
        ComponentKind::Processes => render_processes_component(frame, area, ctx),
        ComponentKind::Network => render_network_component(frame, area, ctx),
        ComponentKind::Uptime => render_uptime_component(frame, area, ctx),
        ComponentKind::Graph => render_graph_component(frame, area, comp, ctx),
        _ => render_text_component(frame, area, comp, ctx),
    }
}

fn to_alignment(align: Align) -> Alignment {
    match align {
        Align::Left => Alignment::Left,
        Align::Center => Alignment::Center,
        Align::Right => Alignment::Right,
    }
}

fn render_text_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    let text = comp
        .content
        .text
        .as_deref()
        .unwrap_or(&ctx.config.general.message);
    let fg = comp
        .visual
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.text))
        .unwrap_or(ctx.theme.text);
    let bg = comp
        .visual
        .background
        .as_deref()
        .map(|c| parse_color(c, ctx.panel_bg()))
        .unwrap_or_else(|| ctx.panel_bg());

    let lines: Vec<Line> = text
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(fg).bg(bg))))
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(to_alignment(comp.visual.align))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_separator_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    let ch = comp
        .content
        .separator_char
        .as_deref()
        .and_then(|s| s.chars().next())
        .unwrap_or('─');
    let fg = comp
        .visual
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.border))
        .unwrap_or(ctx.theme.border);
    let bg = ctx.panel_bg();

    render_clipped(frame, area, |buf, local| {
        let style = Style::default().fg(fg).bg(bg);
        if local.height == 1 {
            let line: String = std::iter::repeat_n(ch, local.width as usize).collect();
            buf.set_stringn(0, 0, &line, local.width as usize, style);
        } else {
            let x = local.width / 2;
            for y in 0..local.height {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(style);
                }
            }
        }
    });
}

fn render_logo_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    let logo_lines = if let Some(custom) = &comp.content.text {
        custom.lines().map(ToString::to_string).collect()
    } else {
        resolve_logo_lines(&ctx.config.typography, ctx.snapshot)
    };
    let fg = comp
        .visual
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
    frame.render_widget(
        Paragraph::new(center_vertically(lines, area.height))
            .alignment(to_alignment(comp.visual.align)),
        area,
    );
}

fn render_greeting_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
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

    let raw_template = comp
        .content
        .text
        .as_deref()
        .unwrap_or(&ctx.config.general.greeting);
    let greeting_text = raw_template
        .replace("{user}", &user)
        .replace("{host}", host)
        .replace("{distro}", distro)
        .replace("{wm}", wm);

    let logo_lines = if ctx.view_mode == ViewMode::Dashboard {
        resolve_logo_lines(&ctx.config.typography, ctx.snapshot)
    } else {
        Vec::new()
    };
    let logo_width = logo_lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;

    let show_side_logo =
        !logo_lines.is_empty() && area.width > logo_width + 28 && area.height >= 5;
    let text_area = if show_side_logo {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(logo_width + 4), Constraint::Min(10)])
            .split(area);

        let logo_styled: Vec<Line<'static>> = logo_lines
            .iter()
            .map(|l| {
                Line::from(Span::styled(
                    l.clone(),
                    Style::default()
                        .fg(ctx.theme.secondary)
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect();
        let logo_para = Paragraph::new(center_vertically(logo_styled, cols[0].height))
            .alignment(Alignment::Center);
        frame.render_widget(logo_para, cols[0]);
        cols[1]
    } else {
        area
    };

    let font_name = comp
        .visual
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
        let offset = match anim_store.get(&comp.id) {
            Some(ComponentAnimState::Marquee(st)) => st.offset_cols.floor() as usize,
            _ => 0,
        };
        slice_marquee_lines(&raw_figlet, text_area.width as usize, offset, 6)
    } else {
        raw_figlet
    };

    let accent_color = comp
        .visual
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
        to_alignment(comp.visual.align)
    };
    let para = Paragraph::new(center_vertically(lines, text_area.height)).alignment(align);
    frame.render_widget(para, text_area);
}

fn render_header_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let now = Local::now();
    let icons = IconSet::from_config(&ctx.config.typography);

    let brand_label = format!(" {} ", ctx.config.general.name.to_uppercase());
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
            "PAUSED".to_string()
        } else {
            ctx.config.background.effect.label().to_uppercase()
        };
        let anim_color = if ctx.paused {
            ctx.theme.accent
        } else {
            ctx.theme.secondary
        };
        left_spans.push(Span::styled(
            format!("[FX: {anim_label}] "),
            Style::default().fg(anim_color),
        ));
        left_spans.push(Span::styled(
            format!("[{}]", ctx.view_mode.label()),
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
        frame.render_widget(
            Paragraph::new(Line::from(left_spans)).style(ctx.panel_style()),
            left_rect,
        );
    }
    if center_rect.width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(brand_span))
                .alignment(Alignment::Center)
                .style(ctx.panel_style()),
            center_rect,
        );
    }
    if right_rect.width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                right_text,
                Style::default()
                    .fg(ctx.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Right)
            .style(ctx.panel_style()),
            right_rect,
        );
    }
}

fn render_system_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let os = &ctx.snapshot.os;
    let sess = &ctx.snapshot.session;
    let hide = ctx.config.general.hide_unavailable;

    let candidates = [
        ("distro", format!("{} Distro", icons.os()), os.distro_name.clone()),
        ("kernel", format!("{} Kernel", icons.kernel()), os.kernel.clone()),
        ("arch", format!("{} Arch", icons.kernel()), os.arch.clone()),
        ("host", format!("{} Host", icons.host()), os.hostname.clone()),
        (
            "user",
            format!("{} User", icons.user()),
            ctx.config
                .general
                .user_override
                .clone()
                .or_else(|| os.username.clone()),
        ),
        ("shell", format!("{} Shell", icons.shell()), os.shell.clone()),
        (
            "terminal",
            format!("{} Terminal", icons.terminal()),
            os.terminal.clone(),
        ),
        (
            "wm",
            format!("{} DE/WM", icons.wm()),
            sess.window_manager
                .clone()
                .or_else(|| sess.desktop_env.clone()),
        ),
        (
            "session",
            format!("{} Session", icons.wm()),
            sess.session_type.clone(),
        ),
        (
            "resolution",
            format!("{} Term Size", icons.terminal()),
            Some(format!("{}x{}", sess.terminal_cols, sess.terminal_rows)),
        ),
    ];

    let mut lines = Vec::new();
    for (key, label, value) in candidates {
        if let Some(allowed) = &comp.content.fields
            && !allowed.iter().any(|f| f.eq_ignore_ascii_case(key))
        {
            continue;
        }
        match value {
            Some(val) => lines.push(kv_line(&label, &val, ctx.theme)),
            None if !hide => lines.push(kv_line(&label, "N/A", ctx.theme)),
            None => {}
        }
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_hardware_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let hw = &ctx.snapshot.hardware;
    let hide = ctx.config.general.hide_unavailable;
    let bar_width = area.width.saturating_sub(38).clamp(6, 18) as usize;

    let mut lines = Vec::new();

    let cpu_label = hw
        .cpu
        .model
        .as_deref()
        .map(|m| truncate_display_width(m, area.width.saturating_sub(14) as usize));
    if let Some(m) = cpu_label {
        lines.push(kv_line(&format!("{} CPU", icons.cpu()), &m, ctx.theme));
    } else if !hide {
        lines.push(kv_line(&format!("{} CPU", icons.cpu()), "N/A", ctx.theme));
    }

    let cpu_usage_line = match (hw.cpu.usage_percent, hw.cpu.temp_celsius) {
        (Some(pct), Some(temp)) => Some(bar_line(
            "   CPU Usage",
            pct,
            &format!("{pct:.1}% ({temp:.0}°C)"),
            bar_width,
            ctx.theme,
        )),
        (Some(pct), None) => Some(bar_line(
            "   CPU Usage",
            pct,
            &format!("{pct:.1}% ({} cores)", hw.cpu.cores),
            bar_width,
            ctx.theme,
        )),
        (None, Some(temp)) => Some(kv_line("   CPU Temp", &format!("{temp:.1}°C"), ctx.theme)),
        (None, None) if !hide => Some(kv_line("   CPU Usage", "N/A", ctx.theme)),
        (None, None) => None,
    };
    if let Some(line) = cpu_usage_line {
        lines.push(line);
    }

    if let (Some(used), Some(total), Some(pct)) = (
        hw.memory.used_bytes,
        hw.memory.total_bytes,
        hw.memory.usage_percent(),
    ) {
        let info = format!("{} / {} ({pct:.0}%)", format_bytes(used), format_bytes(total));
        lines.push(bar_line(
            &format!("{} RAM", icons.memory()),
            pct,
            &info,
            bar_width,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(
            &format!("{} RAM", icons.memory()),
            "N/A",
            ctx.theme,
        ));
    }

    if let (Some(s_used), Some(s_total)) = (hw.memory.swap_used_bytes, hw.memory.swap_total_bytes) {
        let pct = ((s_used as f64 / s_total.max(1) as f64) * 100.0) as f32;
        lines.push(kv_line(
            "   Swap",
            &format!(
                "{} / {} ({pct:.0}%)",
                format_bytes(s_used),
                format_bytes(s_total)
            ),
            ctx.theme,
        ));
    }

    if let Some(disk) = hw.disks.first() {
        let pct = disk.usage_percent();
        let info = format!(
            "{} / {} ({pct:.0}%)",
            format_bytes(disk.used_bytes),
            format_bytes(disk.total_bytes)
        );
        lines.push(bar_line(
            &format!("{} Disk ({})", icons.disk(), disk.mount_point),
            pct,
            &info,
            bar_width,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(&format!("{} Disk", icons.disk()), "N/A", ctx.theme));
    }

    if let Some(gpu) = hw.gpus.first() {
        let mut gpu_str = gpu.name.clone().unwrap_or_else(|| "GPU".to_string());
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
        lines.push(kv_line(&format!("{} GPU", icons.gpu()), &gpu_str, ctx.theme));
    } else if !hide {
        lines.push(kv_line(&format!("{} GPU", icons.gpu()), "N/A", ctx.theme));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_activity_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
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

    let info_lines = vec![
        kv_line(
            &format!("{} Load (1/5/15)", icons.activity()),
            &load_str,
            ctx.theme,
        ),
        kv_line("   Processes", &procs_str, ctx.theme),
        kv_line("   Threads", &threads_str, ctx.theme),
    ];

    let graph_style = comp.visual.graph_style.unwrap_or(GraphStyle::Sparkline);
    if area.height >= 7 && graph_style == GraphStyle::Sparkline {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(2),
            ])
            .split(area);

        frame.render_widget(Paragraph::new(info_lines), chunks[0]);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "CPU % / RAM % History:",
                Style::default().fg(ctx.theme.muted),
            ))),
            chunks[1],
        );

        let graph_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        let cpu_data: Vec<u64> = ctx.snapshot.cpu_history.iter().copied().collect();
        let mem_data: Vec<u64> = ctx.snapshot.mem_history.iter().copied().collect();

        frame.render_widget(
            Sparkline::default()
                .data(&cpu_data)
                .max(100)
                .style(Style::default().fg(ctx.theme.primary)),
            graph_cols[0],
        );
        frame.render_widget(
            Sparkline::default()
                .data(&mem_data)
                .max(100)
                .style(Style::default().fg(ctx.theme.secondary)),
            graph_cols[1],
        );
    } else {
        frame.render_widget(Paragraph::new(info_lines).wrap(Wrap { trim: true }), area);
    }
}

fn render_session_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let icons = IconSet::from_config(&ctx.config.typography);
    let act = &ctx.snapshot.activity;
    let sess = &ctx.snapshot.session;
    let hide = ctx.config.general.hide_unavailable;

    let uptime_str = act.uptime_secs.map(format_uptime);
    let boot_str = act
        .boot_time
        .map(|bt| bt.format("%d/%m %H:%M:%S").to_string());

    let now = Local::now();
    let mut lines = Vec::new();
    if let Some(up) = uptime_str {
        lines.push(kv_line(
            &format!("{} Uptime", icons.uptime()),
            &up,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(
            &format!("{} Uptime", icons.uptime()),
            "N/A",
            ctx.theme,
        ));
    }

    if let Some(boot) = boot_str {
        lines.push(kv_line(
            &format!("{} Boot", icons.clock()),
            &boot,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(
            &format!("{} Boot", icons.clock()),
            "N/A",
            ctx.theme,
        ));
    }

    lines.push(kv_line(
        "   Time",
        &now.format(&ctx.config.general.time_format).to_string(),
        ctx.theme,
    ));

    if let Some(disp) = &sess.display {
        let kind = sess.session_type.as_deref().unwrap_or("Display");
        lines.push(kv_line(&format!("{} {kind}", icons.wm()), disp, ctx.theme));
    }

    if sess.is_hyprland {
        lines.push(kv_line("   Compositor", "Hyprland active", ctx.theme));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_cpu_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let cpu = &ctx.snapshot.hardware.cpu;
    let bar_width = area.width.saturating_sub(28).clamp(6, 20) as usize;
    let mut lines = Vec::new();
    if let Some(model) = &cpu.model {
        lines.push(kv_line(
            "CPU",
            &truncate_display_width(model, area.width.saturating_sub(16) as usize),
            ctx.theme,
        ));
    }
    if let Some(pct) = cpu.usage_percent {
        lines.push(bar_line(
            "Total",
            pct,
            &format!("{pct:.1}% ({} cores)", cpu.cores),
            bar_width,
            ctx.theme,
        ));
    }
    for (idx, pct) in cpu.per_core_percent.iter().enumerate() {
        if lines.len() >= area.height as usize {
            break;
        }
        lines.push(bar_line(
            &format!("Core {idx}"),
            *pct,
            &format!("{pct:.0}%"),
            bar_width,
            ctx.theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_memory_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let mem = &ctx.snapshot.hardware.memory;
    let bar_width = area.width.saturating_sub(32).clamp(6, 20) as usize;
    let mut lines = Vec::new();
    if let (Some(used), Some(total), Some(pct)) =
        (mem.used_bytes, mem.total_bytes, mem.usage_percent())
    {
        lines.push(bar_line(
            "RAM",
            pct,
            &format!("{} / {} ({pct:.0}%)", format_bytes(used), format_bytes(total)),
            bar_width,
            ctx.theme,
        ));
    }
    if let Some(avail) = mem.available_bytes {
        lines.push(kv_line("Available", &format_bytes(avail), ctx.theme));
    }
    if let (Some(su), Some(st)) = (mem.swap_used_bytes, mem.swap_total_bytes) {
        let pct = (su as f64 / st.max(1) as f64 * 100.0) as f32;
        lines.push(bar_line(
            "Swap",
            pct,
            &format!("{} / {}", format_bytes(su), format_bytes(st)),
            bar_width,
            ctx.theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_temperature_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let temp = ctx.snapshot.hardware.cpu.temp_celsius;
    let line = match temp {
        Some(t) => kv_line("CPU Temp", &format!("{t:.1} °C"), ctx.theme),
        None => kv_line("CPU Temp", "Unavailable", ctx.theme),
    };
    frame.render_widget(Paragraph::new(vec![line]).wrap(Wrap { trim: true }), area);
}

fn render_processes_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let act = &ctx.snapshot.activity;
    let lines = vec![
        kv_line(
            "Processes",
            &act.process_count
                .map(|p| p.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            ctx.theme,
        ),
        kv_line(
            "Threads",
            &act.thread_count
                .map(|t| t.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            ctx.theme,
        ),
        kv_line(
            "Runnable",
            &act.runnable_threads
                .map(|r| r.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            ctx.theme,
        ),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_network_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let net = &ctx.snapshot.network;
    let mut lines = vec![
        kv_line(
            "RX Rate",
            &format!("{}/s", format_bytes(net.total_rx_bps)),
            ctx.theme,
        ),
        kv_line(
            "TX Rate",
            &format!("{}/s", format_bytes(net.total_tx_bps)),
            ctx.theme,
        ),
    ];
    for iface in net.interfaces.iter().take(3) {
        lines.push(kv_line(
            &iface.name,
            &format!(
                "↓{} ↑{}",
                format_bytes(iface.rx_bytes),
                format_bytes(iface.tx_bytes)
            ),
            ctx.theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_uptime_component(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    render_session_component(frame, area, ctx);
}

fn render_graph_component(
    frame: &mut Frame,
    area: Rect,
    comp: &ComponentConfig,
    ctx: &RenderContext,
) {
    let metric = comp
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
    frame.render_widget(
        Sparkline::default()
            .data(&data)
            .max(max_val)
            .style(Style::default().fg(color)),
        area,
    );
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

fn pad_label(label: &str, target_width: usize) -> String {
    let w = label.width();
    if w >= target_width {
        label.to_string()
    } else {
        format!("{label}{}", " ".repeat(target_width - w))
    }
}

fn kv_line<'a>(label: &str, value: &str, theme: &Theme) -> Line<'a> {
    let padded = pad_label(label, 14);
    Line::from(vec![
        Span::styled(
            format!("{padded} "),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(value.to_string(), Style::default().fg(theme.text)),
    ])
}

fn bar_line<'a>(label: &str, pct: f32, suffix: &str, bar_width: usize, theme: &Theme) -> Line<'a> {
    let padded = pad_label(label, 14);
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

    Line::from(vec![
        Span::styled(
            format!("{padded} "),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[", Style::default().fg(theme.muted)),
        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
        Span::styled("░".repeat(empty), Style::default().fg(theme.muted)),
        Span::styled("] ", Style::default().fg(theme.muted)),
        Span::styled(suffix.to_string(), Style::default().fg(theme.text)),
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
    let bar = Rect::new(screen.x, screen.bottom() - 1, screen.width, 1);

    if cmd_state.active {
        if cmd_state.popup_open && !cmd_state.suggestions.is_empty() && screen.height >= 5 {
            render_completion_popup(frame, screen, cmd_state, ctx);
        }

        if !ctx.transparent_bg {
            frame.render_widget(Clear, bar);
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

        let line = Line::from(vec![
            Span::styled(
                ":",
                Style::default()
                    .fg(ctx.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(before.to_string(), Style::default().fg(ctx.theme.text)),
            Span::styled(
                cursor_ch,
                Style::default()
                    .fg(ctx.theme.background)
                    .bg(ctx.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(after.to_string(), Style::default().fg(ctx.theme.text)),
        ]);
        frame.render_widget(Paragraph::new(line).style(ctx.panel_style()), bar);
    } else if let Some(msg) = ctx.status_toast {
        if !ctx.transparent_bg {
            frame.render_widget(Clear, bar);
        }
        let line = Line::from(Span::styled(
            format!(" ✦ {msg}"),
            Style::default()
                .fg(ctx.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(Paragraph::new(line).style(ctx.panel_style()), bar);
    }
}

fn render_completion_popup(
    frame: &mut Frame,
    screen: Rect,
    cmd_state: &CommandLineState,
    ctx: &RenderContext,
) {
    let total = cmd_state.suggestions.len();
    let max_visible = 7usize.min(screen.height.saturating_sub(4) as usize).max(1);
    let shown_count = total.min(max_visible);
    let popup_h = (shown_count as u16) + 2;

    let popup_w = 72u16
        .min(screen.width.saturating_sub(2))
        .max(20.min(screen.width));
    if popup_w < 12 || popup_h < 3 || screen.height <= popup_h {
        return;
    }

    let popup_x = screen.x + 1.min(screen.width.saturating_sub(popup_w));
    let popup_y = screen.bottom().saturating_sub(1 + popup_h);
    let popup = Rect::new(popup_x, popup_y, popup_w, popup_h);

    if !ctx.transparent_bg {
        frame.render_widget(Clear, popup);
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ctx.theme.borders.as_ratatui_set())
        .border_style(ctx.border_style(None))
        .style(ctx.panel_style());
    let inner_raw = block.inner(popup);
    frame.render_widget(block, popup);

    let inner = if inner_raw.width >= 4 {
        Rect::new(
            inner_raw.x + 1,
            inner_raw.y,
            inner_raw.width - 2,
            inner_raw.height,
        )
    } else {
        inner_raw
    };

    let sel = cmd_state.selected_suggestion;
    let scroll_start = match sel {
        Some(idx) if idx >= shown_count => idx + 1 - shown_count,
        _ => 0,
    };

    let avail_width = inner.width as usize;
    if avail_width <= 2 {
        return;
    }

    let visible: Vec<_> = cmd_state
        .suggestions
        .iter()
        .skip(scroll_start)
        .take(shown_count)
        .collect();
    let max_label_w = visible
        .iter()
        .map(|i| i.label.width())
        .max()
        .unwrap_or(8);
    let pref_name_w = (max_label_w + 1).clamp(10, 18);
    let max_arg_w = visible
        .iter()
        .filter_map(|i| i.expected_arg.as_deref())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let pref_arg_w = if max_arg_w > 0 {
        (max_arg_w + 1).clamp(6, 14)
    } else {
        0
    };

    let prefix_w = 2usize;
    let content_avail = avail_width.saturating_sub(prefix_w);
    let (name_w, arg_w, desc_w) =
        allocate_three_column_widths(content_avail, pref_name_w, 8, pref_arg_w);

    let panel_bg = ctx.panel_bg();
    let mut lines = Vec::with_capacity(shown_count);
    for (rel_idx, item) in visible.into_iter().enumerate() {
        let abs_idx = scroll_start + rel_idx;
        let is_selected = sel == Some(abs_idx);

        let name_col = if arg_w > 0 || desc_w > 0 {
            format!("{} ", pad_display_width(&item.label, name_w.saturating_sub(1)))
        } else {
            pad_display_width(&item.label, name_w)
        };
        let arg_col = if arg_w > 0 {
            let arg_text = item.expected_arg.as_deref().unwrap_or("");
            if desc_w > 0 {
                format!("{} ", pad_display_width(arg_text, arg_w.saturating_sub(1)))
            } else {
                pad_display_width(arg_text, arg_w)
            }
        } else {
            String::new()
        };

        let row_bg = if is_selected {
            ctx.theme.secondary
        } else {
            panel_bg
        };
        let fg_hi = ctx.theme.background;
        let prefix_fg = if is_selected { fg_hi } else { ctx.theme.primary };
        let name_fg = if is_selected { fg_hi } else { ctx.theme.primary };
        let arg_fg = if is_selected { fg_hi } else { ctx.theme.accent };
        let desc_fg = if is_selected { fg_hi } else { ctx.theme.muted };

        let desc = if desc_w > 0 {
            if is_selected {
                pad_display_width(&item.description, desc_w)
            } else {
                truncate_display_width(&item.description, desc_w)
            }
        } else {
            String::new()
        };

        let mut spans = vec![
            Span::styled(
                if is_selected { "▶ " } else { "  " },
                Style::default().fg(prefix_fg).bg(row_bg),
            ),
            Span::styled(
                name_col,
                Style::default()
                    .fg(name_fg)
                    .bg(row_bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if arg_w > 0 {
            spans.push(Span::styled(arg_col, Style::default().fg(arg_fg).bg(row_bg)));
        }
        if desc_w > 0 {
            spans.push(Span::styled(desc, Style::default().fg(desc_fg).bg(row_bg)));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
