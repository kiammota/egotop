use crate::animation::{AnimationCatalog, BackgroundEffect};
use crate::config::{Config, EffectKind, ThemeEntry};
use crate::runtime::{DiagnosticEngine, DiagnosticSeverity};
use crate::ui::container::{ContainerConfig, ContainerNode};
use crate::ui::theme::Theme;
use crate::ui::widgets::{allocate_three_column_widths, pad_display_width, truncate_display_width};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomizerTab {
    #[default]
    Themes,
    Animations,
    Layout,
}

impl CustomizerTab {
    pub fn all() -> &'static [Self] {
        &[Self::Themes, Self::Animations, Self::Layout]
    }

    pub fn index(self) -> usize {
        match self {
            Self::Themes => 0,
            Self::Animations => 1,
            Self::Layout => 2,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Themes => "Themes",
            Self::Animations => "Animations",
            Self::Layout => "Layout",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Themes => Self::Animations,
            Self::Animations => Self::Layout,
            Self::Layout => Self::Themes,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Themes => Self::Layout,
            Self::Animations => Self::Themes,
            Self::Layout => Self::Animations,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomizerState {
    pub open: bool,
    pub tab: CustomizerTab,
    pub selected: usize,
    pub modal_rect: Rect,
    pub content_rect: Rect,
    pub tab_rects: Vec<Rect>,
}

impl Default for CustomizerState {
    fn default() -> Self {
        Self {
            open: false,
            tab: CustomizerTab::Themes,
            selected: 0,
            modal_rect: Rect::ZERO,
            content_rect: Rect::ZERO,
            tab_rects: Vec::new(),
        }
    }
}

impl CustomizerState {
    pub fn open_tab(&mut self, tab: CustomizerTab) {
        self.open = true;
        self.tab = tab;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn move_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            len - 1
        } else {
            self.selected - 1
        };
    }

    pub fn move_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.selected = (self.selected + 1) % len;
    }
}

pub fn collect_component_ids(container: &ContainerConfig, out: &mut Vec<(String, bool)>) {
    for child in &container.children {
        match child {
            ContainerNode::Container(sub) => collect_component_ids(sub, out),
            ContainerNode::Component(comp) => out.push((comp.id.clone(), comp.visible)),
        }
    }
}

