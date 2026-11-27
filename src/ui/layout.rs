use crate::config::{BorderStyle, Config, ViewMode};
use crate::ui::container::{
    ComponentConfig, ContainerBorderConfig, ContainerConfig, ContainerDirection, ContainerNode,
    ContainerSize,
};
use ratatui::layout::Rect;

#[derive(Debug, Clone)]
pub struct PlacedContainer {
    pub id: String,
    pub outer_rect: Rect,
    pub inner_rect: Rect,
    pub border: ContainerBorderConfig,
    pub background: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlacedComponent {
    pub id: String,
    pub outer_rect: Rect,
    pub inner_rect: Rect,
    pub config: ComponentConfig,
}

#[derive(Debug, Clone, Default)]
pub struct ComputedLayout {
    pub area: Rect,
    pub containers: Vec<PlacedContainer>,
    pub components: Vec<PlacedComponent>,
}

impl ComputedLayout {
    pub fn find_component(&self, id: &str) -> Option<&PlacedComponent> {
        self.components.iter().find(|c| c.id == id)
    }

    pub fn find_container(&self, id: &str) -> Option<&PlacedContainer> {
        self.containers.iter().find(|c| c.id == id)
    }
}

pub fn compute_dashboard_layout(
    screen: Rect,
    cfg: &Config,
    view_mode: ViewMode,
) -> ComputedLayout {
    let no_borders = cfg.theme.border_type == BorderStyle::None;
    match view_mode {
        ViewMode::Dashboard => {
            if no_borders {
                let mut root = cfg.ui.root.clone();
                disable_container_borders(&mut root);
                compute_container_layout(screen, &root)
            } else {
                compute_container_layout(screen, &cfg.ui.root)
            }
        }
        ViewMode::Compact => {
            let mut root = cfg.ui.root.clone();
            apply_compact_overrides(&mut root);
            if no_borders {
                disable_container_borders(&mut root);
            }
            compute_container_layout(screen, &root)
        }
        ViewMode::Minimal => {
            let mut minimal_root = ContainerConfig {
                id: "minimal_root".to_string(),
                visible: true,
                direction: ContainerDirection::Vertical,
                width: ContainerSize::Percent(100),
                height: ContainerSize::Percent(100),
                padding: 1,
                gap: 1,
                ..ContainerConfig::default()
            };
            if let Some(greeting) = find_first_component_by_id(&cfg.ui.root, "greeting") {
                minimal_root.children.push(ContainerNode::Component(greeting));
            }
            if let Some(spinner) = find_first_component_by_id(&cfg.ui.root, "status_spinner") {
                minimal_root.children.push(ContainerNode::Component(spinner));
            }
            if no_borders {
                disable_container_borders(&mut minimal_root);
            }
            compute_container_layout(screen, &minimal_root)
        }
    }
}

fn disable_container_borders(container: &mut ContainerConfig) {
    container.border.enabled = false;
    for child in &mut container.children {
        match child {
            ContainerNode::Container(c) => disable_container_borders(c),
            ContainerNode::Component(comp) => {
                comp.visual.border.enabled = false;
            }
        }
    }
}

fn apply_compact_overrides(container: &mut ContainerConfig) {
    container.padding = container.padding.min(1);
    container.gap = 0;
    for child in &mut container.children {
        match child {
            ContainerNode::Container(c) => apply_compact_overrides(c),
            ContainerNode::Component(comp) => {
                comp.visual.padding = 0;
            }
        }
    }
}

fn find_first_component_by_id(container: &ContainerConfig, id: &str) -> Option<ComponentConfig> {
    for child in &container.children {
        match child {
            ContainerNode::Component(c) if c.id == id && c.visible => return Some(c.clone()),
            ContainerNode::Container(sub) if sub.visible => {
                if let Some(found) = find_first_component_by_id(sub, id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// Computes deterministic, non-overlapping `Rect` geometry for the entire container tree.
pub fn compute_container_layout(viewport: Rect, root: &ContainerConfig) -> ComputedLayout {
    let mut out = ComputedLayout {
        area: viewport,
        containers: Vec::new(),
        components: Vec::new(),
    };

    if !root.visible || viewport.width == 0 || viewport.height == 0 {
        return out;
    }

    let root_w = resolve_axis_size(
        root.width,
        viewport.width,
        root.min_width,
        root.max_width,
    )
    .min(viewport.width);
    let root_h = resolve_axis_size(
        root.height,
        viewport.height,
        root.min_height,
        root.max_height,
    )
    .min(viewport.height);

    let root_rect = Rect::new(viewport.x, viewport.y, root_w, root_h).intersection(viewport);
    layout_container_node(root, root_rect, &mut out);
    out
}

fn layout_container_node(container: &ContainerConfig, outer_rect: Rect, out: &mut ComputedLayout) {
    if !container.visible || outer_rect.width == 0 || outer_rect.height == 0 {
        return;
    }

    let inner_rect = compute_inner_rect(
        outer_rect,
        container.border.enabled && container.border.style != BorderStyle::None,
        container.padding,
    );

    out.containers.push(PlacedContainer {
        id: container.id.clone(),
        outer_rect,
        inner_rect,
        border: container.border.clone(),
        background: container.background.clone(),
    });

    if inner_rect.width == 0 || inner_rect.height == 0 {
        return;
    }

    let visible_children: Vec<&ContainerNode> =
        container.children.iter().filter(|n| n.visible()).collect();
    if visible_children.is_empty() {
        return;
    }

    let main_avail = match container.direction {
        ContainerDirection::Horizontal => inner_rect.width,
        ContainerDirection::Vertical => inner_rect.height,
    };
    let cross_avail = match container.direction {
        ContainerDirection::Horizontal => inner_rect.height,
        ContainerDirection::Vertical => inner_rect.width,
    };

    let child_specs: Vec<ChildAxisSpec> = visible_children
        .iter()
        .map(|node| extract_child_spec(node, container.direction))
        .collect();

    let gap_count = visible_children.len().saturating_sub(1) as u16;
    let total_gap = container.gap.saturating_mul(gap_count).min(main_avail);
    let effective_gap = if gap_count > 0 {
        total_gap / gap_count
    } else {
        0
    };
    let usable_main = main_avail.saturating_sub(effective_gap.saturating_mul(gap_count));

    let main_sizes = allocate_main_axis(usable_main, &child_specs);

    let mut cursor = match container.direction {
        ContainerDirection::Horizontal => inner_rect.x,
        ContainerDirection::Vertical => inner_rect.y,
    };

    for (idx, (node, main_len)) in visible_children.iter().zip(main_sizes).enumerate() {
        let cross_len = resolve_axis_size(
            child_specs[idx].cross_size,
            cross_avail,
            child_specs[idx].cross_min,
            child_specs[idx].cross_max,
        )
        .min(cross_avail);

        let raw_rect = match container.direction {
            ContainerDirection::Horizontal => {
                Rect::new(cursor, inner_rect.y, main_len, cross_len)
            }
            ContainerDirection::Vertical => {
                Rect::new(inner_rect.x, cursor, cross_len, main_len)
            }
        };

        let child_rect = raw_rect.intersection(inner_rect);
        cursor = cursor
            .saturating_add(main_len)
            .saturating_add(if idx + 1 < visible_children.len() {
                effective_gap
            } else {
                0
            });

        if child_rect.width == 0 || child_rect.height == 0 {
            continue;
        }

        match node {
            ContainerNode::Container(sub) => {
                layout_container_node(sub, child_rect, out);
            }
            ContainerNode::Component(comp) => {
                let comp_inner = compute_inner_rect(
                    child_rect,
                    comp.visual.border.enabled && comp.visual.border.style != BorderStyle::None,
                    comp.visual.padding,
                );
                out.components.push(PlacedComponent {
                    id: comp.id.clone(),
                    outer_rect: child_rect,
                    inner_rect: comp_inner,
                    config: comp.clone(),
                });
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChildAxisSpec {
    main_size: ContainerSize,
    main_min: Option<u16>,
    main_max: Option<u16>,
    cross_size: ContainerSize,
    cross_min: Option<u16>,
    cross_max: Option<u16>,
}

fn extract_child_spec(node: &ContainerNode, parent_dir: ContainerDirection) -> ChildAxisSpec {
    let (w, h, min_w, max_w, min_h, max_h) = match node {
        ContainerNode::Container(c) => (
            c.width,
            c.height,
            c.min_width,
            c.max_width,
            c.min_height,
            c.max_height,
        ),
        ContainerNode::Component(c) => (
            c.width,
            c.height,
            c.min_width,
            c.max_width,
            c.min_height,
            c.max_height,
        ),
    };

    match parent_dir {
        ContainerDirection::Horizontal => ChildAxisSpec {
            main_size: w,
            main_min: min_w,
            main_max: max_w,
            cross_size: h,
            cross_min: min_h,
            cross_max: max_h,
        },
        ContainerDirection::Vertical => ChildAxisSpec {
            main_size: h,
            main_min: min_h,
            main_max: max_h,
            cross_size: w,
            cross_min: min_w,
            cross_max: max_w,
        },
    }
}

fn allocate_main_axis(usable_main: u16, specs: &[ChildAxisSpec]) -> Vec<u16> {
    let mut sizes = vec![0u16; specs.len()];
    let mut remaining = usable_main;
    let mut fill_indices = Vec::new();
    let mut total_fill_weight: u32 = 0;

    // Pass 1: Fixed and Percent children
    for (idx, spec) in specs.iter().enumerate() {
        match spec.main_size {
            ContainerSize::Fixed(n) => {
                let clamped = clamp_bounds(n, spec.main_min, spec.main_max).min(remaining);
                sizes[idx] = clamped;
                remaining = remaining.saturating_sub(clamped);
            }
            ContainerSize::Percent(pct) => {
                let raw = ((usable_main as u32 * pct.min(100) as u32) / 100) as u16;
                let clamped = clamp_bounds(raw, spec.main_min, spec.main_max).min(remaining);
                sizes[idx] = clamped;
                remaining = remaining.saturating_sub(clamped);
            }
            ContainerSize::Fill(weight) => {
                let w = weight.max(1) as u32;
                total_fill_weight = total_fill_weight.saturating_add(w);
                fill_indices.push((idx, w));
            }
        }
    }

    // Pass 2: Distribute remaining space among Fill children
    if !fill_indices.is_empty() && remaining > 0 && total_fill_weight > 0 {
        let mut fill_remaining = remaining;
        for (pos, &(idx, weight)) in fill_indices.iter().enumerate() {
            let is_last = pos + 1 == fill_indices.len();
            let raw = if is_last {
                fill_remaining
            } else {
                ((remaining as u32 * weight) / total_fill_weight) as u16
            };
            let clamped = clamp_bounds(raw, specs[idx].main_min, specs[idx].main_max)
                .min(fill_remaining);
            sizes[idx] = clamped;
            fill_remaining = fill_remaining.saturating_sub(clamped);
        }
    }

    sizes
}

fn resolve_axis_size(
    size: ContainerSize,
    avail: u16,
    min_v: Option<u16>,
    max_v: Option<u16>,
) -> u16 {
    let raw = match size {
        ContainerSize::Fixed(n) => n,
        ContainerSize::Percent(pct) => ((avail as u32 * pct.min(100) as u32) / 100) as u16,
        ContainerSize::Fill(_) => avail,
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

    Rect::new(
        after_border.x.saturating_add(pad_x),
        after_border.y.saturating_add(pad_y),
        after_border.width.saturating_sub(pad_x * 2),
        after_border.height.saturating_sub(pad_y * 2),
    )
}
