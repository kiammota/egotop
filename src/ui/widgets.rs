use crate::config::{Align, BorderStyle, Config, GraphStyle, ViewMode, WidgetConfig, WidgetId};
use crate::system::{SystemSnapshot, format_bytes, uptime::format_uptime};
use crate::ui::layout::ComputedLayout;
use crate::ui::theme::{Theme, parse_color};
use crate::ui::typography::{IconSet, RevealState, render_ascii_heading, resolve_logo_lines};
use crate::window::{WindowBackgroundMode, WindowState};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Sparkline, Wrap};

pub struct RenderContext<'a> {
    pub config: &'a Config,
    pub theme: &'a Theme,
    pub snapshot: &'a SystemSnapshot,
    pub reveal: &'a RevealState,
    pub active_page: u8,
    pub view_mode: ViewMode,
    pub paused: bool,
    pub show_help: bool,
    pub fps: f32,
    pub status_toast: Option<&'a str>,
    pub window_transparent: bool,
}

impl<'a> RenderContext<'a> {
    pub fn panel_bg(&self) -> Color {
        if self.window_transparent {
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

pub fn window_config_button_rect(win_rect: Rect) -> Rect {
    if win_rect.width < 16 || win_rect.height < 3 {
        return Rect::ZERO;
    }
    Rect::new(win_rect.right().saturating_sub(10), win_rect.y, 4, 1)
}

pub fn window_help_button_rect(win_rect: Rect) -> Rect {
    if win_rect.width < 16 || win_rect.height < 3 {
        return Rect::ZERO;
    }
    Rect::new(win_rect.right().saturating_sub(5), win_rect.y, 3, 1)
}

pub fn render_window_frame(frame: &mut Frame, win: &WindowState, ctx: &RenderContext) {
    let area = win.resolved_rect;
    if !win.config.visible || area.width < 4 || area.height < 3 {
        return;
    }

    let bg = match win.config.background.mode {
        WindowBackgroundMode::Transparent => Color::Reset,
        WindowBackgroundMode::Solid => win
            .config
            .background
            .color
            .as_deref()
            .map(|c| parse_color(c, ctx.theme.panel_background))
            .unwrap_or(ctx.theme.panel_background),
    };

    if bg != Color::Reset {
        frame.render_widget(Clear, area);
    }

    let win_style = Style::default().fg(ctx.theme.text).bg(bg);
    if win.config.border.enabled {
        let border_type = match win.config.border.style {
            BorderStyle::Rounded => BorderType::Rounded,
            BorderStyle::Plain => BorderType::Plain,
            BorderStyle::Double => BorderType::Double,
            BorderStyle::Thick => BorderType::Thick,
        };
        let border_color = win
            .config
            .border
            .color
            .as_deref()
            .map(|c| parse_color(c, ctx.theme.border))
            .unwrap_or(ctx.theme.border);
        let title = win
            .config
            .border
            .title
            .as_deref()
            .unwrap_or(&ctx.config.general.name);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(border_color).bg(bg))
            .style(win_style)
            .title(Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(ctx.theme.primary)
                    .add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(block, area);
    } else {
        frame.render_widget(Block::default().style(win_style), area);
    }

    // Subtle interactive action pills in top-right corner of the window frame
    let cfg_btn = window_config_button_rect(area);
    if cfg_btn.width > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "[⚙]",
                Style::default().fg(ctx.theme.secondary).bg(bg),
            )),
            cfg_btn,
        );
    }
    let help_btn = window_help_button_rect(area);
    if help_btn.width > 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "[?]",
                Style::default().fg(ctx.theme.muted).bg(bg),
            )),
            help_btn,
        );
    }

    // Subtle resize corner indicator when resizable
    if win.config.resizable && area.width >= 6 && area.height >= 4 {
        let corner = Rect::new(area.right() - 1, area.bottom() - 1, 1, 1);
        frame.render_widget(
            Paragraph::new(Span::styled(
                "◢",
                Style::default().fg(ctx.theme.accent).bg(bg),
            )),
            corner,
        );
    }
}