pub fn toggle_component_visible(container: &mut ContainerConfig, id: &str) -> Option<bool> {
    for child in &mut container.children {
        match child {
            ContainerNode::Component(comp) if comp.id == id => {
                comp.visible = !comp.visible;
                return Some(comp.visible);
            }
            ContainerNode::Container(sub) => {
                if let Some(res) = toggle_component_visible(sub, id) {
                    return Some(res);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn layout_items_summary(cfg: &Config) -> Vec<(String, String)> {
    let mut items = vec![
        (
            "View Mode".to_string(),
            cfg.general.view_mode.label().to_string(),
        ),
        (
            "Background Effect".to_string(),
            cfg.background.effect.label().to_string(),
        ),
    ];
    let mut comps = Vec::new();
    collect_component_ids(&cfg.ui.root, &mut comps);
    for (id, visible) in comps {
        items.push((
            format!("Component: {id}"),
            if visible { "Visible" } else { "Hidden" }.to_string(),
        ));
    }
    items.push(("Save egotop.toml".to_string(), "Persist to ~/.config/egotop/".to_string()));
    items
}

#[allow(clippy::too_many_arguments)]
pub fn render_customizer(
    frame: &mut Frame,
    screen: Rect,
    state: &mut CustomizerState,
    config: &Config,
    themes: &[ThemeEntry],
    animations: &AnimationCatalog,
    bg_effect: &dyn BackgroundEffect,
    theme: &Theme,
) {
    let w = 74.min(screen.width.saturating_sub(4));
    let h = 24.min(screen.height.saturating_sub(4));
    if w < 30 || h < 12 {
        return;
    }

    let modal = Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + (screen.height - h) / 2,
        w,
        h,
    );
    state.modal_rect = modal;
    frame.render_widget(Clear, modal);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.borders.as_ratatui_set())
        .border_style(Style::default().fg(theme.primary).bg(theme.panel_background))
        .style(theme.panel_style())
        .title(Span::styled(
            " Customize — egotop ",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(inner);

    let titles: Vec<Line> = CustomizerTab::all()
        .iter()
        .map(|t| Line::from(format!(" {} ", t.title())))
        .collect();
    let tabs = Tabs::new(titles)
        .select(state.tab.index())
        .style(Style::default().fg(theme.muted))
        .highlight_style(
            Style::default()
                .fg(theme.background)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");
    frame.render_widget(tabs, chunks[0]);
    state.content_rect = chunks[1];

    // Save approximate tab clickable zones
    state.tab_rects.clear();
    let mut tx = chunks[0].x;
    for t in CustomizerTab::all() {
        let tw = (t.title().chars().count() as u16) + 3;
        state.tab_rects.push(Rect::new(tx, chunks[0].y, tw, 1));
        tx = tx.saturating_add(tw + 1);
    }

    // `render_selectable_list` reserves 2 columns on the left for `"▶ "` / `"  "`.
    let row_budget = (chunks[1].width as usize).saturating_sub(2);
    let content_budget = row_budget.saturating_sub(2);

    match state.tab {
        CustomizerTab::Themes => {
            let max_name_w = themes
                .iter()
                .map(|e| e.name.width())
                .max()
                .unwrap_or(10)
                .clamp(8, 20);
            let (name_w, _, origin_w) =
                allocate_three_column_widths(content_budget, max_name_w + 1, 8, 0);
            let items: Vec<ListItem> = themes
                .iter()
                .map(|entry| {
                    let active = entry.name.eq_ignore_ascii_case(&theme.name);
                    let origin = if entry.source_path.is_some() {
                        "~/.config/egotop/themes"
                    } else {
                        "built-in"
                    };
                    let mark = if active { "● " } else { "  " };
                    let mut spans = vec![Span::styled(mark, Style::default().fg(theme.primary))];
                    if origin_w > 0 {
                        let name_cell =
                            format!("{} ", pad_display_width(&entry.name, name_w.saturating_sub(1)));
                        let origin_cell = truncate_display_width(&format!("({origin})"), origin_w);
                        spans.push(Span::styled(
                            name_cell,
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(origin_cell, Style::default().fg(theme.muted)));
                    } else {
                        spans.push(Span::styled(
                            pad_display_width(&entry.name, name_w),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
        CustomizerTab::Animations => {
            let active_startup = &config.general.startup_animation;
            let max_name_w = animations
                .entries
                .iter()
                .map(|e| e.display_name.width())
                .max()
                .unwrap_or(14);
            let pref_name_w = (max_name_w + 1).clamp(12, 24);
            let (name_w, tag_w, desc_w) =
                allocate_three_column_widths(content_budget, pref_name_w, 10, 9);
            let items: Vec<ListItem> = animations
                .entries
                .iter()
                .map(|entry| {
                    let is_startup = entry.id.eq_ignore_ascii_case(active_startup);
                    let mark = if is_startup { "★ " } else { "  " };
                    let tag = if entry.is_community { "[TOML]" } else { "[NATIVE]" };

                    let mut spans = vec![Span::styled(mark, Style::default().fg(theme.accent))];
                    let name_cell = if tag_w > 0 || desc_w > 0 {
                        format!(
                            "{} ",
                            pad_display_width(&entry.display_name, name_w.saturating_sub(1))
                        )
                    } else {
                        pad_display_width(&entry.display_name, name_w)
                    };
                    spans.push(Span::styled(
                        name_cell,
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ));
                    if tag_w > 0 {
                        let tag_cell = if desc_w > 0 {
                            format!("{} ", pad_display_width(tag, tag_w.saturating_sub(1)))
                        } else {
                            pad_display_width(tag, tag_w)
                        };
                        spans.push(Span::styled(tag_cell, Style::default().fg(theme.secondary)));
                    }
                    if desc_w > 0 {
                        spans.push(Span::styled(
                            truncate_display_width(&entry.description, desc_w),
                            Style::default().fg(theme.muted),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
        CustomizerTab::Layout => {
            let mut cfg_view = config.clone();
            cfg_view.background.effect = bg_effect.kind();
            let rows = layout_items_summary(&cfg_view);
            let max_key_w = rows.iter().map(|(k, _)| k.width()).max().unwrap_or(18).clamp(12, 28);
            let (key_w, _, val_w) =
                allocate_three_column_widths(content_budget, max_key_w + 1, 10, 0);
            let items: Vec<ListItem> = rows
                .into_iter()
                .map(|(k, v)| {
                    let mut spans = vec![Span::raw("  ")];
                    if val_w > 0 {
                        spans.push(Span::styled(
                            format!("{} ", pad_display_width(&k, key_w.saturating_sub(1))),
                            Style::default()
                                .fg(theme.secondary)
                                .add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(
                            truncate_display_width(&v, val_w),
                            Style::default().fg(theme.text),
                        ));
                    } else {
                        spans.push(Span::styled(
                            pad_display_width(&k, key_w),
                            Style::default()
                                .fg(theme.secondary)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
    }

    let hint = match state.tab {
        CustomizerTab::Themes => "↑/↓ navigate • Enter apply • s save • Tab switch tab • Esc close",
        CustomizerTab::Animations => "↑/↓ navigate • p preview • Enter set startup • Tab switch tab • Esc close",
        CustomizerTab::Layout => "↑/↓ navigate • Enter/Space toggle • s save egotop.toml • Esc close",
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(theme.muted))))
            .wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    items: Vec<ListItem>,
    selected: usize,
    theme: &Theme,
) {
    let mut list_state = ListState::default();
    if !items.is_empty() {
        list_state.select(Some(selected.min(items.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(theme.background)
                .bg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

pub fn render_help_overlay(frame: &mut Frame, screen: Rect, theme: &Theme) {
    let w = 72.min(screen.width.saturating_sub(4));
    let h = 23.min(screen.height.saturating_sub(4));
    if w < 28 || h < 12 {
        return;
    }
    let modal = Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + (screen.height - h) / 2,
        w,
        h,
    );
    frame.render_widget(Clear, modal);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.borders.as_ratatui_set())
        .border_style(Style::default().fg(theme.primary).bg(theme.panel_background))
        .style(theme.panel_style())
        .title(Span::styled(
            " Help — egotop ",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let kv = |k: &str, v: &str| {
        Line::from(vec![
            Span::styled(
                format!("{k:<22} "),
                Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(v.to_string(), Style::default().fg(theme.text)),
        ])
    };

    let lines = vec![
        kv(":", "Open command line (Neovim-style)"),
        kv(":colorscheme <name>", "Switch colorscheme (built-in or ~/.config/egotop/themes/)"),
        kv(":set border.<prop> <v>", "Customize border chars/style live in runtime"),
        kv(":animation [name]", "Open picker or run startup animation"),
        kv(":layout [mode]", "Switch view mode (dashboard, compact, minimal) or preset"),
        kv(":write [target] / :w", "Write runtime changes to ~/.config/egotop/"),
        kv(":reload", "Reload configs from ~/.config/egotop/"),
        kv(":quit / :q", "Quit and restore terminal"),
        Line::from(""),
        kv("F2 / g", "Open visual customization panel"),
        kv("r / p / v", "Refresh data / Pause animations / Cycle view mode"),
        kv("q / Esc", "Close modal or quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Configuration directory: ~/.config/egotop/",
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

pub fn effect_names() -> Vec<String> {
    [
        EffectKind::Matrix,
        EffectKind::Water,
        EffectKind::Particles,
        EffectKind::Scanlines,
        EffectKind::None,
    ]
    .iter()
    .map(|e| e.label().to_ascii_lowercase())
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticsViewMode {
    #[default]
    Errors,
    Messages,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsOverlayState {
    pub open: bool,
    pub mode: DiagnosticsViewMode,
    pub scroll: usize,
}

impl DiagnosticsOverlayState {
    pub fn open_mode(&mut self, mode: DiagnosticsViewMode) {
        self.open = true;
        self.mode = mode;
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            DiagnosticsViewMode::Errors => DiagnosticsViewMode::Messages,
            DiagnosticsViewMode::Messages => DiagnosticsViewMode::Errors,
        };
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }
}

/// Renders active configuration errors (`:errors`) or chronological diagnostic/log
/// history (`:messages`) inside a themed, bordered modal overlay.
pub fn render_diagnostics_overlay(
    frame: &mut Frame,
    screen: Rect,
    state: &mut DiagnosticsOverlayState,
    engine: &DiagnosticEngine,
    theme: &Theme,
) {
    let w = 84.min(screen.width.saturating_sub(4));
    let h = 24.min(screen.height.saturating_sub(4));
    if w < 32 || h < 10 {
        return;
    }
    let modal = Rect::new(
        screen.x + (screen.width - w) / 2,
        screen.y + (screen.height - h) / 2,
        w,
        h,
    );
    frame.render_widget(Clear, modal);

    let title = match state.mode {
        DiagnosticsViewMode::Errors => {
            format!(" Configuration Errors ({}) — egotop ", engine.error_count())
        }
        DiagnosticsViewMode::Messages => {
            format!(
                " Configuration Messages & Logs ({}) — egotop ",
                engine.history().len() + engine.logs().len()
            )
        }
    };
    let border_color = if state.mode == DiagnosticsViewMode::Errors && engine.error_count() > 0 {
        theme.danger
    } else {
        theme.primary
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.borders.as_ratatui_set())
        .border_style(Style::default().fg(border_color).bg(theme.panel_background))
        .style(theme.panel_style())
        .title(Span::styled(
            truncate_display_width(&title, w.saturating_sub(4) as usize),
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let max_line_w = chunks[0].width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();

    match state.mode {
        DiagnosticsViewMode::Errors => {
            let items = engine.active_warnings_and_errors();
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No active configuration errors. Previous or current configuration is valid.",
                    Style::default().fg(theme.primary),
                )));
            } else {
                for diag in items {
                    let sev_color = match diag.severity {
                        DiagnosticSeverity::Error => theme.danger,
                        DiagnosticSeverity::Warning => theme.accent,
                        DiagnosticSeverity::Info => theme.secondary,
                    };
                    for (idx, text) in diag.format_lines().into_iter().enumerate() {
                        let style = if idx == 0 {
                            Style::default().fg(sev_color).add_modifier(Modifier::BOLD)
                        } else if text.starts_with("Expected:") || text.starts_with("Hint:") {
                            Style::default().fg(theme.secondary)
                        } else {
                            Style::default().fg(theme.text)
                        };
                        lines.push(Line::from(Span::styled(
                            truncate_display_width(&text, max_line_w),
                            style,
                        )));
                    }
                    lines.push(Line::from(""));
                }
            }
        }
        DiagnosticsViewMode::Messages => {
            if engine.history().is_empty() && engine.logs().is_empty() {
                lines.push(Line::from(Span::styled(
                    "No configuration messages recorded yet.",
                    Style::default().fg(theme.muted),
                )));
            } else {
                for entry in engine.logs().iter().rev().take(60) {
                    let color = match entry.severity {
                        DiagnosticSeverity::Error => theme.danger,
                        DiagnosticSeverity::Warning => theme.accent,
                        DiagnosticSeverity::Info => theme.secondary,
                    };
                    let formatted =
                        format!("{} [{}] {}", entry.timestamp, entry.category, entry.message);
                    lines.push(Line::from(Span::styled(
                        truncate_display_width(&formatted, max_line_w),
                        Style::default().fg(color),
                    )));
                }
                if !engine.history().is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "--- Diagnostic History ---",
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    )));
                    for diag in engine.history().iter().rev().take(40) {
                        let sev_color = match diag.severity {
                            DiagnosticSeverity::Error => theme.danger,
                            DiagnosticSeverity::Warning => theme.accent,
                            DiagnosticSeverity::Info => theme.secondary,
                        };
                        let loc = diag
                            .file
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .unwrap_or("config");
                        let summary = format!(
                            "[{}] {} {}: {}",
                            diag.timestamp, diag.code, loc, diag.message
                        );
                        lines.push(Line::from(Span::styled(
                            truncate_display_width(&summary, max_line_w),
                            Style::default().fg(sev_color),
                        )));
                    }
                }
            }
        }
    }

    let visible_h = chunks[0].height as usize;
    let max_scroll = lines.len().saturating_sub(visible_h);
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }
    let body = Paragraph::new(lines)
        .scroll((state.scroll as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let hint = "↑/↓ scroll • Tab switch Errors/Messages • Esc close";
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate_display_width(hint, max_line_w),
            Style::default().fg(theme.muted),
        )),
        chunks[1],
    );
}
