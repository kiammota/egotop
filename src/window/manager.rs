use crate::config::{Anchor, SizeSpec};
use crate::window::{PositionMode, WindowConfig, WindowContext, WindowState};
use ratatui::layout::Rect;
use std::collections::BTreeMap;

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

/// Manages window instances, lifecycle, focus, geometry constraints, conflicts,
/// and Window <-> Context associations.
#[derive(Debug, Clone)]
pub struct WindowManager {
    windows: Vec<WindowState>,
    contexts: BTreeMap<String, WindowContext>,
    focused_id: Option<String>,
    active_drag: Option<(String, DragKind)>,
    next_seq: u64,
}

impl WindowManager {
    pub fn new(main_config: WindowConfig) -> Self {
        let mut mgr = Self {
            windows: Vec::new(),
            contexts: BTreeMap::new(),
            focused_id: None,
            active_drag: None,
            next_seq: 1,
        };
        let _ = mgr.register_window(main_config);
        mgr
    }

    pub fn from_configs(configs: Vec<WindowConfig>) -> Self {
        if configs.is_empty() {
            return Self::new(WindowConfig::default());
        }
        let mut mgr = Self {
            windows: Vec::new(),
            contexts: BTreeMap::new(),
            focused_id: None,
            active_drag: None,
            next_seq: 1,
        };
        for cfg in configs {
            let _ = mgr.register_window(cfg);
        }
        mgr
    }

    pub fn register_window(&mut self, cfg: WindowConfig) -> Result<(), String> {
        let id = cfg.id.trim();
        if id.is_empty() {
            return Err("Window id cannot be empty".to_string());
        }
        if self.windows.iter().any(|w| w.config.id == id) {
            return Err(format!("Window '{id}' is already registered"));
        }

        let ctx_id = if cfg.context.trim().is_empty() {
            id.to_string()
        } else {
            cfg.context.trim().to_string()
        };
        self.contexts
            .entry(ctx_id.clone())
            .or_insert_with(|| WindowContext::from_id(&ctx_id));

        let mut normalized = cfg;
        normalized.context = ctx_id;
        let mut state = WindowState::new(normalized);
        state.order_seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);