pub fn render_dashboard(frame: &mut Frame, layout: &ComputedLayout, ctx: &RenderContext) {
    if let Some(header_area) = layout.header {
        render_header(frame, header_area, ctx);
    }

    for placed in &layout.widgets {
        if placed.area.width < 4 || placed.area.height < 2 {
            continue;
        }
        match placed.config.id {
            WidgetId::Greeting => render_greeting_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Logo => render_logo_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::System => render_system_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Hardware => render_hardware_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Activity => render_activity_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Session => render_session_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Uptime => render_uptime_widget(frame, placed.area, &placed.config, ctx),
            WidgetId::Message => render_message_widget(frame, placed.area, &placed.config, ctx),
        }
    }
}

fn render_header(frame: &mut Frame, area: Rect, ctx: &RenderContext) {
    let now = Local::now();
    let icons = IconSet::from_config(&ctx.config.typography);

    let mut left_spans = vec![
        Span::styled(
            format!(" {} ", ctx.config.general.name.to_uppercase()),
            Style::default()
                .fg(ctx.theme.background)
                .bg(ctx.theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

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
            format!("[Pg {}] ", ctx.active_page),
            Style::default().fg(ctx.theme.primary),
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

    if area.height >= 3 {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ctx.theme.border_type)
            .border_style(ctx.border_style(None))
            .style(ctx.panel_style());
        let inner = block.inner(area);
        if !ctx.window_transparent {
            frame.render_widget(Clear, area);
        }
        frame.render_widget(block, area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(right_text.chars().count() as u16 + 1),
            ])
            .split(inner);

        frame.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                right_text,
                Style::default()
                    .fg(ctx.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Right),
            cols[1],
        );
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(right_text.chars().count() as u16 + 1),
            ])
            .split(area);
        frame.render_widget(
            Paragraph::new(Line::from(left_spans)).style(ctx.panel_style()),
            cols[0],
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                right_text,
                Style::default()
                    .fg(ctx.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Right)
            .style(ctx.panel_style()),
            cols[1],
        );
    }
}

fn panel_block<'a>(widget: &'a WidgetConfig, ctx: &'a RenderContext) -> (Block<'a>, bool) {
    let show_border = widget.show_border.unwrap_or(true);
    let title = widget
        .title
        .as_deref()
        .unwrap_or_else(|| widget.id.default_title());
    let title_color = widget
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.primary))
        .unwrap_or(ctx.theme.primary);

    let block = if show_border {
        Block::default()
            .borders(Borders::ALL)
            .border_type(ctx.theme.border_type)
            .border_style(ctx.border_style(widget.border_color.as_deref()))
            .style(ctx.panel_style())
            .title(Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(title_color)
                    .add_modifier(Modifier::BOLD),
            ))
    } else {
        Block::default().style(ctx.panel_style())
    };
    (block, show_border)
}

fn render_panel_frame(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) -> Rect {
    if !ctx.window_transparent {
        frame.render_widget(Clear, area);
    }
    let (block, _) = panel_block(widget, ctx);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

fn render_greeting_widget(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    if inner.width == 0 || inner.height == 0 {
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

    let greeting_text = ctx
        .config
        .general
        .greeting
        .replace("{user}", &user)
        .replace("{host}", host)
        .replace("{distro}", distro)
        .replace("{wm}", wm);

    let logo_lines = if ctx.view_mode == ViewMode::Dashboard {
        resolve_logo_lines(&ctx.config.typography, ctx.snapshot)
    } else {
        Vec::new()
    };
    let logo_width = logo_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;

    let show_side_logo =
        !logo_lines.is_empty() && inner.width > logo_width + 32 && inner.height >= 6;
    let text_area = if show_side_logo {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(logo_width + 3), Constraint::Min(10)])
            .split(inner);

        let logo_para = Paragraph::new(
            logo_lines
                .iter()
                .map(|l| {
                    Line::from(Span::styled(
                        l.clone(),
                        Style::default()
                            .fg(ctx.theme.secondary)
                            .add_modifier(Modifier::BOLD),
                    ))
                })
                .collect::<Vec<_>>(),
        )
        .alignment(Alignment::Center);
        frame.render_widget(logo_para, cols[0]);
        cols[1]
    } else {
        inner
    };

    let font_name = widget
        .font
        .as_deref()
        .unwrap_or(&ctx.config.typography.font);

    let figlet_lines = if text_area.height >= 7 {
        render_ascii_heading(
            &greeting_text,
            font_name,
            ctx.config.typography.ascii_fallback,
            text_area.width.saturating_sub(2),
        )
    } else {
        vec![greeting_text.clone()]
    };

    let animate = widget.reveal_animation.unwrap_or(true);
    let accent_color = widget
        .color
        .as_deref()
        .map(|c| parse_color(c, ctx.theme.primary))
        .unwrap_or(ctx.theme.primary);

    let mut lines: Vec<Line> = figlet_lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let shown = if animate {
                ctx.reveal.mask_line(&l, i)
            } else {
                l
            };
            Line::from(Span::styled(
                shown,
                Style::default()
                    .fg(accent_color)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    if !ctx.config.general.message.trim().is_empty() && lines.len() < text_area.height as usize {
        let msg = if animate {
            ctx.reveal
                .mask_line(&ctx.config.general.message, lines.len())
        } else {
            ctx.config.general.message.clone()
        };
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(ctx.theme.accent),
        )));
    }

    let align = match widget.align {
        Align::Left => Alignment::Left,
        Align::Center => Alignment::Center,
        Align::Right => Alignment::Right,
    };

    let para = Paragraph::new(lines).alignment(align);
    frame.render_widget(para, text_area);
}

