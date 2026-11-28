use crate::config::{BorderStyle, Config, UiConfig, ViewMode};
use crate::ui::container::{
    ContainerConfig, ContainerSize, ContainerStyle, EffectPolicy, LayoutRegion,
};
use ratatui::layout::Rect;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct PlacedContainer {
    pub name: String,
    pub description: String,
    pub outer_rect: Rect,
    pub inner_rect: Rect,
    pub config: ContainerConfig,
}

#[derive(Debug, Clone, Default)]
pub struct ComputedLayout {
    pub area: Rect,
    pub containers: Vec<PlacedContainer>,
}

impl ComputedLayout {
    pub fn find_container(&self, name: &str) -> Option<&PlacedContainer> {
        self.containers.iter().find(|c| c.name == name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExclusionMask {
    pub rects: Vec<Rect>,
}

impl ExclusionMask {
    pub fn from_layout(layout: &ComputedLayout) -> Self {
        Self::from_layout_with_styles(layout, &BTreeMap::new())
    }

    pub fn from_layout_with_styles(
        layout: &ComputedLayout,
        styles: &BTreeMap<String, ContainerStyle>,
    ) -> Self {
        let mut rects = Vec::with_capacity(layout.containers.len());
        for c in &layout.containers {
            let policy = styles
                .get(&c.name)
                .map(|s| s.effects)
                .unwrap_or(c.config.effects);
            let transparent = styles
                .get(&c.name)
                .map(|s| matches!(s.background, crate::ui::container::BackgroundFill::Transparent))
                .unwrap_or(c.config.transparent);
            // Transparent containers intentionally reveal the background canvas
            // across their whole rectangle; opaque containers remain isolated.
            if policy != EffectPolicy::Allow && !transparent {
                rects.push(c.outer_rect);
            }
        }
        Self { rects }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.rects
            .iter()
            .any(|r| x >= r.x && x < r.right() && y >= r.y && y < r.bottom())
    }
}

pub fn compute_dashboard_layout(
    screen: Rect,
    cfg: &Config,
    view_mode: ViewMode,
) -> ComputedLayout {
    let mut ui = match view_mode {
        ViewMode::Dashboard => cfg.ui.clone(),
        ViewMode::Compact => {
            let mut compact = cfg.ui.clone();
            compact.padding = compact.padding.min(1);
            compact.gap = 0;
            for c in &mut compact.containers {
                c.padding = 0;
            }
            compact
        }
        ViewMode::Minimal => {
            let mut minimal = UiConfig {
                padding: cfg.ui.padding.min(1),
                gap: cfg.ui.gap,
                min_container_width: cfg.ui.min_container_width,
                containers: Vec::new(),
            };
            for target_name in ["greeting", "status_spinner"] {
                if let Some(c) = cfg
                    .ui
                    .containers
                    .iter()
                    .find(|c| c.name == target_name && c.visible)
                {
                    minimal.containers.push(c.clone());
                }
            }
            minimal
        }
    };

    // On short terminals, keep the activity graphs readable by reclaiming
    // spare welcome/footer height and inter-row gaps before the grid is sized.
    if screen.height < 38 {
        ui.padding = 0;
        ui.gap = 0;
        for c in &mut ui.containers {
            match c.region {
                LayoutRegion::Top if c.row == Some(1) => {
                    c.height = ContainerSize::Fixed(5);
                    c.min_height = Some(5);
                }
                LayoutRegion::Bottom => {
                    c.height = ContainerSize::Fixed(1);
                    c.min_height = Some(1);
                    c.border.enabled = false;
                }
                LayoutRegion::Main if c.row != Some(2) => {
                    c.min_height = Some(c.min_height.unwrap_or(0).min(3));
                }
                _ => {}
            }
        }
    }

    for c in &mut ui.containers {
        let ov_style = cfg
            .container_styles
            .get(&c.name)
            .and_then(|s| s.border_style);
        if let Some(st) = ov_style {
            if st == BorderStyle::None {
                c.border.enabled = false;
                c.border.style = Some(BorderStyle::None);
            } else {
                c.border.enabled = true;
                c.border.style = Some(st);
            }
        } else if cfg.theme.border_type == BorderStyle::None {
            c.border.enabled = false;
        }
        // Activity uses three information rows, a graph heading, and two chart
        // rows. Reserve enough outer height for that content plus its frame.
        if c.kind == crate::ui::container::ComponentKind::Activity
            && c.graph_style.unwrap_or(crate::config::GraphStyle::Sparkline)
                == crate::config::GraphStyle::Sparkline
        {
            let bordered = c.border.enabled && c.border.style != Some(BorderStyle::None);
            let titled = c.border.title.as_deref().or(c.content.title.as_deref()).is_some();
            let decoration = if bordered {
                2
            } else if titled {
                1
            } else {
                0
            };
            let padding = if bordered { c.padding.saturating_mul(2) } else { (c.padding.saturating_mul(2)).max(u16::from(titled)) };
            c.min_height = Some(c.min_height.unwrap_or(0).max(6u16.saturating_add(decoration).saturating_add(padding)));
        }
    }

    // Reserve the absolute bottom row permanently for the Vim-style command line
    // so opening, typing, or closing `:` never shifts or resizes dashboard containers.
    let dashboard_area = if screen.height >= 2 {
        Rect::new(screen.x, screen.y, screen.width, screen.height - 1)
    } else {
        screen
    };

    compute_layout(dashboard_area, &ui)
}

/// Computes deterministic, balanced, non-overlapping `Rect` geometry for a flat queue of
/// independent containers across interface regions (`top`, `left`, `main`, `right`, `bottom`).
pub fn compute_layout(viewport: Rect, ui: &UiConfig) -> ComputedLayout {
    let mut out = ComputedLayout {
        area: viewport,
        containers: Vec::new(),
    };
    if viewport.width == 0 || viewport.height == 0 {
        return out;
    }

    let pad_x = ui.padding.min(viewport.width / 2);
    let pad_y = ui.padding.min(viewport.height / 2);
    let work = Rect::new(
        viewport.x.saturating_add(pad_x),
        viewport.y.saturating_add(pad_y),
        viewport.width.saturating_sub(pad_x.saturating_mul(2)),
        viewport.height.saturating_sub(pad_y.saturating_mul(2)),
    )
    .intersection(viewport);

    if work.width == 0 || work.height == 0 {
        return out;
    }

    let mut ordered: Vec<(usize, &ContainerConfig)> = ui
        .containers
        .iter()
        .enumerate()
        .filter(|(_, c)| c.visible)
        .collect();
    ordered.sort_by_key(|(idx, c)| (c.order, *idx));

    if ordered.is_empty() {
        return out;
    }

    let mut top_items = Vec::new();
    let mut left_items = Vec::new();
    let mut main_items = Vec::new();
    let mut right_items = Vec::new();
    let mut bottom_items = Vec::new();

    for (_, c) in ordered {
        match c.region {
            LayoutRegion::Top => top_items.push(c),
            LayoutRegion::Left => left_items.push(c),
            LayoutRegion::Main => main_items.push(c),
            LayoutRegion::Right => right_items.push(c),
            LayoutRegion::Bottom => bottom_items.push(c),
        }
    }

    let min_col_w = ui.min_container_width.max(6);
    let top_rows = group_into_rows(&top_items, work.width, ui.gap, min_col_w);
    let bottom_rows = group_into_rows(&bottom_items, work.width, ui.gap, min_col_w);
    let has_middle = !left_items.is_empty() || !main_items.is_empty() || !right_items.is_empty();

    // Allocate vertical bands: Top rows, optional Middle band, Bottom rows.
    let mut band_specs: Vec<AxisSpec> = Vec::new();
    for row in &top_rows {
        band_specs.push(row_vertical_spec(row));
    }
    let middle_band_idx = if has_middle {
        let idx = band_specs.len();
        band_specs.push(AxisSpec {
            size: ContainerSize::Fill(1),
            min: None,
            max: None,
        });
        Some(idx)
    } else {
        None
    };
    for row in &bottom_rows {
        band_specs.push(row_vertical_spec(row));
    }

    let band_rects = allocate_vertical_bands(work, ui.gap, &band_specs);

    let mut band_cursor = 0usize;
    for row in &top_rows {
        if let Some(&row_rect) = band_rects.get(band_cursor) {
            place_row(row, row_rect, ui.gap, viewport, &mut out);
        }
        band_cursor += 1;
    }

    if let Some(mid_idx) = middle_band_idx {
        if let Some(&mid_rect) = band_rects.get(mid_idx)
            && mid_rect.width > 0
            && mid_rect.height > 0
        {
            place_middle_band(
                mid_rect,
                &left_items,
                &main_items,
                &right_items,
                ui.gap,
                min_col_w,
                viewport,
                &mut out,
            );
        }
        band_cursor += 1;
    }

    for row in &bottom_rows {
        if let Some(&row_rect) = band_rects.get(band_cursor) {
            place_row(row, row_rect, ui.gap, viewport, &mut out);
        }
        band_cursor += 1;
    }

    out
}

#[allow(clippy::too_many_arguments)]
fn place_middle_band(
    mid_rect: Rect,
    left_items: &[&ContainerConfig],
    main_items: &[&ContainerConfig],
    right_items: &[&ContainerConfig],
    gap: u16,
    min_col_w: u16,
    viewport: Rect,
    out: &mut ComputedLayout,
) {
    let mut regions: Vec<(u8, AxisSpec)> = Vec::new();
    if !left_items.is_empty() {
        regions.push((0, sidebar_width_spec(left_items)));
    }
    if !main_items.is_empty() {
        regions.push((
            1,
            AxisSpec {
                size: ContainerSize::Fill(1),
                min: None,
                max: None,
            },
        ));
    }
    if !right_items.is_empty() {
        regions.push((2, sidebar_width_spec(right_items)));
    }

    if regions.is_empty() {
        return;
    }

    let specs: Vec<AxisSpec> = regions.iter().map(|(_, s)| *s).collect();
    let col_rects = allocate_horizontal_bands(mid_rect, gap, &specs);

    for ((tag, _), rect) in regions.into_iter().zip(col_rects) {
        if rect.width == 0 || rect.height == 0 {
            continue;
        }
        match tag {
            0 => place_vertical_column(left_items, rect, gap, viewport, out),
            1 => {
                let rows = group_into_rows(main_items, rect.width, gap, min_col_w);
                let row_specs: Vec<AxisSpec> = rows.iter().map(|r| row_vertical_spec(r)).collect();
                let row_rects = allocate_vertical_bands(rect, gap, &row_specs);
                for (row, rrect) in rows.iter().zip(row_rects) {
                    place_row(row, rrect, gap, viewport, out);
                }
            }
            _ => place_vertical_column(right_items, rect, gap, viewport, out),
        }
    }
}

fn place_vertical_column(
    items: &[&ContainerConfig],
    area: Rect,
    gap: u16,
    viewport: Rect,
    out: &mut ComputedLayout,
) {
    if items.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let specs: Vec<AxisSpec> = items
        .iter()
        .map(|c| AxisSpec {
            size: normalize_fill_or_auto(c.height),
            min: c.min_height,
            max: c.max_height,
        })
        .collect();
    let rects = allocate_vertical_bands(area, gap, &specs);
    for (c, rect) in items.iter().zip(rects) {
        let width = resolve_cross_size(c.width, rect.width, c.min_width, c.max_width);
        let outer = Rect::new(rect.x, rect.y, width, rect.height)
            .intersection(area)
            .intersection(viewport);
        push_placed_container(c, outer, out);
    }
}

fn sidebar_width_spec(items: &[&ContainerConfig]) -> AxisSpec {
    for c in items {
        if !matches!(c.width, ContainerSize::Auto | ContainerSize::Fill(_)) {
            return AxisSpec {
                size: c.width,
                min: c.min_width,
                max: c.max_width,
            };
        }
    }
    AxisSpec {
        size: ContainerSize::Percent(24),
        min: Some(16),
        max: Some(40),
    }
}

/// Groups a linear queue of containers into rows from left to right.
/// Respects explicit `row` grouping, explicit percentage widths that fill 100%,
/// and responsive width wrapping when `Auto` items would become narrower than `min_col_w`.
fn group_into_rows<'a>(
    items: &[&'a ContainerConfig],
    avail_width: u16,
    gap: u16,
    min_col_w: u16,
) -> Vec<Vec<&'a ContainerConfig>> {
    if items.is_empty() {
        return Vec::new();
    }

    // Step 1: Split into segments whenever `c.row` explicitly changes.
    let mut segments: Vec<Vec<&'a ContainerConfig>> = Vec::new();
    let mut cur_seg: Vec<&'a ContainerConfig> = Vec::new();
    let mut prev_row: Option<u16> = items[0].row;

    for &item in items {
        if !cur_seg.is_empty() && (item.row.is_some() || prev_row.is_some()) && item.row != prev_row {
            segments.push(std::mem::take(&mut cur_seg));
        }
        prev_row = item.row;
        cur_seg.push(item);
    }
    if !cur_seg.is_empty() {
        segments.push(cur_seg);
    }

    // Step 2: Wrap each segment cleanly left-to-right based on percentage saturation or min width.
    let mut rows: Vec<Vec<&'a ContainerConfig>> = Vec::new();
    for seg in segments {
        let all_auto = seg
            .iter()
            .all(|c| matches!(c.width, ContainerSize::Auto));

        if all_auto {
            // Balanced wrapping for pure Auto queues: compute how many fit per row, then balance evenly.
            let item_min = seg
                .iter()
                .map(|c| c.min_width.unwrap_or(min_col_w).max(4))
                .max()
                .unwrap_or(min_col_w);
            let max_per_row = compute_max_items_per_row(avail_width, gap, item_min);
            let total = seg.len();
            if total <= max_per_row {
                rows.push(seg);
            } else {
                let row_count = total.div_ceil(max_per_row);
                let base = total / row_count;
                let extra = total % row_count;
                let mut offset = 0;
                for r in 0..row_count {
                    let take = base + usize::from(r < extra);
                    rows.push(seg[offset..offset + take].to_vec());
                    offset += take;
                }
            }
            continue;
        }

        let mut cur_row: Vec<&'a ContainerConfig> = Vec::new();
        let mut used_pct: u16 = 0;
        let mut row_closed = false;

        for item in seg {
            let wants_full_row = matches!(item.width, ContainerSize::Percent(100));
            let next_pct = match item.width {
                ContainerSize::Percent(p) => Some(p.min(100)),
                _ => None,
            };

            let would_overflow_pct = if let Some(p) = next_pct {
                !cur_row.is_empty() && used_pct.saturating_add(p) > 100
            } else {
                false
            };

            let would_crowd_width = if !cur_row.is_empty() && item.row.is_none() {
                let next_count = cur_row.len() + 1;
                let total_gap = gap.saturating_mul((next_count - 1) as u16);
                let needed_min: u16 = cur_row
                    .iter()
                    .chain(std::iter::once(&item))
                    .map(|c| c.min_width.unwrap_or(min_col_w).max(4))
                    .sum();
                avail_width > 0 && needed_min.saturating_add(total_gap) > avail_width && next_pct.is_none()
            } else {
                false
            };

            if !cur_row.is_empty()
                && (row_closed || wants_full_row || would_overflow_pct || would_crowd_width)
            {
                rows.push(std::mem::take(&mut cur_row));
                used_pct = 0;
                row_closed = false;
            }

            match item.width {
                ContainerSize::Percent(p) => {
                    used_pct = used_pct.saturating_add(p.min(100));
                    if used_pct >= 100 {
                        row_closed = true;
                    }
                }
                ContainerSize::Fill(_) if used_pct > 0 => {
                    // Fill following fixed/percent columns completes the row.
                    row_closed = true;
                }
                _ => {}
            }
            cur_row.push(item);
        }
        if !cur_row.is_empty() {
            rows.push(cur_row);
        }
    }

    rows
}

fn compute_max_items_per_row(avail_width: u16, gap: u16, min_item_w: u16) -> usize {
    if avail_width == 0 || min_item_w == 0 {
        return 1;
    }
    let mut k = 1usize;
    loop {
        let next = k + 1;
        let needed = (min_item_w as u32)
            .saturating_mul(next as u32)
            .saturating_add((gap as u32).saturating_mul((next - 1) as u32));
        if needed > avail_width as u32 {
            return k;
        }
        k = next;
    }
}

fn row_vertical_spec(row: &[&ContainerConfig]) -> AxisSpec {
    let mut fixed_max: Option<u16> = None;
    let mut pct_max: Option<u16> = None;
    let mut fill_weight: u16 = 0;
    let mut min_h: Option<u16> = None;
    let mut max_h: Option<u16> = None;

    for c in row {
        if let Some(mn) = c.min_height {
            min_h = Some(min_h.map_or(mn, |cur| cur.max(mn)));
        }
        if let Some(mx) = c.max_height {
            max_h = Some(max_h.map_or(mx, |cur| cur.min(mx)));
        }
        match c.height {
            ContainerSize::Fixed(n) => {
                fixed_max = Some(fixed_max.map_or(n, |cur| cur.max(n)));
            }
            ContainerSize::Percent(p) => {
                pct_max = Some(pct_max.map_or(p, |cur| cur.max(p)));
            }
            ContainerSize::Fill(w) => {
                fill_weight = fill_weight.max(w.max(1));
            }
            ContainerSize::Auto => {
                fill_weight = fill_weight.max(1);
            }
        }
    }

    let size = if let Some(f) = fixed_max {
        ContainerSize::Fixed(f)
    } else if let Some(p) = pct_max {
        ContainerSize::Percent(p)
    } else {
        ContainerSize::Fill(fill_weight.max(1))
    };

    AxisSpec {
        size,
        min: min_h,
        max: max_h,
    }
}

fn place_row(
    row: &[&ContainerConfig],
    row_rect: Rect,
    gap: u16,
    viewport: Rect,
    out: &mut ComputedLayout,
) {
    if row.is_empty() || row_rect.width == 0 || row_rect.height == 0 {
        return;
    }
    let specs: Vec<AxisSpec> = row
        .iter()
        .map(|c| AxisSpec {
            size: normalize_fill_or_auto(c.width),
            min: c.min_width,
            max: c.max_width,
        })
        .collect();

    let cell_rects = allocate_horizontal_bands(row_rect, gap, &specs);
    for (c, cell_rect) in row.iter().zip(cell_rects) {
        let h = resolve_cross_size(c.height, cell_rect.height, c.min_height, c.max_height);
        let outer = Rect::new(cell_rect.x, cell_rect.y, cell_rect.width, h)
            .intersection(row_rect)
            .intersection(viewport);
        push_placed_container(c, outer, out);
    }
}

fn push_placed_container(c: &ContainerConfig, outer: Rect, out: &mut ComputedLayout) {
    if outer.width == 0 || outer.height == 0 {
        return;
    }
    let has_border = c.border.enabled && c.border.style != Some(BorderStyle::None);
    let has_title = c
        .border
        .title
        .as_deref()
        .or(c.content.title.as_deref())
        .is_some_and(|t| !t.is_empty());
    let padding = if c.kind == crate::ui::container::ComponentKind::Canvas { 0 } else { c.padding };
    let inner = compute_inner_rect_with_title(outer, has_border, has_title, padding);
    out.containers.push(PlacedContainer {
        name: c.name.clone(),
        description: c.description.clone(),
        outer_rect: outer,
        inner_rect: inner,
        config: c.clone(),
    });
}

#[derive(Debug, Clone, Copy)]
struct AxisSpec {
    size: ContainerSize,
    min: Option<u16>,
    max: Option<u16>,
}

fn normalize_fill_or_auto(s: ContainerSize) -> ContainerSize {
    match s {
        ContainerSize::Auto => ContainerSize::Fill(1),
        other => other,
    }
}

fn allocate_vertical_bands(area: Rect, gap: u16, specs: &[AxisSpec]) -> Vec<Rect> {
    let (eff_gap, lengths) = allocate_axis(area.height, gap, specs);
    let mut rects = Vec::with_capacity(lengths.len());
    let mut cursor = area.y;
    for (idx, len) in lengths.into_iter().enumerate() {
        let rect = Rect::new(area.x, cursor, area.width, len).intersection(area);
        rects.push(rect);
        cursor = cursor.saturating_add(len);
        if idx + 1 < specs.len() {
            cursor = cursor.saturating_add(eff_gap);
        }
    }
    rects
}

fn allocate_horizontal_bands(area: Rect, gap: u16, specs: &[AxisSpec]) -> Vec<Rect> {
    let (eff_gap, lengths) = allocate_axis(area.width, gap, specs);
    let mut rects = Vec::with_capacity(lengths.len());
    let mut cursor = area.x;
    for (idx, len) in lengths.into_iter().enumerate() {
        let rect = Rect::new(cursor, area.y, len, area.height).intersection(area);
        rects.push(rect);
        cursor = cursor.saturating_add(len);
        if idx + 1 < specs.len() {
            cursor = cursor.saturating_add(eff_gap);
        }
    }
    rects
}

fn allocate_axis(total_avail: u16, gap: u16, specs: &[AxisSpec]) -> (u16, Vec<u16>) {
    if specs.is_empty() || total_avail == 0 {
        return (0, vec![0; specs.len()]);
    }
    let gap_count = specs.len().saturating_sub(1) as u16;
    let total_gap = gap.saturating_mul(gap_count).min(total_avail.saturating_sub(specs.len().min(total_avail as usize) as u16));
    let eff_gap = if gap_count > 0 {
        total_gap / gap_count
    } else {
        0
    };
    let usable = total_avail.saturating_sub(eff_gap.saturating_mul(gap_count));

    let mut sizes = vec![0u16; specs.len()];
    let mut remaining = usable;
    let mut fill_indices = Vec::new();
    let mut total_fill_weight: u32 = 0;

    // Pass 1: compute desired Fixed and Percent lengths
    let mut desired_fixed_sum: u32 = 0;
    let mut desired = vec![0u16; specs.len()];
    let percent_total: u32 = specs.iter().map(|s| match s.size { ContainerSize::Percent(p) => p.min(100) as u32, _ => 0 }).sum();
    let last_percent = specs.iter().rposition(|s| matches!(s.size, ContainerSize::Percent(_)));
    for (idx, spec) in specs.iter().enumerate() {
        match spec.size {
            ContainerSize::Fixed(n) => {
                let d = clamp_bounds(n, spec.min, spec.max);
                desired[idx] = d;
                desired_fixed_sum += d as u32;
            }
            ContainerSize::Percent(pct) => {
                let raw = if percent_total >= 100 && Some(idx) == last_percent {
                    usable.saturating_sub(desired_fixed_sum.min(u16::MAX as u32) as u16)
                } else {
                    ((usable as u32 * pct.min(100) as u32) / 100) as u16
                };
                let d = clamp_bounds(raw, spec.min, spec.max);
                desired[idx] = d;
                desired_fixed_sum += d as u32;
            }
            ContainerSize::Fill(_) | ContainerSize::Auto => {
                let weight = match spec.size {
                    ContainerSize::Fill(v) => v.max(1) as u32,
                    _ => 1,
                };
                total_fill_weight += weight;
                fill_indices.push((idx, weight));
            }
        }
    }

    // If Fixed+Percent items exceed usable space, scale them proportionally so later bands aren't starved to 0.
    if desired_fixed_sum > usable as u32 && desired_fixed_sum > 0 {
        for (idx, spec) in specs.iter().enumerate() {
            if matches!(spec.size, ContainerSize::Fixed(_) | ContainerSize::Percent(_)) {
                let scaled = ((desired[idx] as u32 * usable as u32) / desired_fixed_sum) as u16;
                let alloc = scaled.min(remaining);
                sizes[idx] = alloc;
                remaining = remaining.saturating_sub(alloc);
            }
        }
    } else {
        for (idx, spec) in specs.iter().enumerate() {
            if matches!(spec.size, ContainerSize::Fixed(_) | ContainerSize::Percent(_)) {
                let alloc = desired[idx].min(remaining);
                sizes[idx] = alloc;
                remaining = remaining.saturating_sub(alloc);
            }
        }
    }

    // Pass 2: reserve each fill band's minimum before sharing leftover space.
    // This keeps content minimums (such as Activity's charts) intact when the
    // terminal is just tall enough for the dashboard.
    if !fill_indices.is_empty() && remaining > 0 && total_fill_weight > 0 {
        let min_total: u32 = fill_indices
            .iter()
            .map(|(idx, _)| {
                let spec = specs[*idx];
                spec.min.unwrap_or(0).min(spec.max.unwrap_or(u16::MAX)) as u32
            })
            .sum();
        let enough_for_minimums = min_total <= remaining as u32;
        let extra_total = if enough_for_minimums {
            remaining.saturating_sub(min_total as u16)
        } else {
            0
        };
        let mut extra_remaining = extra_total;
        let mut fill_remaining = remaining;
        for (pos, &(idx, weight)) in fill_indices.iter().enumerate() {
            let is_last = pos + 1 == fill_indices.len();
            let spec = specs[idx];
            let minimum = spec.min.unwrap_or(0).min(spec.max.unwrap_or(u16::MAX));
            let raw = if enough_for_minimums {
                let extra = if is_last {
                    extra_remaining
                } else {
                    ((extra_total as u32 * weight) / total_fill_weight) as u16
                };
                extra_remaining = extra_remaining.saturating_sub(extra);
                minimum.saturating_add(extra)
            } else if min_total > 0 {
                ((fill_remaining as u32 * minimum as u32) / min_total) as u16
            } else if is_last {
                fill_remaining
            } else {
                ((remaining as u32 * weight) / total_fill_weight) as u16
            };
            let clamped = clamp_bounds(raw, spec.min, spec.max).min(fill_remaining);
            sizes[idx] = clamped;
            fill_remaining = fill_remaining.saturating_sub(clamped);
        }
    }

    (eff_gap, sizes)
}

fn resolve_cross_size(
    size: ContainerSize,
    avail: u16,
    min_v: Option<u16>,
    max_v: Option<u16>,
) -> u16 {
    let raw = match size {
        ContainerSize::Fixed(n) => n,
        ContainerSize::Percent(pct) => ((avail as u32 * pct.min(100) as u32) / 100) as u16,
        ContainerSize::Fill(_) | ContainerSize::Auto => avail,
    };
    clamp_bounds(raw, min_v, max_v).min(avail)
}

fn clamp_bounds(val: u16, min_v: Option<u16>, max_v: Option<u16>) -> u16 {
    let mut out = val;
    if let Some(min_bound) = min_v {
        out = out.max(min_bound);
    }
    if let Some(max_bound) = max_v {
        out = out.min(max_bound);
    }
    out
}

pub fn compute_inner_rect(outer: Rect, has_border: bool, padding: u16) -> Rect {
    compute_inner_rect_with_title(outer, has_border, false, padding)
}

pub fn compute_inner_rect_with_title(
    outer: Rect,
    has_border: bool,
    has_title: bool,
    padding: u16,
) -> Rect {
    let border_inset = if has_border && outer.width >= 2 && outer.height >= 2 {
        1
    } else {
        0
    };

    let after_border = Rect::new(
        outer.x.saturating_add(border_inset),
        outer.y.saturating_add(border_inset),
        outer.width.saturating_sub(border_inset * 2),
        outer.height.saturating_sub(border_inset * 2),
    );

    if after_border.width == 0 || after_border.height == 0 {
        return Rect::ZERO;
    }

    let pad_x = padding.min(after_border.width / 2);
    let pad_y = padding.min(after_border.height / 2);
    let top_pad = if border_inset == 0 && has_title {
        pad_y.max(1).min(after_border.height)
    } else {
        pad_y
    };

    Rect::new(
        after_border.x.saturating_add(pad_x),
        after_border.y.saturating_add(top_pad),
        after_border.width.saturating_sub(pad_x * 2),
        after_border
            .height
            .saturating_sub(top_pad.saturating_add(pad_y)),
    )
}
