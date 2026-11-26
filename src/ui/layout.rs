use crate::config::{
    Anchor, Config, LayoutMode, SizeSpec, ViewMode, WidgetConfig, WidgetId,
};
use ratatui::layout::Rect;

#[derive(Debug, Clone)]
pub struct PlacedWidget {
    pub config: WidgetConfig,
    pub area: Rect,
}

#[derive(Debug, Clone)]
pub struct ComputedLayout {
    pub header: Option<Rect>,
    pub body: Rect,
    pub footer: Rect,
    pub widgets: Vec<PlacedWidget>,
}

pub fn compute_layout(
    screen: Rect,
    cfg: &Config,
    active_page: u8,
    view_mode: ViewMode,
) -> ComputedLayout {
    if screen.width < 10 || screen.height < 6 {
        return ComputedLayout {
            header: None,
            body: screen,
            footer: Rect::ZERO,
            widgets: Vec::new(),
        };
    }

    let pad = cfg.layout.padding.min(screen.width / 8).min(screen.height / 8);
    let inner = Rect::new(
        screen.x + pad,
        screen.y + pad,
        screen.width.saturating_sub(pad * 2),
        screen.height.saturating_sub(pad * 2),
    );

    let header_h = if cfg.header.enabled && inner.height >= 10 {
        if inner.height >= 22 { 3 } else { 1 }
    } else {
        0
    };
    let footer_h = if inner.height >= 8 { 1 } else { 0 };

    let header = (header_h > 0).then(|| Rect::new(inner.x, inner.y, inner.width, header_h));
    let footer = (footer_h > 0).then(|| {
        Rect::new(
            inner.x,
            inner.bottom().saturating_sub(footer_h),
            inner.width,
            footer_h,
        )
    });
    let body_y = inner.y + header_h;
    let body_h = inner
        .height
        .saturating_sub(header_h)
        .saturating_sub(footer_h);
    let body = Rect::new(inner.x, body_y, inner.width, body_h);

    let active_widgets = select_widgets_for_view(cfg, active_page, view_mode);
    let mut placed = match (cfg.layout.mode, view_mode) {
        (_, ViewMode::Minimal) => layout_minimal(body, &active_widgets),
        (LayoutMode::Free, _) => layout_free(body, &active_widgets),
        (LayoutMode::Compact, _) | (_, ViewMode::Compact) => {
            layout_responsive(body, &active_widgets, cfg.layout.gap, cfg.layout.min_width_for_split, true)
        }
        (LayoutMode::Responsive, _) => {
            layout_responsive(body, &active_widgets, cfg.layout.gap, cfg.layout.min_width_for_split, false)
        }
    };

    // Respect configured z_index for deterministic render order
    let indexed: Vec<(usize, PlacedWidget)> = placed.drain(..).enumerate().collect();
    let mut sorted = indexed;
    sorted.sort_by_key(|(idx, p)| (p.config.z_index, *idx));
    let widgets = sorted.into_iter().map(|(_, p)| p).collect();

    ComputedLayout {
        header,
        body,
        footer: footer.unwrap_or(Rect::ZERO),
        widgets,
    }
}

fn select_widgets_for_view(cfg: &Config, active_page: u8, view_mode: ViewMode) -> Vec<WidgetConfig> {
    if view_mode == ViewMode::Minimal {
        let mut minimal = Vec::new();
        if let Some(g) = cfg.widgets.iter().find(|w| w.id == WidgetId::Greeting && w.enabled) {
            let mut w = g.clone();
            w.position = Anchor::Center;
            minimal.push(w);
        } else {
            minimal.push(WidgetConfig {
                id: WidgetId::Greeting,
                position: Anchor::Center,
                ..WidgetConfig::default()
            });
        }
        return minimal;
    }

    let explicit_for_page: Vec<WidgetConfig> = cfg
        .widgets
        .iter()
        .filter(|w| w.enabled && w.page.unwrap_or(1) == active_page)
        .cloned()
        .collect();

    if !explicit_for_page.is_empty() || active_page == 1 {
        return explicit_for_page;
    }

    // Built-in secondary pages when user hasn't customized page 2 / page 3 widgets
    match active_page {
        2 => vec![
            WidgetConfig {
                id: WidgetId::Hardware,
                title: Some("Hardware Detalhado".to_string()),
                position: Anchor::TopLeft,
                width: Some(SizeSpec::Percent(50)),
                page: Some(2),
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::Activity,
                title: Some("Atividade & Gráficos".to_string()),
                position: Anchor::TopRight,
                width: Some(SizeSpec::Percent(50)),
                page: Some(2),
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::System,
                title: Some("Sistema & Ambiente".to_string()),
                position: Anchor::BottomCenter,
                width: Some(SizeSpec::Percent(100)),
                page: Some(2),
                ..WidgetConfig::default()
            },
        ],
        3 => vec![
            WidgetConfig {
                id: WidgetId::Session,
                title: Some("Sessão & Hyprland".to_string()),
                position: Anchor::TopLeft,
                width: Some(SizeSpec::Percent(50)),
                page: Some(3),
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::Uptime,
                title: Some("Tempo & Boot".to_string()),
                position: Anchor::TopRight,
                width: Some(SizeSpec::Percent(50)),
                page: Some(3),
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::Message,
                title: Some("Mensagem & Integração".to_string()),
                position: Anchor::BottomCenter,
                width: Some(SizeSpec::Percent(100)),
                page: Some(3),
                ..WidgetConfig::default()
            },
        ],
        _ => Vec::new(),
    }
}

