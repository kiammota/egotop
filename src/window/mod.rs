pub mod compositor;
pub mod context;
pub mod manager;

use crate::config::{Anchor, BorderStyle, SizeSpec};
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

pub use compositor::{ComposedWindow, Compositor};
pub use context::{WindowContext, WindowContextKind};
pub use manager::{DragKind, WindowHit, WindowManager, resolve_window_rect};

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

/// Complete declarative configuration and behavior properties of a terminal window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowConfig {
    pub id: String,
    pub title: Option<String>,
    pub context: String,
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
    pub resizable: bool,
    #[serde(alias = "draggable")]
    pub movable: bool,
    pub minimizable: bool,
    pub maximizable: bool,
    pub fullscreen: bool,
    pub visible: bool,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub layer: i32,
    pub z_index: i32,
    pub overlap: bool,
    pub padding: u16,
    pub border: WindowBorderConfig,
    pub background: WindowBackgroundConfig,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            id: "main".to_string(),
            title: None,
            context: "dashboard".to_string(),
            position: PositionMode::Relative,
            anchor: Anchor::Center,
            x: 0,
            y: 0,
            offset_x: 0,
            offset_y: 0,
            width: SizeSpec::Percent(88),
            height: SizeSpec::Percent(86),
            min_width: 24,
            min_height: 8,
            resizable: true,
            movable: true,
            minimizable: true,
            maximizable: true,
            fullscreen: false,
            visible: true,
            is_static: false,
            layer: 0,
            z_index: 10,
            overlap: true,
            padding: 1,
            border: WindowBorderConfig::default(),
            background: WindowBackgroundConfig::default(),
        }
    }
}

impl WindowConfig {
    pub fn effective_title(&self) -> Option<&str> {
        self.title.as_deref().or(self.border.title.as_deref())
    }

    pub fn can_move(&self) -> bool {
        self.movable && !self.is_static
    }

    pub fn can_resize(&self) -> bool {
        self.resizable
    }
}

/// Snapshot of geometry preserved before maximizing or entering fullscreen.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedGeometry {
    pub position: PositionMode,
    pub anchor: Anchor,
    pub x: u16,
    pub y: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub width: SizeSpec,
    pub height: SizeSpec,
    pub fullscreen: bool,
}

/// Runtime instance of a managed terminal window.
#[derive(Debug, Clone)]
pub struct WindowState {
    pub config: WindowConfig,
    pub resolved_rect: Rect,
    pub minimized: bool,
    pub maximized: bool,
    pub focused: bool,
    pub saved_geometry: Option<SavedGeometry>,
    pub order_seq: u64,
}

impl WindowState {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            resolved_rect: Rect::ZERO,
            minimized: false,
            maximized: false,
            focused: false,
            saved_geometry: None,
            order_seq: 0,
        }
    }

    pub fn save_geometry_if_absent(&mut self) {
        if self.saved_geometry.is_none() {
            self.saved_geometry = Some(SavedGeometry {
                position: self.config.position,
                anchor: self.config.anchor,
                x: self.config.x,
                y: self.config.y,
                offset_x: self.config.offset_x,
                offset_y: self.config.offset_y,
                width: self.config.width,
                height: self.config.height,
                fullscreen: self.config.fullscreen,
            });
        }
    }

    pub fn restore_saved_geometry(&mut self) -> bool {
        let Some(saved) = self.saved_geometry.take() else {
            return false;
        };
        self.config.position = saved.position;
        self.config.anchor = saved.anchor;
        self.config.x = saved.x;
        self.config.y = saved.y;
        self.config.offset_x = saved.offset_x;
        self.config.offset_y = saved.offset_y;
        self.config.width = saved.width;
        self.config.height = saved.height;
        self.config.fullscreen = saved.fullscreen;
        true
    }

    pub fn inner_area(&self) -> Rect {
        inner_area_for_rect(&self.config, self.resolved_rect)
    }

    pub fn hit_test(&self, col: u16, row: u16) -> WindowHit {
        if !self.config.visible
            || self.minimized
            || self.resolved_rect.width == 0
            || self.resolved_rect.height == 0
        {
            return WindowHit::None;
        }
        let r = self.resolved_rect;
        if col < r.x || col >= r.right() || row < r.y || row >= r.bottom() {
            return WindowHit::None;
        }

        if self.config.can_resize()
            && !self.maximized
            && !self.config.fullscreen
            && col >= r.right().saturating_sub(2)
            && row >= r.bottom().saturating_sub(2)
        {
            return WindowHit::ResizeCorner;
        }

        if row == r.y {
            return WindowHit::TitleBar;
        }

        WindowHit::Body
    }
}

pub fn inner_area_for_rect(cfg: &WindowConfig, resolved_rect: Rect) -> Rect {
    let mut area = resolved_rect;
    if cfg.border.enabled && area.width >= 2 && area.height >= 2 {
        area = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width - 2,
            area.height - 2,
        );
    }
    let pad_x = cfg.padding.min(area.width / 4);
    let pad_y = (cfg.padding / 2).min(area.height / 4);
    Rect::new(
        area.x + pad_x,
        area.y + pad_y,
        area.width.saturating_sub(pad_x * 2),
        area.height.saturating_sub(pad_y * 2),
    )
}
