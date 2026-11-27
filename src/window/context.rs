use crate::config::{Align, Anchor, GraphStyle, WidgetConfig, WidgetId};
use crate::ui::layout::compute_layout;
use crate::ui::widgets::{RenderContext, render_dashboard, render_single_widget};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

/// Known window content domains rendered independently from window geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowContextKind {
    Dashboard,
    System,
    Hardware,
    Activity,
    Session,
    Greeting,
    Message,
    Custom(String),
}

impl WindowContextKind {
    pub fn from_name(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "dashboard" | "main" => Self::Dashboard,
            "system" | "sys" => Self::System,
            "hardware" | "hw" => Self::Hardware,
            "activity" => Self::Activity,
            "session" | "uptime" => Self::Session,
            "greeting" | "logo" => Self::Greeting,
            "message" => Self::Message,
            _ => Self::Custom(raw.trim().to_string()),
        }
    }
}

/// Persistent state and content attached to a window instance, preserved across
/// minimize/restore, maximize/restore, focus changes, and re-composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowContext {
    pub id: String,
    pub kind: WindowContextKind,
    pub custom_lines: Vec<String>,
    pub scroll_offset: u16,
    pub active_status: Option<String>,
}

impl WindowContext {
    pub fn from_id(id: &str) -> Self {
        let normalized = if id.trim().is_empty() {
            "dashboard"
        } else {
            id.trim()
        };
        Self {
            id: normalized.to_string(),
            kind: WindowContextKind::from_name(normalized),
            custom_lines: Vec::new(),
            scroll_offset: 0,
            active_status: None,
        }
    }
}

/// Renders a `WindowContext` inside the composed `inner` rectangle without mixing
/// window management or geometry rules into the renderer.
pub fn render_context_content(
    frame: &mut Frame,
    inner: Rect,
    window_context: Option<&WindowContext>,
    ctx: &RenderContext,
) {
    if inner.width < 6 || inner.height < 2 {
        return;
    }
    let fallback = WindowContext::from_id("dashboard");
    let wctx = window_context.unwrap_or(&fallback);

    match &wctx.kind {
        WindowContextKind::Dashboard => {
            if inner.width >= 10 && inner.height >= 4 {
                let layout = compute_layout(inner, ctx.config, ctx.active_page, ctx.view_mode);
                render_dashboard(frame, &layout, ctx);
            }
        }
        WindowContextKind::System => render_focused_widget(frame, inner, WidgetId::System, "System", ctx),
        WindowContextKind::Hardware => {
            render_focused_widget(frame, inner, WidgetId::Hardware, "Hardware", ctx);
        }
        WindowContextKind::Activity => {
            render_focused_widget(frame, inner, WidgetId::Activity, "Activity", ctx);
        }
        WindowContextKind::Session => {
            render_focused_widget(frame, inner, WidgetId::Session, "Session & Uptime", ctx);
        }
        WindowContextKind::Greeting => {
            render_focused_widget(frame, inner, WidgetId::Greeting, "Welcome", ctx);
        }
        WindowContextKind::Message => {
            render_focused_widget(frame, inner, WidgetId::Message, "Message", ctx);
        }
        WindowContextKind::Custom(name) => {
            let mut lines = Vec::new();
            lines.push(Line::from(Span::styled(
                format!("Context: {name}"),
                Style::default().fg(ctx.theme.primary),
            )));
            if let Some(status) = &wctx.active_status {
                lines.push(Line::from(Span::styled(
                    status.clone(),
                    Style::default().fg(ctx.theme.accent),
                )));
            }
            for row in &wctx.custom_lines {
                lines.push(Line::from(Span::styled(
                    row.clone(),
                    Style::default().fg(ctx.theme.text),
                )));
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .scroll((wctx.scroll_offset, 0))
                    .wrap(Wrap { trim: true })
                    .style(ctx.panel_style()),
                inner,
            );
        }
    }
}

fn render_focused_widget(
    frame: &mut Frame,
    area: Rect,
    id: WidgetId,
    default_title: &str,
    ctx: &RenderContext,
) {
    let widget = ctx
        .config
        .widgets
        .iter()
        .find(|w| w.id == id)
        .cloned()
        .unwrap_or_else(|| WidgetConfig {
            id,
            enabled: true,
            title: Some(default_title.to_string()),
            position: Anchor::Center,
            align: Align::Left,
            graph_style: (id == WidgetId::Activity).then_some(GraphStyle::Sparkline),
            show_border: Some(false),
            ..WidgetConfig::default()
        });
    render_single_widget(frame, area, &widget, ctx);
}
