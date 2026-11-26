use crate::animation::{AnimationCatalog, BackgroundEffect};
use crate::config::{Anchor, BorderStyle, Config, EffectKind, LayoutMode, ThemeEntry, WidgetId};
use crate::ui::theme::Theme;
use crate::window::{WindowBackgroundMode, WindowConfig};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomizerTab {
    #[default]
    Themes,
    Animations,
    Window,
    Layout,
}

impl CustomizerTab {
    pub fn all() -> &'static [Self] {
        &[Self::Themes, Self::Animations, Self::Window, Self::Layout]
    }

    pub fn index(self) -> usize {
        match self {
            Self::Themes => 0,
            Self::Animations => 1,
            Self::Window => 2,
            Self::Layout => 3,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Themes => "Themes",
            Self::Animations => "Animations",
            Self::Window => "Window",
            Self::Layout => "Layout",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Themes => Self::Animations,
            Self::Animations => Self::Window,
            Self::Window => Self::Layout,
            Self::Layout => Self::Themes,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Themes => Self::Layout,
            Self::Animations => Self::Themes,
            Self::Window => Self::Animations,
            Self::Layout => Self::Window,
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

pub fn window_items_summary(win: &WindowConfig) -> Vec<(String, String)> {
    vec![
        ("Anchor".to_string(), format!("{:?}", win.anchor)),
        (
            "Borders".to_string(),
            if win.border.enabled {
                format!("Enabled ({})", win.border.style.label())
            } else {
                "Hidden".to_string()
            },
        ),
        (
            "Window Background".to_string(),
            match win.background.mode {
                WindowBackgroundMode::Solid => "Solid".to_string(),
                WindowBackgroundMode::Transparent => "Transparent (cells)".to_string(),
            },
        ),
        ("Inner Padding".to_string(), format!("{}", win.padding)),
        (
            "Draggable".to_string(),
            if win.draggable { "Yes" } else { "No" }.to_string(),
        ),
        (
            "Resizable".to_string(),
            if win.resizable { "Yes" } else { "No" }.to_string(),
        ),
        ("Center & Reset".to_string(), "Restore default".to_string()),
        ("Save window.toml".to_string(), "Persist to ~/.config/egotop/".to_string()),
    ]
}

pub fn layout_items_summary(cfg: &Config) -> Vec<(String, String)> {
    let mut items = vec![
        ("Layout Mode".to_string(), format!("{:?}", cfg.layout.mode)),
        (
            "View Mode".to_string(),
            cfg.general.view_mode.label().to_string(),
        ),
        (
            "Background Effect".to_string(),
            cfg.background.effect.label().to_string(),
        ),
    ];
    for w in &cfg.widgets {
        items.push((
            format!("Component: {}", w.id.default_title()),
            if w.enabled { "Visible" } else { "Hidden" }.to_string(),
        ));
    }
    items.push(("Save egotop.toml".to_string(), "Persist to ~/.config/egotop/".to_string()));
    items
}

pub fn cycle_anchor(anchor: Anchor) -> Anchor {
    match anchor {
        Anchor::TopLeft => Anchor::TopCenter,
        Anchor::TopCenter => Anchor::TopRight,
        Anchor::TopRight => Anchor::CenterLeft,
        Anchor::CenterLeft => Anchor::Center,
        Anchor::Center => Anchor::CenterRight,
        Anchor::CenterRight => Anchor::BottomLeft,
        Anchor::BottomLeft => Anchor::BottomCenter,
        Anchor::BottomCenter => Anchor::BottomRight,
        Anchor::BottomRight | Anchor::Absolute => Anchor::TopLeft,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_customizer(
    frame: &mut Frame,
    screen: Rect,
    state: &mut CustomizerState,
    config: &Config,
    window_cfg: &WindowConfig,
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

    match state.tab {
        CustomizerTab::Themes => {
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
                    ListItem::new(Line::from(vec![
                        Span::styled(mark, Style::default().fg(theme.primary)),
                        Span::styled(
                            format!("{:<18}", entry.name),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!(" ({origin})"), Style::default().fg(theme.muted)),
                    ]))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
        CustomizerTab::Animations => {
            let active_startup = &config.general.startup_animation;
            let items: Vec<ListItem> = animations
                .entries
                .iter()
                .map(|entry| {
                    let is_startup = entry.id.eq_ignore_ascii_case(active_startup);
                    let mark = if is_startup { "★ " } else { "  " };
                    let tag = if entry.is_community { "TOML" } else { "NATIVA" };
                    ListItem::new(Line::from(vec![
                        Span::styled(mark, Style::default().fg(theme.accent)),
                        Span::styled(
                            format!("{:<24}", entry.display_name),
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("[{tag}] "), Style::default().fg(theme.secondary)),
                        Span::styled(&entry.description, Style::default().fg(theme.muted)),
                    ]))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
        CustomizerTab::Window => {
            let rows = window_items_summary(window_cfg);
            let items: Vec<ListItem> = rows
                .into_iter()
                .map(|(k, v)| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("  {:<24}", k),
                            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(v, Style::default().fg(theme.text)),
                    ]))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
        CustomizerTab::Layout => {
            let mut cfg_view = config.clone();
            cfg_view.background.effect = bg_effect.kind();
            let rows = layout_items_summary(&cfg_view);
            let items: Vec<ListItem> = rows
                .into_iter()
                .map(|(k, v)| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("  {:<28}", k),
                            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(v, Style::default().fg(theme.text)),
                    ]))
                })
                .collect();
            render_selectable_list(frame, chunks[1], items, state.selected, theme);
        }
    }

    let hint = match state.tab {
        CustomizerTab::Themes => "↑/↓ navigate • Enter apply • s save • Tab switch tab • Esc close",
        CustomizerTab::Animations => "↑/↓ navigate • p preview • Enter set startup • Tab switch tab • Esc close",
        CustomizerTab::Window => "↑/↓ navigate • Enter/Space toggle • s save window.toml • Esc close",
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
        kv(":window [subcmd]", "Configure window (center, anchor, border, bg...)"),
        kv(":layout [mode]", "Switch layout (responsive, free, compact...)"),
        kv(":write [target] / :w", "Write runtime changes to ~/.config/egotop/"),
        kv(":reload", "Reload configs from ~/.config/egotop/"),
        kv(":quit / :q", "Quit and restore terminal"),
        Line::from(""),
        kv("Mouse (top border)", "Drag to move window"),
        kv("Mouse (bottom-right)", "Drag to resize window"),
        kv("Click [⚙] / F2 / g", "Open visual customization panel"),
        kv("r / p / 1-3", "Refresh data / Pause animations / Pages"),
        kv("q / Esc", "Close modal or quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Configuration directory: ~/.config/egotop/",
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

pub fn toggle_widget_enabled(cfg: &mut Config, id: WidgetId) -> bool {
    if let Some(w) = cfg.widgets.iter_mut().find(|w| w.id == id) {
        w.enabled = !w.enabled;
        return w.enabled;
    }
    false
}

pub fn cycle_layout_mode(mode: LayoutMode) -> LayoutMode {
    match mode {
        LayoutMode::Responsive => LayoutMode::Free,
        LayoutMode::Free => LayoutMode::Compact,
        LayoutMode::Compact => LayoutMode::Responsive,
    }
}

pub fn cycle_border_style(win: &mut WindowConfig) {
    if !win.border.enabled {
        win.border.enabled = true;
        win.border.style = BorderStyle::Rounded;
    } else if win.border.style == BorderStyle::Thick {
        win.border.enabled = false;
    } else {
        win.border.style = win.border.style.next();
    }
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