fn render_logo_widget(frame: &mut Frame, area: Rect, widget: &WidgetConfig, ctx: &RenderContext) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    let logo_lines = resolve_logo_lines(&ctx.config.typography, ctx.snapshot);
    let lines: Vec<Line> = logo_lines
        .into_iter()
        .map(|l| {
            Line::from(Span::styled(
                l,
                Style::default()
                    .fg(ctx.theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

fn render_system_widget(frame: &mut Frame, area: Rect, widget: &WidgetConfig, ctx: &RenderContext) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    let icons = IconSet::from_config(&ctx.config.typography);
    let os = &ctx.snapshot.os;
    let sess = &ctx.snapshot.session;
    let hide = ctx.config.general.hide_unavailable;

    let candidates = [
        (
            "distro",
            format!("{} Distro", icons.os()),
            os.distro_name.clone(),
        ),
        (
            "kernel",
            format!("{} Kernel", icons.kernel()),
            os.kernel.clone(),
        ),
        ("arch", format!("{} Arch", icons.kernel()), os.arch.clone()),
        (
            "host",
            format!("{} Host", icons.host()),
            os.hostname.clone(),
        ),
        (
            "user",
            format!("{} Usuário", icons.user()),
            ctx.config
                .general
                .user_override
                .clone()
                .or_else(|| os.username.clone()),
        ),
        (
            "shell",
            format!("{} Shell", icons.shell()),
            os.shell.clone(),
        ),
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
            format!("{} Sessão", icons.wm()),
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
        if let Some(allowed) = &widget.fields
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

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_hardware_widget(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    let icons = IconSet::from_config(&ctx.config.typography);
    let hw = &ctx.snapshot.hardware;
    let hide = ctx.config.general.hide_unavailable;
    let bar_width = inner.width.saturating_sub(26).clamp(8, 22) as usize;

    let mut lines = Vec::new();

    // CPU
    let cpu_label = hw
        .cpu
        .model
        .as_deref()
        .map(|m| truncate_str(m, inner.width.saturating_sub(14) as usize));
    if let Some(m) = cpu_label {
        lines.push(kv_line(&format!("{} CPU", icons.cpu()), &m, ctx.theme));
    } else if !hide {
        lines.push(kv_line(&format!("{} CPU", icons.cpu()), "N/A", ctx.theme));
    }

    let cpu_usage_line = match (hw.cpu.usage_percent, hw.cpu.temp_celsius) {
        (Some(pct), Some(temp)) => Some(bar_line(
            "   Uso CPU",
            pct,
            &format!("{pct:.1}% ({temp:.0}°C)"),
            bar_width,
            ctx.theme,
        )),
        (Some(pct), None) => Some(bar_line(
            "   Uso CPU",
            pct,
            &format!("{pct:.1}% ({} cores)", hw.cpu.cores),
            bar_width,
            ctx.theme,
        )),
        (None, Some(temp)) => Some(kv_line("   Temp CPU", &format!("{temp:.1}°C"), ctx.theme)),
        (None, None) if !hide => Some(kv_line("   Uso CPU", "N/A", ctx.theme)),
        (None, None) => None,
    };
    if let Some(line) = cpu_usage_line {
        lines.push(line);
    }

    // RAM
    if let (Some(used), Some(total), Some(pct)) = (
        hw.memory.used_bytes,
        hw.memory.total_bytes,
        hw.memory.usage_percent(),
    ) {
        let info = format!(
            "{} / {} ({pct:.0}%)",
            format_bytes(used),
            format_bytes(total)
        );
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

    // Swap if present
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

    // Primary Disk
    if let Some(disk) = hw.disks.first() {
        let pct = disk.usage_percent();
        let info = format!(
            "{} / {} ({pct:.0}%)",
            format_bytes(disk.used_bytes),
            format_bytes(disk.total_bytes)
        );
        lines.push(bar_line(
            &format!("{} Disco ({})", icons.disk(), disk.mount_point),
            pct,
            &info,
            bar_width,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(
            &format!("{} Disco", icons.disk()),
            "N/A",
            ctx.theme,
        ));
    }

    // GPU
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
        lines.push(kv_line(
            &format!("{} GPU", icons.gpu()),
            &gpu_str,
            ctx.theme,
        ));
    } else if !hide {
        lines.push(kv_line(&format!("{} GPU", icons.gpu()), "N/A", ctx.theme));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_activity_widget(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

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
        (Some(run), Some(tot)) => format!("{tot} ({run} ativos)"),
        (None, Some(tot)) => tot.to_string(),
        _ => "N/A".to_string(),
    };

    let info_lines = vec![
        kv_line(
            &format!("{} Load (1/5/15)", icons.activity()),
            &load_str,
            ctx.theme,
        ),
        kv_line("   Processos", &procs_str, ctx.theme),
        kv_line("   Threads", &threads_str, ctx.theme),
    ];

    let graph_style = widget.graph_style.unwrap_or(GraphStyle::Sparkline);
    if inner.height >= 7 && graph_style == GraphStyle::Sparkline {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(2),
            ])
            .split(inner);

        frame.render_widget(Paragraph::new(info_lines), chunks[0]);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Histórico CPU % / RAM %:",
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

        let cpu_spark = Sparkline::default()
            .data(&cpu_data)
            .max(100)
            .style(Style::default().fg(ctx.theme.primary));
        let mem_spark = Sparkline::default()
            .data(&mem_data)
            .max(100)
            .style(Style::default().fg(ctx.theme.secondary));

        frame.render_widget(cpu_spark, graph_cols[0]);
        frame.render_widget(mem_spark, graph_cols[1]);
    } else {
        frame.render_widget(Paragraph::new(info_lines).wrap(Wrap { trim: true }), inner);
    }
}

fn render_session_widget(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) {
    let inner = render_panel_frame(frame, area, widget, ctx);
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
        "   Horário",
        &now.format(&ctx.config.general.time_format).to_string(),
        ctx.theme,
    ));

    if let Some(disp) = &sess.display {
        let kind = sess.session_type.as_deref().unwrap_or("Display");
        lines.push(kv_line(&format!("{} {kind}", icons.wm()), disp, ctx.theme));
    }

    if sess.is_hyprland {
        lines.push(kv_line("   Compositor", "Hyprland ativo", ctx.theme));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_uptime_widget(frame: &mut Frame, area: Rect, widget: &WidgetConfig, ctx: &RenderContext) {
    render_session_widget(frame, area, widget, ctx);
}

fn render_message_widget(
    frame: &mut Frame,
    area: Rect,
    widget: &WidgetConfig,
    ctx: &RenderContext,
) {
    let inner = render_panel_frame(frame, area, widget, ctx);
    let align = match widget.align {
        Align::Left => Alignment::Left,
        Align::Center => Alignment::Center,
        Align::Right => Alignment::Right,
    };
    let text = if ctx.active_page == 3 {
        format!(
            "{}  |  Config: ~/.config/egotop/egotop.toml",
            ctx.config.general.message
        )
    } else {
        ctx.config.general.message.clone()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().fg(ctx.theme.accent),
        )))
        .alignment(align)
        .wrap(Wrap { trim: true }),
        inner,
    );
}

fn kv_line<'a>(label: &str, value: &str, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label}: "),
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(value.to_string(), Style::default().fg(theme.text)),
    ])
}

fn bar_line<'a>(label: &str, pct: f32, suffix: &str, bar_width: usize, theme: &Theme) -> Line<'a> {
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
            format!("{label}: "),
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

fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        s.chars().take(max_chars).collect()
    } else {
        let mut out: String = s.chars().take(max_chars - 1).collect();
        out.push('…');
        out
    }
}