fn layout_minimal(body: Rect, widgets: &[WidgetConfig]) -> Vec<PlacedWidget> {
    let Some(first) = widgets.first() else {
        return Vec::new();
    };
    let w = (body.width * 85 / 100).max(30).min(body.width);
    let h = 11.min(body.height);
    let rect = place_by_anchor(body, Anchor::Center, w, h, None, None, 0, 0);
    vec![PlacedWidget {
        config: first.clone(),
        area: rect,
    }]
}

fn layout_free(body: Rect, widgets: &[WidgetConfig]) -> Vec<PlacedWidget> {
    widgets
        .iter()
        .map(|w| {
            let default_w = (body.width / 2).max(24).min(body.width);
            let default_h = match w.id {
                WidgetId::Greeting | WidgetId::Logo => 10,
                WidgetId::Message => 5,
                _ => (body.height / 3).max(6),
            };
            let rw = resolve_size(w.width, body.width, default_w).min(body.width);
            let rh = resolve_size(w.height, body.height, default_h).min(body.height);
            let area = place_by_anchor(
                body,
                w.position,
                rw,
                rh,
                w.x,
                w.y,
                w.offset_x,
                w.offset_y,
            );
            PlacedWidget {
                config: w.clone(),
                area,
            }
        })
        .collect()
}

