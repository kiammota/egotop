use crate::config::{Anchor, BorderStyle, SizeSpec};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PositionMode {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct WindowBorderConfig {
    pub enabled: bool,
    pub style: BorderStyle,
    pub color: Option<String>,
    pub title: Option<String>,
}

impl Default for WindowBorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            style: BorderStyle::Rounded,
            color: None,
            title: Some("egotop".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowBackgroundMode {
    #[default]
    Solid,
    /// Uses terminal cell `Color::Reset` so the terminal emulator's own
    /// background/transparency shows through (distinct from OS compositor transparency).
    Transparent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct WindowBackgroundConfig {
    pub mode: WindowBackgroundMode,
    pub color: Option<String>,
}

impl Default for WindowBackgroundConfig {
    fn default() -> Self {
        Self {
            mode: WindowBackgroundMode::Solid,
            color: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowConfig {
    pub id: String,
    pub position: PositionMode,
    pub anchor: Anchor,
    pub x: u16,
    pub y: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub width: SizeSpec,
    pub height: SizeSpec,
    pub min_width: u16,
    pub min_height: u16,
    pub draggable: bool,
    pub resizable: bool,
    pub padding: u16,
    pub visible: bool,
    pub z_index: i32,
    pub border: WindowBorderConfig,
    pub background: WindowBackgroundConfig,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            id: "main".to_string(),
            position: PositionMode::Relative,
            anchor: Anchor::Center,
            x: 0,
            y: 0,
            offset_x: 0,
            offset_y: 0,
            width: SizeSpec::Percent(88),
            height: SizeSpec::Percent(86),
            min_width: 36,
            min_height: 12,
            draggable: true,
            resizable: true,
            padding: 1,
            visible: true,
            z_index: 10,
            border: WindowBorderConfig::default(),
            background: WindowBackgroundConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Move {
        start_mouse_x: u16,
        start_mouse_y: u16,
        orig_x: u16,
        orig_y: u16,
    },
    ResizeBottomRight {
        start_mouse_x: u16,
        start_mouse_y: u16,
        orig_w: u16,
        orig_h: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHit {
    None,
    TitleBar,
    ResizeCorner,
    Body,
}

#[derive(Debug, Clone)]
pub struct WindowState {
    pub config: WindowConfig,
    pub resolved_rect: Rect,
}

impl WindowState {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            resolved_rect: Rect::ZERO,
        }
    }

    pub fn inner_area(&self) -> Rect {
        let mut area = self.resolved_rect;
        if self.config.border.enabled && area.width >= 2 && area.height >= 2 {
            area = Rect::new(
                area.x + 1,
                area.y + 1,
                area.width - 2,
                area.height - 2,
            );
        }
        let pad_x = self.config.padding.min(area.width / 4);
        let pad_y = (self.config.padding / 2).min(area.height / 4);
        Rect::new(
            area.x + pad_x,
            area.y + pad_y,
            area.width.saturating_sub(pad_x * 2),
            area.height.saturating_sub(pad_y * 2),
        )
    }

    pub fn hit_test(&self, col: u16, row: u16) -> WindowHit {
        if !self.config.visible || self.resolved_rect.width == 0 || self.resolved_rect.height == 0 {
            return WindowHit::None;
        }
        let r = self.resolved_rect;
        if col < r.x || col >= r.right() || row < r.y || row >= r.bottom() {
            return WindowHit::None;
        }

        // Bottom-right 2x2 corner for resizing
        if self.config.resizable
            && col >= r.right().saturating_sub(2)
            && row >= r.bottom().saturating_sub(2)
        {
            return WindowHit::ResizeCorner;
        }

        // Top border row acts as draggable title bar
        if row == r.y {
            return WindowHit::TitleBar;
        }

        WindowHit::Body
    }
}

#[derive(Debug, Clone)]
pub struct WindowManager {
    windows: Vec<WindowState>,
    active_drag: Option<(String, DragKind)>,
}

impl WindowManager {
    pub fn new(main_config: WindowConfig) -> Self {
        Self {
            windows: vec![WindowState::new(main_config)],
            active_drag: None,
        }
    }

    pub fn main_window(&self) -> &WindowState {
        &self.windows[0]
    }

    pub fn main_window_mut(&mut self) -> &mut WindowState {
        &mut self.windows[0]
    }

    pub fn windows_sorted(&self) -> Vec<&WindowState> {
        let mut refs: Vec<&WindowState> = self.windows.iter().collect();
        refs.sort_by_key(|w| w.config.z_index);
        refs
    }

    pub fn resolve_all(&mut self, viewport: Rect) {
        for win in &mut self.windows {
            win.resolved_rect = resolve_window_rect(&win.config, viewport);
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn begin_mouse_down(&mut self, col: u16, row: u16, viewport: Rect) -> bool {
        self.resolve_all(viewport);
        // Check top-most visible window first (highest z_index)
        let mut order: Vec<usize> = (0..self.windows.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.windows[i].config.z_index));

        for idx in order {
            let hit = self.windows[idx].hit_test(col, row);
            match hit {
                WindowHit::TitleBar if self.windows[idx].config.draggable => {
                    let rect = self.windows[idx].resolved_rect;
                    self.windows[idx].config.position = PositionMode::Absolute;
                    self.windows[idx].config.x = rect.x;
                    self.windows[idx].config.y = rect.y;
                    self.windows[idx].config.offset_x = 0;
                    self.windows[idx].config.offset_y = 0;
                    let id = self.windows[idx].config.id.clone();
                    self.active_drag = Some((
                        id,
                        DragKind::Move {
                            start_mouse_x: col,
                            start_mouse_y: row,
                            orig_x: rect.x,
                            orig_y: rect.y,
                        },
                    ));
                    return true;
                }
                WindowHit::ResizeCorner if self.windows[idx].config.resizable => {
                    let rect = self.windows[idx].resolved_rect;
                    let id = self.windows[idx].config.id.clone();
                    self.active_drag = Some((
                        id,
                        DragKind::ResizeBottomRight {
                            start_mouse_x: col,
                            start_mouse_y: row,
                            orig_w: rect.width,
                            orig_h: rect.height,
                        },
                    ));
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub fn update_mouse_drag(&mut self, col: u16, row: u16, viewport: Rect) -> bool {
        let Some((ref id, drag)) = self.active_drag.clone() else {
            return false;
        };
        let Some(win) = self.windows.iter_mut().find(|w| &w.config.id == id) else {
            self.active_drag = None;
            return false;
        };

        match drag {
            DragKind::Move {
                start_mouse_x,
                start_mouse_y,
                orig_x,
                orig_y,
            } => {
                let dx = col as i32 - start_mouse_x as i32;
                let dy = row as i32 - start_mouse_y as i32;
                let max_x = viewport
                    .right()
                    .saturating_sub(win.resolved_rect.width) as i32;
                let max_y = viewport
                    .bottom()
                    .saturating_sub(win.resolved_rect.height) as i32;
                win.config.position = PositionMode::Absolute;
                win.config.x = (orig_x as i32 + dx).clamp(viewport.x as i32, max_x.max(viewport.x as i32)) as u16;
                win.config.y = (orig_y as i32 + dy).clamp(viewport.y as i32, max_y.max(viewport.y as i32)) as u16;
            }
            DragKind::ResizeBottomRight {
                start_mouse_x,
                start_mouse_y,
                orig_w,
                orig_h,
            } => {
                let dw = col as i32 - start_mouse_x as i32;
                let dh = row as i32 - start_mouse_y as i32;
                let max_w = viewport.right().saturating_sub(win.resolved_rect.x);
                let max_h = viewport.bottom().saturating_sub(win.resolved_rect.y);
                let min_w = win.config.min_width.min(max_w).max(12);
                let min_h = win.config.min_height.min(max_h).max(6);
                let new_w = (orig_w as i32 + dw).clamp(min_w as i32, max_w.max(min_w) as i32) as u16;
                let new_h = (orig_h as i32 + dh).clamp(min_h as i32, max_h.max(min_h) as i32) as u16;
                win.config.width = SizeSpec::Cells(new_w);
                win.config.height = SizeSpec::Cells(new_h);
            }
        }

        win.resolved_rect = resolve_window_rect(&win.config, viewport);
        true
    }

    pub fn end_mouse_drag(&mut self) {
        self.active_drag = None;
    }

    pub fn nudge_main(&mut self, dx: i16, dy: i16, viewport: Rect) {
        let win = &mut self.windows[0];
        if !win.config.draggable {
            return;
        }
        if win.config.position == PositionMode::Absolute {
            let max_x = viewport.right().saturating_sub(win.resolved_rect.width) as i32;
            let max_y = viewport.bottom().saturating_sub(win.resolved_rect.height) as i32;
            win.config.x = (win.config.x as i32 + dx as i32)
                .clamp(viewport.x as i32, max_x.max(viewport.x as i32)) as u16;
            win.config.y = (win.config.y as i32 + dy as i32)
                .clamp(viewport.y as i32, max_y.max(viewport.y as i32)) as u16;
        } else {
            win.config.offset_x = win.config.offset_x.saturating_add(dx);
            win.config.offset_y = win.config.offset_y.saturating_add(dy);
        }
        win.resolved_rect = resolve_window_rect(&win.config, viewport);
    }

    pub fn resize_main_by(&mut self, dw: i16, dh: i16, viewport: Rect) {
        let win = &mut self.windows[0];
        if !win.config.resizable {
            return;
        }
        let cur = resolve_window_rect(&win.config, viewport);
        let new_w = (cur.width as i32 + dw as i32)
            .clamp(win.config.min_width.min(viewport.width) as i32, viewport.width as i32) as u16;
        let new_h = (cur.height as i32 + dh as i32)
            .clamp(win.config.min_height.min(viewport.height) as i32, viewport.height as i32) as u16;
        win.config.width = SizeSpec::Cells(new_w);
        win.config.height = SizeSpec::Cells(new_h);
        win.resolved_rect = resolve_window_rect(&win.config, viewport);
    }
}

pub fn resolve_window_rect(cfg: &WindowConfig, viewport: Rect) -> Rect {
    if !cfg.visible || viewport.width == 0 || viewport.height == 0 {
        return Rect::ZERO;
    }

    let raw_w = match cfg.width {
        SizeSpec::Cells(c) => c,
        SizeSpec::Percent(p) => ((viewport.width as u32 * p.min(100) as u32) / 100) as u16,
    };
    let raw_h = match cfg.height {
        SizeSpec::Cells(c) => c,
        SizeSpec::Percent(p) => ((viewport.height as u32 * p.min(100) as u32) / 100) as u16,
    };

    let w = raw_w
        .max(cfg.min_width.min(viewport.width))
        .min(viewport.width);
    let h = raw_h
        .max(cfg.min_height.min(viewport.height))
        .min(viewport.height);

    let (base_x, base_y) = match cfg.position {
        PositionMode::Absolute => (viewport.x.saturating_add(cfg.x), viewport.y.saturating_add(cfg.y)),
        PositionMode::Relative => match cfg.anchor {
            Anchor::TopLeft => (viewport.x, viewport.y),
            Anchor::TopCenter => (viewport.x + viewport.width.saturating_sub(w) / 2, viewport.y),
            Anchor::TopRight => (viewport.right().saturating_sub(w), viewport.y),
            Anchor::CenterLeft => (viewport.x, viewport.y + viewport.height.saturating_sub(h) / 2),
            Anchor::Center | Anchor::Absolute => (
                viewport.x + viewport.width.saturating_sub(w) / 2,
                viewport.y + viewport.height.saturating_sub(h) / 2,
            ),
            Anchor::CenterRight => (
                viewport.right().saturating_sub(w),
                viewport.y + viewport.height.saturating_sub(h) / 2,
            ),
            Anchor::BottomLeft => (viewport.x, viewport.bottom().saturating_sub(h)),
            Anchor::BottomCenter => (
                viewport.x + viewport.width.saturating_sub(w) / 2,
                viewport.bottom().saturating_sub(h),
            ),
            Anchor::BottomRight => (
                viewport.right().saturating_sub(w),
                viewport.bottom().saturating_sub(h),
            ),
        },
    };

    let max_x = viewport.right().saturating_sub(w) as i32;
    let max_y = viewport.bottom().saturating_sub(h) as i32;
    let clamped_x = (base_x as i32 + cfg.offset_x as i32)
        .clamp(viewport.x as i32, max_x.max(viewport.x as i32)) as u16;
    let clamped_y = (base_y as i32 + cfg.offset_y as i32)
        .clamp(viewport.y as i32, max_y.max(viewport.y as i32)) as u16;

    Rect::new(clamped_x, clamped_y, w, h)
}
