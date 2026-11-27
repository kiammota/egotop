use crate::window::manager::{rects_intersect, resolve_state_rect};
use crate::window::{WindowConfig, WindowManager, WindowState, inner_area_for_rect};
use ratatui::layout::Rect;

/// Visual composition unit emitted by `Compositor::compose` in exact `(layer, z_index)` order.
#[derive(Debug, Clone)]
pub struct ComposedWindow {
    pub window_id: String,
    pub context_id: String,
    pub config: WindowConfig,
    pub rect: Rect,
    pub inner_area: Rect,
    pub focused: bool,
}

/// Visual compositor responsible solely for ordering, terminal clamping, and enforcing
/// non-overlapping rules across managed windows.
pub struct Compositor;

impl Compositor {
    pub fn compose(manager: &WindowManager, viewport: Rect) -> Vec<ComposedWindow> {
        if viewport.width == 0 || viewport.height == 0 {
            return Vec::new();
        }

        let ordered = manager.windows_sorted();
        let mut placed: Vec<(ComposedWindow, bool)> = Vec::new();

        for win in ordered {
            if !win.config.visible || win.minimized {
                continue;
            }
            let raw_rect = clamp_to_viewport(resolve_state_rect(win, viewport), viewport);
            if raw_rect.width == 0 || raw_rect.height == 0 {
                continue;
            }

            let resolved = resolve_composition_rect(win, raw_rect, &placed, viewport);
            let Some(rect) = resolved else {
                continue;
            };

            let composed = ComposedWindow {
                window_id: win.config.id.clone(),
                context_id: win.config.context.clone(),
                config: win.config.clone(),
                rect,
                inner_area: inner_area_for_rect(&win.config, rect),
                focused: win.focused,
            };
            placed.push((composed, win.config.overlap));
        }

        placed.into_iter().map(|(c, _)| c).collect()
    }
}

fn resolve_composition_rect(
    win: &WindowState,
    desired: Rect,
    placed: &[(ComposedWindow, bool)],
    viewport: Rect,
) -> Option<Rect> {
    let conflicting: Vec<Rect> = placed
        .iter()
        .filter(|(other, other_overlap)| {
            (!win.config.overlap || !*other_overlap) && rects_intersect(desired, other.rect)
        })
        .map(|(other, _)| other.rect)
        .collect();

    if conflicting.is_empty() {
        return Some(desired);
    }

    // Try shifting right, below, left, or above non-overlapping windows inside terminal bounds.
    let mut candidates = Vec::new();
    for blocker in &conflicting {
        // Right of blocker
        if blocker.right() + desired.width <= viewport.right() {
            candidates.push(Rect::new(
                blocker.right(),
                desired.y,
                desired.width,
                desired.height,
            ));
        }
        // Below blocker
        if blocker.bottom() + desired.height <= viewport.bottom() {
            candidates.push(Rect::new(
                desired.x,
                blocker.bottom(),
                desired.width,
                desired.height,
            ));
        }
        // Left of blocker
        if blocker.x >= viewport.x + desired.width {
            candidates.push(Rect::new(
                blocker.x - desired.width,
                desired.y,
                desired.width,
                desired.height,
            ));
        }
        // Above blocker
        if blocker.y >= viewport.y + desired.height {
            candidates.push(Rect::new(
                desired.x,
                blocker.y - desired.height,
                desired.width,
                desired.height,
            ));
        }
    }

    if win.config.can_move() {
        for cand in candidates {
            let clamped = clamp_to_viewport(cand, viewport);
            if clamped.width >= 4
                && clamped.height >= 3
                && is_conflict_free(clamped, win.config.overlap, placed)
            {
                return Some(clamped);
            }
        }
    }

    // If non-movable or no full shift fits cleanly, trim width/height cleanly around blocker.
    for blocker in &conflicting {
        if blocker.right() < desired.right() {
            let trimmed_w = desired.right() - blocker.right();
            let trimmed = Rect::new(blocker.right(), desired.y, trimmed_w, desired.height);
            if trimmed.width >= 4 && is_conflict_free(trimmed, win.config.overlap, placed) {
                return Some(trimmed);
            }
        }
        if blocker.bottom() < desired.bottom() {
            let trimmed_h = desired.bottom() - blocker.bottom();
            let trimmed = Rect::new(desired.x, blocker.bottom(), desired.width, trimmed_h);
            if trimmed.height >= 3 && is_conflict_free(trimmed, win.config.overlap, placed) {
                return Some(trimmed);
            }
        }
    }

    // Never draw over an overlap=false window if no clean space exists.
    None
}

fn is_conflict_free(candidate: Rect, allow_overlap: bool, placed: &[(ComposedWindow, bool)]) -> bool {
    !placed
        .iter()
        .any(|(other, other_overlap)| (!allow_overlap || !*other_overlap) && rects_intersect(candidate, other.rect))
}

fn clamp_to_viewport(rect: Rect, viewport: Rect) -> Rect {
    let x = rect.x.clamp(viewport.x, viewport.right());
    let y = rect.y.clamp(viewport.y, viewport.bottom());
    let w = rect.width.min(viewport.right().saturating_sub(x));
    let h = rect.height.min(viewport.bottom().saturating_sub(y));
    Rect::new(x, y, w, h)
}