fn layout_responsive(
    body: Rect,
    widgets: &[WidgetConfig],
    gap: u16,
    min_width_for_split: u16,
    compact: bool,
) -> Vec<PlacedWidget> {
    if body.width == 0 || body.height == 0 || widgets.is_empty() {
        return Vec::new();
    }

    let mut placed = Vec::new();
    let mut top_band = Vec::new();
    let mut mid_band = Vec::new();
    let mut bot_band = Vec::new();

    for w in widgets {
        match w.position {
            Anchor::Absolute => {
                let rw = resolve_size(w.width, body.width, (body.width / 2).max(20));
                let rh = resolve_size(w.height, body.height, (body.height / 3).max(5));
                let area = place_by_anchor(body, Anchor::Absolute, rw, rh, w.x, w.y, w.offset_x, w.offset_y);
                placed.push(PlacedWidget {
                    config: w.clone(),
                    area,
                });
            }
            Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => top_band.push(w.clone()),
            Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight => mid_band.push(w.clone()),
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => bot_band.push(w.clone()),
        }
    }

    let bands: Vec<Vec<WidgetConfig>> = [top_band, mid_band, bot_band]
        .into_iter()
        .filter(|b| !b.is_empty())
        .collect();

    if bands.is_empty() {
        return placed;
    }

    let split_horizontally = body.width >= min_width_for_split;
    let total_gaps = gap.saturating_mul(bands.len().saturating_sub(1) as u16);
    let avail_h = body.height.saturating_sub(total_gaps);

    // Compute desired height weights for each band
    let band_heights = allocate_band_heights(&bands, avail_h, split_horizontally, compact);

    let mut cur_y = body.y;
    for (idx, mut band) in bands.into_iter().enumerate() {
        let bh = band_heights[idx];
        if bh == 0 {
            continue;
        }
        band.sort_by_key(|w| anchor_horizontal_order(w.position));
        let band_rect = Rect::new(body.x, cur_y, body.width, bh);

        if split_horizontally && band.len() > 1 {
            let h_gaps = gap.saturating_mul(band.len().saturating_sub(1) as u16);
            let avail_w = band_rect.width.saturating_sub(h_gaps);
            let widths = allocate_widget_widths(&band, avail_w);

            let mut cur_x = band_rect.x;
            for (w_idx, w) in band.into_iter().enumerate() {
                let ww = widths[w_idx];
                let wh = w
                    .height
                    .map(|s| resolve_size(Some(s), bh, bh))
                    .unwrap_or(bh)
                    .min(bh);
                let cell = Rect::new(cur_x, band_rect.y, ww, wh);
                let clamped = apply_offset_and_clamp(cell, body, w.offset_x, w.offset_y);
                placed.push(PlacedWidget {
                    config: w,
                    area: clamped,
                });
                cur_x = cur_x.saturating_add(ww).saturating_add(gap);
            }
        } else if band.len() == 1 {
            let w = band.into_iter().next().unwrap();
            let ww = w
                .width
                .map(|s| resolve_size(Some(s), band_rect.width, band_rect.width))
                .unwrap_or(band_rect.width)
                .min(band_rect.width);
            let wh = w
                .height
                .map(|s| resolve_size(Some(s), bh, bh))
                .unwrap_or(bh)
                .min(bh);
            let x = match w.position {
                Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => {
                    band_rect.right().saturating_sub(ww)
                }
                Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => {
                    band_rect.x + band_rect.width.saturating_sub(ww) / 2
                }
                _ => band_rect.x,
            };
            let cell = Rect::new(x, band_rect.y, ww, wh);
            placed.push(PlacedWidget {
                area: apply_offset_and_clamp(cell, body, w.offset_x, w.offset_y),
                config: w,
            });
        } else {
            // Narrow terminal fallback: stack widgets vertically inside the band
            let n = band.len() as u16;
            let each_h = (bh / n).max(3);
            let mut wy = band_rect.y;
            for (i, w) in band.into_iter().enumerate() {
                let remaining = band_rect.bottom().saturating_sub(wy);
                if remaining == 0 {
                    break;
                }
                let h = if i + 1 == n as usize {
                    remaining
                } else {
                    each_h.min(remaining)
                };
                let cell = Rect::new(band_rect.x, wy, band_rect.width, h);
                placed.push(PlacedWidget {
                    area: apply_offset_and_clamp(cell, body, w.offset_x, w.offset_y),
                    config: w,
                });
                wy = wy.saturating_add(h);
            }
        }

        cur_y = cur_y.saturating_add(bh).saturating_add(gap);
    }

    placed
}

