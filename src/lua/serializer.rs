use crate::config::schema::BackgroundBaseType;
use crate::config::{Config, StartupTrigger};
use crate::ui::container::{ContainerConfig, ContainerDirection, ContainerNode, ContainerSize};
use std::fmt::Write;

fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn size_to_lua(sz: ContainerSize) -> String {
    match sz {
        ContainerSize::Fixed(n) => n.to_string(),
        ContainerSize::Percent(p) => format!("\"{p}%\""),
        ContainerSize::Fill(_) => "\"fill\"".to_string(),
    }
}

fn component_kind_str(kind: crate::ui::container::ComponentKind) -> &'static str {
    use crate::ui::container::ComponentKind::*;
    match kind {
        Header => "header",
        Greeting => "greeting",
        System => "system",
        Hardware => "hardware",
        Environment => "environment",
        Terminal => "terminal",
        Storage => "storage",
        Activity => "activity",
        Cpu => "cpu",
        Memory => "memory",
        Temperature => "temperature",
        Processes => "processes",
        Network => "network",
        Uptime => "uptime",
        Session => "session",
        Graph => "graph",
        AsciiLogo => "ascii_logo",
        Separator => "separator",
        Text => "text",
        Spinner => "spinner",
        Typewriter => "typewriter",
        Frames => "frames",
        Matrix => "matrix",
        Particles => "particles",
        AsciiObject => "ascii_object",
        Canvas => "canvas",
    }
}