        if self.focused_id.is_none() && state.config.visible {
            state.focused = true;
            self.focused_id = Some(state.config.id.clone());
        }
        self.windows.push(state);
        Ok(())
    }

    pub fn remove_window(&mut self, id: &str) -> bool {
        let Some(pos) = self.windows.iter().position(|w| w.config.id == id) else {
            return false;
        };
        self.windows.remove(pos);
        if self.focused_id.as_deref() == Some(id) {
            self.focused_id = self
                .windows
                .iter()
                .find(|w| w.config.visible && !w.minimized)
                .map(|w| w.config.id.clone());
            self.sync_focus_flags();
        }
        true
    }

    pub fn replace_windows(&mut self, configs: Vec<WindowConfig>) {
        if configs.is_empty() {
            return;
        }
        self.windows.clear();
        self.focused_id = None;
        self.active_drag = None;
        self.next_seq = 1;
        for cfg in configs {
            let _ = self.register_window(cfg);
        }
    }

    pub fn associate_context(&mut self, window_id: &str, context_id: &str) -> bool {
        let trimmed = context_id.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(win) = self.windows.iter_mut().find(|w| w.config.id == window_id) else {
            return false;
        };
        self.contexts
            .entry(trimmed.to_string())
            .or_insert_with(|| WindowContext::from_id(trimmed));
        win.config.context = trimmed.to_string();
        true
    }

    pub fn context(&self, id: &str) -> Option<&WindowContext> {
        self.contexts.get(id)
    }

    pub fn context_mut(&mut self, id: &str) -> Option<&mut WindowContext> {
        self.contexts.get_mut(id)
    }

    pub fn context_for_window(&self, window_id: &str) -> Option<&WindowContext> {
        self.window(window_id)
            .and_then(|w| self.contexts.get(&w.config.context))
    }

    pub fn context_for_window_mut(&mut self, window_id: &str) -> Option<&mut WindowContext> {
        let ctx_key = self.window(window_id)?.config.context.clone();
        self.contexts.get_mut(&ctx_key)
    }

    pub fn window(&self, id: &str) -> Option<&WindowState> {
        self.windows.iter().find(|w| w.config.id == id)
    }

    pub fn window_mut(&mut self, id: &str) -> Option<&mut WindowState> {
        self.windows.iter_mut().find(|w| w.config.id == id)
    }

    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    pub fn main_window(&self) -> &WindowState {
        if let Some(fid) = &self.focused_id
            && let Some(found) = self.windows.iter().find(|w| &w.config.id == fid)
        {
            return found;
        }
        &self.windows[0]
    }

    pub fn main_window_mut(&mut self) -> &mut WindowState {
        if let Some(fid) = self.focused_id.clone()
            && let Some(pos) = self.windows.iter().position(|w| w.config.id == fid)
        {
            return &mut self.windows[pos];
        }
        &mut self.windows[0]
    }

    /// Returns windows sorted strictly by `(layer, z_index, order_seq)` ascending.
    pub fn windows_sorted(&self) -> Vec<&WindowState> {
        let mut refs: Vec<&WindowState> = self.windows.iter().collect();
        refs.sort_by_key(|w| (w.config.layer, w.config.z_index, w.order_seq));
        refs
    }

    pub fn focused_id(&self) -> Option<&str> {
        self.focused_id.as_deref()
    }

    pub fn focus_window(&mut self, id: &str) -> bool {
        let can_focus = self
            .windows
            .iter()
            .any(|w| w.config.id == id && w.config.visible && !w.minimized);
        if !can_focus {
            return false;
        }
        self.focused_id = Some(id.to_string());
        self.sync_focus_flags();
        true
    }

    pub fn cycle_focus(&mut self) -> Option<&str> {
        let mut eligible: Vec<usize> = self
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| w.config.visible && !w.minimized)
            .map(|(idx, _)| idx)
            .collect();
        if eligible.is_empty() {
            return None;
        }
        eligible.sort_by_key(|&i| (self.windows[i].config.layer, self.windows[i].config.z_index, self.windows[i].order_seq));
        let cur_pos = self
            .focused_id
            .as_deref()
            .and_then(|fid| eligible.iter().position(|&i| self.windows[i].config.id == fid))
            .map(|p| (p + 1) % eligible.len())
            .unwrap_or(0);
        let next_id = self.windows[eligible[cur_pos]].config.id.clone();
        self.focused_id = Some(next_id);
        self.sync_focus_flags();
        self.focused_id.as_deref()
    }

    fn sync_focus_flags(&mut self) {
        let fid = self.focused_id.as_deref();
        for w in &mut self.windows {
            w.focused = fid == Some(w.config.id.as_str());
        }
    }

    pub fn set_visible(&mut self, id: &str, visible: bool) -> bool {
        let Some(win) = self.window_mut(id) else {
            return false;
        };
        win.config.visible = visible;
        if !visible && self.focused_id.as_deref() == Some(id) {
            self.focused_id = self
                .windows
                .iter()
                .find(|w| w.config.visible && !w.minimized)
                .map(|w| w.config.id.clone());
            self.sync_focus_flags();
        }
        true
    }

    pub fn move_window(
        &mut self,
        id: &str,
        x: u16,
        y: u16,
        viewport: Rect,
    ) -> Result<Rect, String> {
        let idx = self
            .windows
            .iter()
            .position(|w| w.config.id == id)
            .ok_or_else(|| format!("Window '{id}' not found"))?;

        if !self.windows[idx].config.can_move() {
            return Err(format!("Window '{id}' is static or non-movable"));
        }
        if self.windows[idx].minimized || self.windows[idx].maximized || self.windows[idx].config.fullscreen {
            return Err(format!("Cannot move '{id}' while minimized, maximized, or fullscreen"));
        }

        let mut candidate_cfg = self.windows[idx].config.clone();
        candidate_cfg.position = PositionMode::Absolute;
        candidate_cfg.anchor = Anchor::TopLeft;
        candidate_cfg.x = x;
        candidate_cfg.y = y;
        candidate_cfg.offset_x = 0;
        candidate_cfg.offset_y = 0;
        let candidate_rect = resolve_window_rect(&candidate_cfg, viewport);

        if let Some(blocking) = self.first_overlap_conflict(id, candidate_cfg.overlap, candidate_rect) {
            return Err(format!(
                "Geometry conflict with non-overlapping window '{blocking}'"
            ));
        }

        self.windows[idx].config = candidate_cfg;
        self.windows[idx].resolved_rect = candidate_rect;
        Ok(candidate_rect)
    }

    pub fn resize_window(
        &mut self,
        id: &str,
        width: u16,
        height: u16,
        viewport: Rect,
    ) -> Result<Rect, String> {
        let idx = self
            .windows
            .iter()
            .position(|w| w.config.id == id)
            .ok_or_else(|| format!("Window '{id}' not found"))?;

        if !self.windows[idx].config.can_resize() {
            return Err(format!("Window '{id}' is not resizable"));
        }
        if self.windows[idx].minimized || self.windows[idx].maximized || self.windows[idx].config.fullscreen {
            return Err(format!("Cannot resize '{id}' while minimized, maximized, or fullscreen"));
        }

        let mut candidate_cfg = self.windows[idx].config.clone();
        candidate_cfg.width = SizeSpec::Cells(width);
        candidate_cfg.height = SizeSpec::Cells(height);
        let candidate_rect = resolve_window_rect(&candidate_cfg, viewport);

        if let Some(blocking) = self.first_overlap_conflict(id, candidate_cfg.overlap, candidate_rect) {
            return Err(format!(
                "Geometry conflict with non-overlapping window '{blocking}'"
            ));
        }

        self.windows[idx].config = candidate_cfg;
        self.windows[idx].resolved_rect = candidate_rect;
        Ok(candidate_rect)
    }

    pub fn minimize(&mut self, id: &str) -> Result<(), String> {
        let idx = self
            .windows
            .iter()
            .position(|w| w.config.id == id)
            .ok_or_else(|| format!("Window '{id}' not found"))?;
        if !self.windows[idx].config.minimizable {
            return Err(format!("Window '{id}' is not minimizable"));
        }
        self.windows[idx].minimized = true;
        if self.focused_id.as_deref() == Some(id) {
            self.focused_id = self
                .windows
                .iter()
                .find(|w| w.config.id != id && w.config.visible && !w.minimized)
                .map(|w| w.config.id.clone());
            self.sync_focus_flags();
        }
        Ok(())
    }

    pub fn restore_minimized(&mut self, id: &str, viewport: Rect) -> bool {
        let Some(win) = self.window_mut(id) else {
            return false;
        };
        if !win.minimized {
            return false;
        }
        win.minimized = false;
        win.resolved_rect = resolve_state_rect(win, viewport);
        let _ = self.focus_window(id);
        true
    }

    pub fn maximize(&mut self, id: &str, viewport: Rect) -> Result<(), String> {
        let idx = self
            .windows
            .iter()
            .position(|w| w.config.id == id)
            .ok_or_else(|| format!("Window '{id}' not found"))?;
        if !self.windows[idx].config.maximizable {
            return Err(format!("Window '{id}' is not maximizable"));
        }
        if self.windows[idx].maximized {
            return Ok(());
        }
        self.windows[idx].minimized = false;
        self.windows[idx].save_geometry_if_absent();
        self.windows[idx].maximized = true;
        self.windows[idx].config.position = PositionMode::Absolute;
        self.windows[idx].config.anchor = Anchor::TopLeft;
        self.windows[idx].config.x = 0;
        self.windows[idx].config.y = 0;
        self.windows[idx].config.offset_x = 0;
        self.windows[idx].config.offset_y = 0;
        self.windows[idx].config.width = SizeSpec::Percent(100);
        self.windows[idx].config.height = SizeSpec::Percent(100);
        self.windows[idx].resolved_rect = viewport;
        let _ = self.focus_window(id);
        Ok(())
    }

    pub fn restore_maximized(&mut self, id: &str, viewport: Rect) -> bool {
        let Some(win) = self.window_mut(id) else {
            return false;
        };
        if !win.maximized {
            return false;
        }
        win.maximized = false;
        let restored = win.restore_saved_geometry();
        win.resolved_rect = resolve_state_rect(win, viewport);
        restored
    }

    pub fn set_fullscreen(&mut self, id: &str, enabled: bool, viewport: Rect) -> bool {
        let Some(win) = self.window_mut(id) else {
            return false;
        };
        if enabled == win.config.fullscreen {
            return true;
        }
        if enabled {
            win.minimized = false;
            win.save_geometry_if_absent();
            win.config.fullscreen = true;
            win.resolved_rect = viewport;
        } else {
            win.config.fullscreen = false;
            if !win.maximized {
                win.restore_saved_geometry();
            }
            win.resolved_rect = resolve_state_rect(win, viewport);
        }
        true
    }

    pub fn resolve_all(&mut self, viewport: Rect) {
        for win in &mut self.windows {
            win.resolved_rect = resolve_state_rect(win, viewport);
        }
    }

    /// Detects pairs of visible, non-minimized windows whose resolved rects intersect
    /// when at least one forbids overlap (`overlap = false`).
    pub fn geometry_conflicts(&self, viewport: Rect) -> Vec<(String, String)> {
        let sorted = self.windows_sorted();
        let active: Vec<(&str, bool, Rect)> = sorted
            .into_iter()
            .filter(|w| w.config.visible && !w.minimized)
            .map(|w| (w.config.id.as_str(), w.config.overlap, resolve_state_rect(w, viewport)))
            .collect();

        let mut conflicts = Vec::new();
        for i in 0..active.len() {
            for j in (i + 1)..active.len() {
                if (!active[i].1 || !active[j].1) && rects_intersect(active[i].2, active[j].2) {
                    conflicts.push((active[i].0.to_string(), active[j].0.to_string()));
                }
            }
        }
        conflicts
    }

    fn first_overlap_conflict(
        &self,
        moving_id: &str,
        moving_overlap: bool,
        candidate_rect: Rect,
    ) -> Option<String> {
        for other in &self.windows {
            if other.config.id == moving_id || !other.config.visible || other.minimized {
                continue;
            }
            if (!moving_overlap || !other.config.overlap)
                && rects_intersect(candidate_rect, other.resolved_rect)
            {
                return Some(other.config.id.clone());
            }
        }
        None
    }

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn begin_mouse_down(&mut self, col: u16, row: u16, viewport: Rect) -> bool {
        self.resolve_all(viewport);
        let mut order: Vec<usize> = (0..self.windows.len()).collect();
        order.sort_by_key(|&i| {
            std::cmp::Reverse((
                self.windows[i].config.layer,
                self.windows[i].config.z_index,
                self.windows[i].order_seq,
            ))
        });

        for idx in order {
            let hit = self.windows[idx].hit_test(col, row);
            if hit == WindowHit::None {
                continue;
            }
            let id = self.windows[idx].config.id.clone();
            let _ = self.focus_window(&id);
            match hit {
                WindowHit::TitleBar if self.windows[idx].config.can_move() && !self.windows[idx].maximized && !self.windows[idx].config.fullscreen => {
                    let rect = self.windows[idx].resolved_rect;
                    self.windows[idx].config.position = PositionMode::Absolute;
                    self.windows[idx].config.x = rect.x;
                    self.windows[idx].config.y = rect.y;
                    self.windows[idx].config.offset_x = 0;
                    self.windows[idx].config.offset_y = 0;
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
                WindowHit::ResizeCorner if self.windows[idx].config.can_resize() && !self.windows[idx].maximized && !self.windows[idx].config.fullscreen => {
                    let rect = self.windows[idx].resolved_rect;
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
                WindowHit::Body | WindowHit::TitleBar | WindowHit::ResizeCorner => {
                    return false;
                }
                WindowHit::None => {}
            }
        }
        false
    }

    pub fn update_mouse_drag(&mut self, col: u16, row: u16, viewport: Rect) -> bool {
        let Some((ref id, drag)) = self.active_drag.clone() else {
            return false;
        };
        let Some(win_idx) = self.windows.iter().position(|w| &w.config.id == id) else {
            self.active_drag = None;
            return false;
        };

        let mut candidate_cfg = self.windows[win_idx].config.clone();
        match drag {
            DragKind::Move {
                start_mouse_x,
                start_mouse_y,
                orig_x,
                orig_y,
            } => {
                if !candidate_cfg.can_move() {
                    return false;
                }
                let cur_rect = self.windows[win_idx].resolved_rect;
                let dx = col as i32 - start_mouse_x as i32;
                let dy = row as i32 - start_mouse_y as i32;
                let max_x = viewport.right().saturating_sub(cur_rect.width) as i32;
                let max_y = viewport.bottom().saturating_sub(cur_rect.height) as i32;
                candidate_cfg.position = PositionMode::Absolute;
                candidate_cfg.x =
                    (orig_x as i32 + dx).clamp(viewport.x as i32, max_x.max(viewport.x as i32)) as u16;
                candidate_cfg.y =
                    (orig_y as i32 + dy).clamp(viewport.y as i32, max_y.max(viewport.y as i32)) as u16;
            }
            DragKind::ResizeBottomRight {
                start_mouse_x,
                start_mouse_y,
                orig_w,
                orig_h,
            } => {
                if !candidate_cfg.can_resize() {
                    return false;
                }
                let cur_rect = self.windows[win_idx].resolved_rect;
                let dw = col as i32 - start_mouse_x as i32;
                let dh = row as i32 - start_mouse_y as i32;
                let max_w = viewport.right().saturating_sub(cur_rect.x);
                let max_h = viewport.bottom().saturating_sub(cur_rect.y);
                let min_w = candidate_cfg.min_width.min(max_w).max(12);
                let min_h = candidate_cfg.min_height.min(max_h).max(6);
                let new_w = (orig_w as i32 + dw).clamp(min_w as i32, max_w.max(min_w) as i32) as u16;
                let new_h = (orig_h as i32 + dh).clamp(min_h as i32, max_h.max(min_h) as i32) as u16;
                candidate_cfg.width = SizeSpec::Cells(new_w);
                candidate_cfg.height = SizeSpec::Cells(new_h);
            }
        }

        let candidate_rect = resolve_window_rect(&candidate_cfg, viewport);
        if self
            .first_overlap_conflict(id, candidate_cfg.overlap, candidate_rect)
            .is_some()
        {
            return false;
        }
        self.windows[win_idx].config = candidate_cfg;
        self.windows[win_idx].resolved_rect = candidate_rect;
        true
    }

    pub fn end_mouse_drag(&mut self) {
        self.active_drag = None;
    }

    pub fn nudge_main(&mut self, dx: i16, dy: i16, viewport: Rect) {
        let Some(fid) = self.focused_id.clone().or_else(|| self.windows.first().map(|w| w.config.id.clone())) else {
            return;
        };
        let Some(win_idx) = self.windows.iter().position(|w| w.config.id == fid) else {
            return;
        };
        if !self.windows[win_idx].config.can_move()
            || self.windows[win_idx].minimized
            || self.windows[win_idx].maximized
            || self.windows[win_idx].config.fullscreen
        {
            return;
        }
        let mut candidate_cfg = self.windows[win_idx].config.clone();
        let cur_rect = self.windows[win_idx].resolved_rect;
        if candidate_cfg.position == PositionMode::Absolute {
            let max_x = viewport.right().saturating_sub(cur_rect.width) as i32;
            let max_y = viewport.bottom().saturating_sub(cur_rect.height) as i32;
            candidate_cfg.x = (candidate_cfg.x as i32 + dx as i32)
                .clamp(viewport.x as i32, max_x.max(viewport.x as i32)) as u16;
            candidate_cfg.y = (candidate_cfg.y as i32 + dy as i32)
                .clamp(viewport.y as i32, max_y.max(viewport.y as i32)) as u16;
        } else {
            candidate_cfg.offset_x = candidate_cfg.offset_x.saturating_add(dx);
            candidate_cfg.offset_y = candidate_cfg.offset_y.saturating_add(dy);
        }
        let candidate_rect = resolve_window_rect(&candidate_cfg, viewport);
        if self
            .first_overlap_conflict(&fid, candidate_cfg.overlap, candidate_rect)
            .is_some()
        {
            return;
        }
        self.windows[win_idx].config = candidate_cfg;
        self.windows[win_idx].resolved_rect = candidate_rect;
    }

    pub fn resize_main_by(&mut self, dw: i16, dh: i16, viewport: Rect) {
        let Some(fid) = self.focused_id.clone().or_else(|| self.windows.first().map(|w| w.config.id.clone())) else {
            return;
        };
        let Some(win_idx) = self.windows.iter().position(|w| w.config.id == fid) else {
            return;
        };
        if !self.windows[win_idx].config.can_resize()
            || self.windows[win_idx].minimized
            || self.windows[win_idx].maximized
            || self.windows[win_idx].config.fullscreen
        {
            return;
        }
        let mut candidate_cfg = self.windows[win_idx].config.clone();
        let cur = resolve_window_rect(&candidate_cfg, viewport);
        let new_w = (cur.width as i32 + dw as i32)
            .clamp(candidate_cfg.min_width.min(viewport.width) as i32, viewport.width as i32)
            as u16;
        let new_h = (cur.height as i32 + dh as i32)
            .clamp(candidate_cfg.min_height.min(viewport.height) as i32, viewport.height as i32)
            as u16;
        candidate_cfg.width = SizeSpec::Cells(new_w);
        candidate_cfg.height = SizeSpec::Cells(new_h);
        let candidate_rect = resolve_window_rect(&candidate_cfg, viewport);
        if self
            .first_overlap_conflict(&fid, candidate_cfg.overlap, candidate_rect)
            .is_some()
        {
            return;
        }
        self.windows[win_idx].config = candidate_cfg;
        self.windows[win_idx].resolved_rect = candidate_rect;
    }
}

pub fn resolve_state_rect(state: &WindowState, viewport: Rect) -> Rect {
    if !state.config.visible || state.minimized || viewport.width == 0 || viewport.height == 0 {
        return Rect::ZERO;
    }
    if state.config.fullscreen || state.maximized {
        return viewport;
    }
    resolve_window_rect(&state.config, viewport)
}

pub fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.width > 0
        && a.height > 0
        && b.width > 0
        && b.height > 0
        && a.x < b.right()
        && b.x < a.right()
        && a.y < b.bottom()
        && b.y < a.bottom()
}

pub fn resolve_window_rect(cfg: &WindowConfig, viewport: Rect) -> Rect {
    if !cfg.visible || viewport.width == 0 || viewport.height == 0 {
        return Rect::ZERO;
    }
    if cfg.fullscreen {
        return viewport;
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