fn allocate_band_heights(
    bands: &[Vec<WidgetConfig>],
    avail_h: u16,
    split_horizontally: bool,
    compact: bool,
) -> Vec<u16> {
    if bands.is_empty() || avail_h == 0 {
        return vec![0; bands.len()];
    }

    let mut heights = vec![0u16; bands.len()];
    let mut preferred = Vec::with_capacity(bands.len());

    for band in bands {
        let explicit = band
            .iter()
            .filter_map(|w| w.height.map(|s| resolve_size(Some(s), avail_h, 8)))
            .max();
        if let Some(h) = explicit {
            preferred.push(h.max(3));
            continue;
        }

        let has_greeting = band
            .iter()
            .any(|w| matches!(w.id, WidgetId::Greeting | WidgetId::Logo));
        let has_message_only = band.len() == 1 && band[0].id == WidgetId::Message;

        let base = if has_greeting {
            if compact {
                6
            } else if avail_h >= 28 {
                10
            } else {
                8
            }
        } else if has_message_only {
            3
        } else if !split_horizontally && band.len() > 1 {
            (band.len() as u16) * 6
        } else {
            9
        };
        preferred.push(base);
    }

    let sum_pref: u32 = preferred.iter().map(|&h| h as u32).sum();
    if sum_pref == 0 {
        return heights;
    }

    let mut assigned = 0u16;
    for (i, &pref) in preferred.iter().enumerate() {
        let h = ((avail_h as u32 * pref as u32) / sum_pref) as u16;
        heights[i] = h.max(3.min(avail_h));
        assigned = assigned.saturating_add(heights[i]);
    }

    // Distribute rounding remainder or trim overflow gracefully
    while assigned < avail_h {
        let mut grew = false;
        for h in &mut heights {
            if assigned < avail_h {
                *h += 1;
                assigned += 1;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    while assigned > avail_h {
        if let Some(max_h) = heights.iter_mut().max() {
            if *max_h > 1 {
                *max_h -= 1;
                assigned -= 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    heights
}

fn allocate_widget_widths(band: &[WidgetConfig], avail_w: u16) -> Vec<u16> {
    let n = band.len();
    if n == 0 || avail_w == 0 {
        return vec![0; n];
    }
    let mut widths = vec![0u16; n];
    let mut remaining = avail_w;
    let mut unspecified = Vec::new();

    for (i, w) in band.iter().enumerate() {
        if let Some(spec) = w.width {
            let val = resolve_size(Some(spec), avail_w, avail_w / n as u16).min(remaining);
            widths[i] = val;
            remaining = remaining.saturating_sub(val);
        } else {
            unspecified.push(i);
        }
    }

    if !unspecified.is_empty() {
        let each = remaining / unspecified.len() as u16;
        let rem = remaining % unspecified.len() as u16;
        for (idx, pos) in unspecified.into_iter().enumerate() {
            widths[pos] = each + if (idx as u16) < rem { 1 } else { 0 };
        }
    } else if remaining > 0
        && let Some(last) = widths.last_mut()
    {
        *last = last.saturating_add(remaining);
    }

    widths
}

fn anchor_horizontal_order(anchor: Anchor) -> u8 {
    match anchor {
        Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => 0,
        Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => 1,
        Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => 2,
        Anchor::Absolute => 3,
    }
}

pub fn resolve_size(spec: Option<SizeSpec>, total: u16, fallback: u16) -> u16 {
    match spec {
        Some(SizeSpec::Cells(c)) => c.min(total),
        Some(SizeSpec::Percent(p)) => ((total as u32 * p.min(100) as u32) / 100) as u16,
        None => fallback.min(total),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_by_anchor(
    bounds: Rect,
    anchor: Anchor,
    width: u16,
    height: u16,
    abs_x: Option<u16>,
    abs_y: Option<u16>,
    offset_x: i16,
    offset_y: i16,
) -> Rect {
    let w = width.min(bounds.width);
    let h = height.min(bounds.height);

    let (x, y) = match anchor {
        Anchor::TopLeft => (bounds.x, bounds.y),
        Anchor::TopCenter => (bounds.x + bounds.width.saturating_sub(w) / 2, bounds.y),
        Anchor::TopRight => (bounds.right().saturating_sub(w), bounds.y),
        Anchor::CenterLeft => (bounds.x, bounds.y + bounds.height.saturating_sub(h) / 2),
        Anchor::Center => (
            bounds.x + bounds.width.saturating_sub(w) / 2,
            bounds.y + bounds.height.saturating_sub(h) / 2,
        ),
        Anchor::CenterRight => (
            bounds.right().saturating_sub(w),
            bounds.y + bounds.height.saturating_sub(h) / 2,
        ),
        Anchor::BottomLeft => (bounds.x, bounds.bottom().saturating_sub(h)),
        Anchor::BottomCenter => (
            bounds.x + bounds.width.saturating_sub(w) / 2,
            bounds.bottom().saturating_sub(h),
        ),
        Anchor::BottomRight => (
            bounds.right().saturating_sub(w),
            bounds.bottom().saturating_sub(h),
        ),
        Anchor::Absolute => (
            bounds.x.saturating_add(abs_x.unwrap_or(0)),
            bounds.y.saturating_add(abs_y.unwrap_or(0)),
        ),
    };

    apply_offset_and_clamp(Rect::new(x, y, w, h), bounds, offset_x, offset_y)
}

fn apply_offset_and_clamp(rect: Rect, bounds: Rect, dx: i16, dy: i16) -> Rect {
    let nx = (rect.x as i32 + dx as i32)
        .clamp(bounds.x as i32, bounds.right() as i32) as u16;
    let ny = (rect.y as i32 + dy as i32)
        .clamp(bounds.y as i32, bounds.bottom() as i32) as u16;
    let nw = rect.width.min(bounds.right().saturating_sub(nx));
    let nh = rect.height.min(bounds.bottom().saturating_sub(ny));
    Rect::new(nx, ny, nw, nh)
}