fn write_container(out: &mut String, c: &ContainerConfig, indent: usize, is_root: bool) {
    let pad = " ".repeat(indent);
    if is_root {
        let _ = writeln!(out, "{pad}egotop.layout({{");
    } else {
        let _ = writeln!(out, "{pad}egotop.container({{");
    }
    let ip = " ".repeat(indent + 4);
    let _ = writeln!(out, "{ip}id = {},", lua_str(&c.id));
    if !c.visible {
        let _ = writeln!(out, "{ip}visible = false,");
    }
    let dir = match c.direction {
        ContainerDirection::Horizontal => "horizontal",
        ContainerDirection::Vertical => "vertical",
    };
    let _ = writeln!(out, "{ip}direction = \"{dir}\",");
    let _ = writeln!(out, "{ip}width = {},", size_to_lua(c.width));
    let _ = writeln!(out, "{ip}height = {},", size_to_lua(c.height));
    if let Some(v) = c.min_width {
        let _ = writeln!(out, "{ip}min_width = {v},");
    }
    if let Some(v) = c.max_width {
        let _ = writeln!(out, "{ip}max_width = {v},");
    }
    if let Some(v) = c.min_height {
        let _ = writeln!(out, "{ip}min_height = {v},");
    }
    if let Some(v) = c.max_height {
        let _ = writeln!(out, "{ip}max_height = {v},");
    }
    if c.padding > 0 {
        let _ = writeln!(out, "{ip}padding = {},", c.padding);
    }
    if c.gap > 0 {
        let _ = writeln!(out, "{ip}gap = {},", c.gap);
    }
    let _ = writeln!(out, "{ip}transparent = {},", c.transparent);
    if let Some(bg) = &c.background {
        let _ = writeln!(out, "{ip}background = {},", lua_str(bg));
    }
    if let Some(op) = c.opacity {
        let _ = writeln!(out, "{ip}opacity = {op:.2},");
    }
    if c.border.enabled || c.border.title.is_some() {
        let _ = writeln!(
            out,
            "{ip}border = {{ enabled = {}, style = \"{}\"{} }},",
            c.border.enabled,
            c.border.style.label(),
            c.border
                .title
                .as_deref()
                .map(|t| format!(", title = {}", lua_str(t)))
                .unwrap_or_default()
        );
    }
    if !c.children.is_empty() {
        let _ = writeln!(out, "{ip}children = {{");
        for child in &c.children {
            match child {
                ContainerNode::Container(sub) => write_container(out, sub, indent + 8, false),
                ContainerNode::Component(comp) => {
                    let cp = " ".repeat(indent + 8);
                    let cip = " ".repeat(indent + 12);
                    let _ = writeln!(out, "{cp}egotop.component({{");
                    let _ = writeln!(out, "{cip}id = {},", lua_str(&comp.id));
                    if !comp.visible {
                        let _ = writeln!(out, "{cip}visible = false,");
                    }
                    let cat = match comp.category {
                        crate::ui::container::ComponentCategory::Static => "static",
                        crate::ui::container::ComponentCategory::Dynamic => "dynamic",
                        crate::ui::container::ComponentCategory::Animated => "animated",
                    };
                    let kind = component_kind_str(comp.kind);
                    let _ = writeln!(out, "{cip}category = \"{cat}\",");
                    let _ = writeln!(out, "{cip}kind = \"{kind}\",");
                    let _ = writeln!(out, "{cip}width = {},", size_to_lua(comp.width));
                    let _ = writeln!(out, "{cip}height = {},", size_to_lua(comp.height));
                    if let Some(v) = comp.min_height {
                        let _ = writeln!(out, "{cip}min_height = {v},");
                    }
                    if comp.visual.padding > 0 {
                        let _ = writeln!(out, "{cip}padding = {},", comp.visual.padding);
                    }
                    if comp.visual.align != crate::config::Align::Left {
                        let al = match comp.visual.align {
                            crate::config::Align::Center => "center",
                            crate::config::Align::Right => "right",
                            crate::config::Align::Left => "left",
                        };
                        let _ = writeln!(out, "{cip}align = \"{al}\",");
                    }
                    if let Some(f) = &comp.visual.font {
                        let _ = writeln!(out, "{cip}font = {},", lua_str(f));
                    }
                    if let Some(gs) = comp.visual.graph_style {
                        let gs_s = match gs {
                            crate::config::GraphStyle::Bar => "bar",
                            crate::config::GraphStyle::Sparkline => "sparkline",
                        };
                        let _ = writeln!(out, "{cip}graph_style = \"{gs_s}\",");
                    }
                    if let Some(t) = &comp.content.text {
                        let _ = writeln!(out, "{cip}text = {},", lua_str(t));
                    }
                    if let Some(s) = &comp.content.script {
                        let _ = writeln!(out, "{cip}script = {},", lua_str(s));
                    }
                    if let Some(anim) = &comp.animation {
                        let fps = anim.fps.unwrap_or(12);
                        let st = anim
                            .style
                            .as_deref()
                            .map(|s| format!(", style = {}", lua_str(s)))
                            .unwrap_or_default();
                        let _ = writeln!(
                            out,
                            "{cip}animation = {{ fps = {fps}, speed = {:.2}{st} }},",
                            anim.speed
                        );
                    }
                    if let Some(bg) = &comp.visual.background {
                        let _ = writeln!(out, "{cip}background = {},", lua_str(bg));
                    }
                    if comp.visual.border.enabled || comp.visual.border.title.is_some() {
                        let _ = writeln!(
                            out,
                            "{cip}border = {{ enabled = {}, style = \"{}\"{} }},",
                            comp.visual.border.enabled,
                            comp.visual.border.style.label(),
                            comp.visual
                                .border
                                .title
                                .as_deref()
                                .map(|t| format!(", title = {}", lua_str(t)))
                                .unwrap_or_default()
                        );
                    }
                    let _ = writeln!(out, "{cp}}}),");
                }
            }
        }
        let _ = writeln!(out, "{ip}}},");
    }
    if is_root {
        let _ = writeln!(out, "{pad}}})");
    } else {
        let _ = writeln!(out, "{pad}}}),");
    }
}

/// Serializes `Config` into an idiomatic `egotop.lua` script that cleanly round-trips
/// through `load_lua_config_str`.
pub fn serialize_config_to_lua(cfg: &Config) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "-- Generated by egotop (:write)");
    let _ = writeln!(out, "egotop.config({{");
    let _ = writeln!(out, "    name = {},", lua_str(&cfg.general.name));
    let _ = writeln!(out, "    greeting = {},", lua_str(&cfg.general.greeting));
    if let Some(u) = &cfg.general.user_override {
        let _ = writeln!(out, "    user_override = {},", lua_str(u));
    }
    let _ = writeln!(out, "    message = {},", lua_str(&cfg.general.message));
    let _ = writeln!(out, "    refresh_rate_ms = {},", cfg.general.refresh_rate_ms);
    let _ = writeln!(out, "    animation_fps = {},", cfg.general.animation_fps);
    let _ = writeln!(out, "    time_format = {},", lua_str(&cfg.general.time_format));
    let _ = writeln!(out, "    date_format = {},", lua_str(&cfg.general.date_format));
    let _ = writeln!(out, "    hide_unavailable = {},", cfg.general.hide_unavailable);
    let _ = writeln!(
        out,
        "    view_mode = \"{}\",",
        cfg.general.view_mode.label().to_ascii_lowercase()
    );
    let _ = writeln!(out, "    default_page = {},", cfg.general.default_page);
    let _ = writeln!(
        out,
        "    startup_animation = {},",
        lua_str(&cfg.general.startup_animation)
    );
    let _ = writeln!(out, "    font = {},", lua_str(&cfg.typography.font));
    let _ = writeln!(
        out,
        "    ascii_fallback = {},",
        cfg.typography.ascii_fallback
    );
    let _ = writeln!(
        out,
        "    use_nerd_fonts = {},",
        cfg.typography.use_nerd_fonts
    );
    let _ = writeln!(out, "}})\n");

    if cfg.init.wait_for_input {
        let trig = match cfg.startup.trigger {
            StartupTrigger::Mouse => "mouse",
            StartupTrigger::Key => "key",
            StartupTrigger::Any => "any",
        };
        let _ = writeln!(out, "egotop.wait({{");
        let _ = writeln!(out, "    enabled = true,");
        let _ = writeln!(out, "    trigger = \"{trig}\",");
        let _ = writeln!(out, "    message = {},", lua_str(&cfg.init.message));
        let _ = writeln!(out, "    animation = {},", lua_str(&cfg.startup.animation));
        let _ = writeln!(
            out,
            "    cursor = {{ enabled = {}, character = {}, color = {}, blink = {}, blink_interval_ms = {} }},",
            cfg.init.cursor,
            lua_str(&cfg.startup.cursor.character),
            lua_str(&cfg.startup.cursor.color),
            cfg.startup.cursor.blink,
            cfg.startup.cursor.blink_interval
        );
        let _ = writeln!(out, "}})\n");
    }

    let _ = writeln!(out, "egotop.theme({{");
    let _ = writeln!(out, "    name = {},", lua_str(&cfg.theme.name));
    if !cfg.theme.background.is_empty() {
        let _ = writeln!(out, "    background = {},", lua_str(&cfg.theme.background));
    }
    if !cfg.theme.panel_background.is_empty() {
        let _ = writeln!(
            out,
            "    panel_background = {},",
            lua_str(&cfg.theme.panel_background)
        );
    }
    if !cfg.theme.primary.is_empty() {
        let _ = writeln!(out, "    primary = {},", lua_str(&cfg.theme.primary));
    }
    if !cfg.theme.secondary.is_empty() {
        let _ = writeln!(out, "    secondary = {},", lua_str(&cfg.theme.secondary));
    }
    if !cfg.theme.accent.is_empty() {
        let _ = writeln!(out, "    accent = {},", lua_str(&cfg.theme.accent));
    }
    if !cfg.theme.text.is_empty() {
        let _ = writeln!(out, "    text = {},", lua_str(&cfg.theme.text));
    }
    if !cfg.theme.muted.is_empty() {
        let _ = writeln!(out, "    muted = {},", lua_str(&cfg.theme.muted));
    }
    if !cfg.theme.border.is_empty() {
        let _ = writeln!(out, "    border = {},", lua_str(&cfg.theme.border));
    }
    if !cfg.theme.danger.is_empty() {
        let _ = writeln!(out, "    danger = {},", lua_str(&cfg.theme.danger));
    }
    let _ = writeln!(
        out,
        "    border_type = \"{}\",",
        cfg.theme.border_type.label()
    );
    let _ = writeln!(out, "}})\n");

    let base_kind = if cfg.background.transparent
        || cfg.background.base.kind == BackgroundBaseType::Transparent
    {
        "transparent"
    } else if cfg.background.base.kind == BackgroundBaseType::Gradient {
        "gradient"
    } else if cfg.background.base.kind == BackgroundBaseType::Solid {
        "solid"
    } else {
        "theme"
    };
    let fx_mod = cfg.background.effect.label().to_ascii_lowercase();
    let _ = writeln!(out, "egotop.background({{");
    let _ = writeln!(out, "    enabled = {},", cfg.background.enabled);
    if let Some(preset) = &cfg.background.preset {
        let _ = writeln!(out, "    preset = {},", lua_str(preset));
    }
    let _ = writeln!(out, "    base = {{");
    let _ = writeln!(out, "        type = \"{base_kind}\",");
    if let Some(col) = &cfg.background.base.color {
        let _ = writeln!(out, "        color = {},", lua_str(col));
    }
    let _ = writeln!(out, "        opacity = {:.2},", cfg.background.base.opacity);
    let _ = writeln!(out, "    }},");
    let _ = writeln!(out, "    effects = {{");
    if cfg.background.enabled && fx_mod != "none" {
        let _ = writeln!(
            out,
            "        require(\"effects.{fx_mod}\").setup({{ speed = {:.2}, density = {:.2} }}),",
            cfg.background.speed, cfg.background.density
        );
    }
    let _ = writeln!(out, "    }},");
    if !cfg.background.layers.is_empty() {
        let _ = writeln!(out, "    layers = {{");
        for l in &cfg.background.layers {
            let asset_str = l
                .asset
                .as_deref()
                .map(|a| format!(", asset = {}", lua_str(a)))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "        {{ effect = \"{}\", speed = {:.2}, density = {:.2}, z_index = {}{} }},",
                l.effect.label(),
                l.speed,
                l.density,
                l.z_index,
                asset_str
            );
        }
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "}})\n");

    let _ = writeln!(out, "egotop.footer({{");
    let _ = writeln!(out, "    enabled = {},", cfg.footer.enabled);
    let _ = writeln!(out, "    text = {},", lua_str(&cfg.footer.text));
    if let Some(ic) = &cfg.footer.icon {
        let _ = writeln!(out, "    icon = {},", lua_str(ic));
    }
    if let Some(col) = &cfg.footer.color {
        let _ = writeln!(out, "    color = {},", lua_str(col));
    }
    let f_align = match cfg.footer.align {
        crate::config::Align::Center => "center",
        crate::config::Align::Right => "right",
        crate::config::Align::Left => "left",
    };
    let _ = writeln!(out, "    align = \"{f_align}\",");
    if let Some(anim) = &cfg.footer.animation {
        let _ = writeln!(out, "    animation = {},", lua_str(anim));
    }
    let _ = writeln!(out, "    fps = {},", cfg.footer.fps);
    let _ = writeln!(out, "}})\n");

    if !cfg.container_styles.is_empty() {
        let _ = writeln!(out, "egotop.container_styles({{");
        for (id, st) in &cfg.container_styles {
            let _ = writeln!(out, "    [{}] = {{", lua_str(id));
            match &st.background {
                crate::ui::container::BackgroundFill::Inherit => {
                    let _ = writeln!(out, "        background = \"inherit\",");
                }
                crate::ui::container::BackgroundFill::Transparent => {
                    let _ = writeln!(out, "        background = \"transparent\",");
                }
                crate::ui::container::BackgroundFill::Solid(c) => {
                    let _ = writeln!(out, "        background = {},", lua_str(c));
                }
            }
            if let Some(fg) = &st.foreground {
                let _ = writeln!(out, "        foreground = {},", lua_str(fg));
            }
            if let Some(bc) = &st.border_color {
                let _ = writeln!(out, "        border_color = {},", lua_str(bc));
            }
            if let Some(bs) = st.border_style {
                let _ = writeln!(out, "        border_style = \"{}\",", bs.label());
            }
            if let Some(grad) = &st.gradient {
                let dir = match grad.direction {
                    crate::ui::container::GradientDirection::Horizontal => "horizontal",
                    crate::ui::container::GradientDirection::Vertical => "vertical",
                };
                let cols: Vec<String> = grad.colors.iter().map(|c| lua_str(c)).collect();
                let _ = writeln!(
                    out,
                    "        gradient = {{ direction = \"{dir}\", colors = {{ {} }} }},",
                    cols.join(", ")
                );
            }
            if let Some(op) = st.opacity {
                let _ = writeln!(out, "        opacity = {op:.2},");
            }
            let ef = match st.effects {
                crate::ui::container::EffectPolicy::Allow => "allow",
                crate::ui::container::EffectPolicy::Isolated => "isolated",
                crate::ui::container::EffectPolicy::Exclude => "exclude",
            };
            let _ = writeln!(out, "        effects = \"{ef}\",");
            let _ = writeln!(out, "    }},");
        }
        let _ = writeln!(out, "}})\n");
    }

    write_container(&mut out, &cfg.ui.root, 0, true);
    out
}
