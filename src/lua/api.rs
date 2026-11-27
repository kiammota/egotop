use crate::config::schema::BackgroundBaseType;
use crate::config::{Align, BorderStyle, GraphStyle, StartupTrigger, ThemeConfig, ViewMode};
use crate::lua::{LuaConfigBundle, LuaEffectSpec};
use crate::ui::borders::parse_border_style;
use crate::ui::container::{
    ComponentCategory, ComponentConfig, ComponentKind, ContainerBorderConfig, ContainerConfig,
    ContainerDirection, ContainerGradient, ContainerNode, ContainerSize, EffectPolicy,
    GradientDirection,
};
use mlua::{Function, Lua, Table, Value};
use std::sync::{Arc, Mutex};

pub fn register_egotop_api(lua: &Lua, state: Arc<Mutex<LuaConfigBundle>>) -> mlua::Result<()> {
    let egotop = lua.create_table()?;

    // egotop.config({ ... })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "config",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                apply_general_config(&tbl, &mut bundle);
                Ok(())
            })?,
        )?;
    }

    // egotop.wait({ ... })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "wait",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                apply_wait_config(&tbl, &mut bundle);
                Ok(())
            })?,
        )?;
    }

    // egotop.theme({ theme = module.setup, ...overrides })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "theme",
            lua.create_function(move |lua, tbl: Table| {
                let resolved = resolve_theme_table(lua, &tbl)?;
                let mut bundle = st.lock().unwrap();
                apply_theme_table(&resolved, &mut bundle.theme_config);
                bundle.config.theme = bundle.theme_config.clone();
                Ok(())
            })?,
        )?;
    }

    // egotop.background({ base = { ... }, effects = { ... } })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "background",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                apply_background_config(&tbl, &mut bundle);
                Ok(())
            })?,
        )?;
    }

    // egotop.footer({ enabled = ..., text = ..., icon = ..., color = ... })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "footer",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                apply_footer_config(&tbl, &mut bundle);
                Ok(())
            })?,
        )?;
    }

    // egotop.layout({ id = "root", ... })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "layout",
            lua.create_function(move |_, tbl: Table| {
                let mut root = parse_container_table(&tbl);
                root.id = tbl.get::<String>("id").unwrap_or_else(|_| "root".to_string());
                let mut bundle = st.lock().unwrap();
                bundle.config.ui.root = root;
                Ok(())
            })?,
        )?;
    }

    // egotop.container_styles({ [id] = { background = ..., foreground = ..., ... } })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "container_styles",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                for pair in tbl.pairs::<String, Table>().flatten() {
                    let (id, stbl) = pair;
                    let cs = parse_container_style_table(&stbl);
                    bundle.config.container_styles.insert(id, cs);
                }
                Ok(())
            })?,
        )?;
    }

    // Helper constructors: egotop.container(tbl) & egotop.component(tbl)
    egotop.set(
        "container",
        lua.create_function(|_, tbl: Table| {
            tbl.set("node", "container")?;
            Ok(tbl)
        })?,
    )?;
    egotop.set(
        "component",
        lua.create_function(|_, tbl: Table| {
            tbl.set("node", "component")?;
            Ok(tbl)
        })?,
    )?;

    lua.globals().set("egotop", egotop)?;
    Ok(())
}

fn apply_general_config(tbl: &Table, bundle: &mut LuaConfigBundle) {
    let g = &mut bundle.config.general;
    if let Ok(v) = tbl.get::<String>("name") {
        g.name = v;
    }
    if let Ok(v) = tbl.get::<String>("greeting") {
        g.greeting = v;
    }
    if let Ok(v) = tbl.get::<Option<String>>("user_override") {
        g.user_override = v;
    }
    if let Ok(v) = tbl.get::<String>("message") {
        g.message = v;
    }
    if let Ok(v) = tbl.get::<u64>("refresh_rate_ms") {
        g.refresh_rate_ms = v;
    } else if let Ok(v) = tbl.get::<u64>("refresh_rate") {
        g.refresh_rate_ms = v;
    }
    if let Ok(v) = tbl.get::<u32>("animation_fps") {
        g.animation_fps = v;
    } else if let Ok(v) = tbl.get::<u32>("fps") {
        g.animation_fps = v;
    }
    if let Ok(v) = tbl.get::<String>("time_format") {
        g.time_format = v;
    }
    if let Ok(v) = tbl.get::<String>("date_format") {
        g.date_format = v;
    }
    if let Some(v) = tbl.get::<Option<bool>>("hide_unavailable").ok().flatten() {
        g.hide_unavailable = v;
    }
    if let Ok(v) = tbl.get::<u8>("default_page") {
        g.default_page = v.clamp(1, 3);
    }
    if let Ok(v) = tbl.get::<String>("view_mode") {
        g.view_mode = match v.trim().to_ascii_lowercase().as_str() {
            "compact" => ViewMode::Compact,
            "minimal" => ViewMode::Minimal,
            _ => ViewMode::Dashboard,
        };
    }
    if let Ok(v) = tbl.get::<String>("startup_animation") {
        g.startup_animation = v;
    }
    if let Ok(v) = tbl.get::<u64>("startup_duration_ms") {
        g.startup_duration_ms = v;
    }
    if let Ok(v) = tbl.get::<String>("font") {
        bundle.config.typography.font = v;
    }
    if let Some(v) = tbl.get::<Option<bool>>("ascii_fallback").ok().flatten() {
        bundle.config.typography.ascii_fallback = v;
    }
    if let Some(v) = tbl.get::<Option<bool>>("use_nerd_fonts").ok().flatten() {
        bundle.config.typography.use_nerd_fonts = v;
    }
    if let Ok(v) = tbl.get::<f32>("marquee_speed") {
        bundle.config.typography.marquee_speed = v;
    }
}

fn apply_wait_config(tbl: &Table, bundle: &mut LuaConfigBundle) {
    let enabled = tbl.get::<Option<bool>>("enabled").ok().flatten().unwrap_or(true);
    bundle.config.init.wait_for_input = enabled;
    if let Ok(msg) = tbl.get::<String>("message") {
        bundle.config.init.message = msg;
    }
    if let Ok(trig) = tbl.get::<String>("trigger") {
        bundle.config.startup.trigger = match trig.trim().to_ascii_lowercase().as_str() {
            "mouse" => StartupTrigger::Mouse,
            "key" | "keyboard" => StartupTrigger::Key,
            _ => StartupTrigger::Any,
        };
    }
    if let Ok(anim) = tbl.get::<String>("animation") {
        bundle.config.startup.animation = anim.clone();
        bundle.wait_animation = Some(anim);
    }
    if let Ok(cursor_tbl) = tbl.get::<Table>("cursor") {
        bundle.config.init.cursor = cursor_tbl
            .get::<Option<bool>>("enabled")
            .ok()
            .flatten()
            .unwrap_or(true);
        if let Ok(ch) = cursor_tbl.get::<String>("character") {
            bundle.config.startup.cursor.character = ch;
        }
        if let Ok(col) = cursor_tbl.get::<String>("color") {
            bundle.config.startup.cursor.color = col;
        }
        if let Some(blink) = cursor_tbl.get::<Option<bool>>("blink").ok().flatten() {
            bundle.config.startup.cursor.blink = blink;
        }
        if let Ok(ms) = cursor_tbl.get::<u64>("blink_interval_ms") {
            bundle.config.startup.cursor.blink_interval = ms.max(50);
        }
    }
}

fn resolve_theme_table(lua: &Lua, tbl: &Table) -> mlua::Result<Table> {
    let out = lua.create_table()?;
    let base_val: Value = tbl.get("theme").unwrap_or(Value::Nil);
    let base_tbl: Option<Table> = match base_val {
        Value::Function(f) => Some(f.call::<Table>(())?),
        Value::Table(t) => {
            if let Ok(Value::Function(setup_fn)) = t.get::<Value>("setup") {
                Some(setup_fn.call::<Table>(())?)
            } else {
                Some(t)
            }
        }
        _ => None,
    };

    if let Some(bt) = base_tbl {
        for pair in bt.pairs::<Value, Value>() {
            let (k, v) = pair?;
            out.set(k, v)?;
        }
    }

    for pair in tbl.pairs::<Value, Value>() {
        let (k, v) = pair?;
        if let Value::String(ref s) = k
            && matches!(s.to_str().as_deref(), Ok("theme"))
        {
            continue;
        }
        out.set(k, v)?;
    }
    Ok(out)
}

pub fn apply_theme_table(tbl: &Table, cfg: &mut ThemeConfig) {
    if let Ok(v) = tbl.get::<String>("name") {
        cfg.name = v;
    }
    if let Some(v) = tbl.get::<Option<bool>>("background_transparent").ok().flatten() {
        cfg.background_transparent = v;
    }
    if let Ok(v) = tbl.get::<String>("background") {
        cfg.background = v;
    }
    if let Ok(v) = tbl.get::<String>("panel_background") {
        cfg.panel_background = v;
    }
    if let Ok(v) = tbl.get::<String>("primary") {
        cfg.primary = v;
    }
    if let Ok(v) = tbl.get::<String>("secondary") {
        cfg.secondary = v;
    }
    if let Ok(v) = tbl.get::<String>("accent") {
        cfg.accent = v;
    }
    if let Ok(v) = tbl.get::<String>("text") {
        cfg.text = v;
    }
    if let Ok(v) = tbl.get::<String>("muted") {
        cfg.muted = v;
    }
    if let Ok(v) = tbl.get::<String>("border") {
        cfg.border = v;
    }
    if let Ok(v) = tbl.get::<String>("danger") {
        cfg.danger = v;
    }
    if let Ok(v) = tbl.get::<String>("border_type")
        && let Ok(parsed) = parse_border_style(&v)
    {
        cfg.border_type = parsed;
    }
    if let Ok(btbl) = tbl.get::<Table>("borders") {
        if let Ok(v) = btbl.get::<String>("top_left") {
            cfg.borders.top_left = Some(v);
        }
        if let Ok(v) = btbl.get::<String>("top_right") {
            cfg.borders.top_right = Some(v);
        }
        if let Ok(v) = btbl.get::<String>("bottom_left") {
            cfg.borders.bottom_left = Some(v);
        }
        if let Ok(v) = btbl.get::<String>("bottom_right") {
            cfg.borders.bottom_right = Some(v);
        }
        if let Ok(v) = btbl.get::<String>("horizontal") {
            cfg.borders.horizontal = Some(v);
        }
        if let Ok(v) = btbl.get::<String>("vertical") {
            cfg.borders.vertical = Some(v);
        }
    }
}

fn apply_background_config(tbl: &Table, bundle: &mut LuaConfigBundle) {
    let bg = &mut bundle.config.background;
    if let Ok(base) = tbl.get::<Table>("base") {
        if let Ok(kind_str) = base.get::<String>("type") {
            match kind_str.trim().to_ascii_lowercase().as_str() {
                "transparent" => {
                    bg.base.kind = BackgroundBaseType::Transparent;
                    bg.transparent = true;
                }
                "gradient" => {
                    bg.base.kind = BackgroundBaseType::Gradient;
                    bg.transparent = false;
                }
                "solid" => {
                    bg.base.kind = BackgroundBaseType::Solid;
                    bg.transparent = false;
                }
                _ => {}
            }
        }
        if let Ok(color) = base.get::<String>("color") {
            if color.eq_ignore_ascii_case("transparent") {
                bg.transparent = true;
                bg.base.kind = BackgroundBaseType::Transparent;
            } else {
                bg.base.color = Some(color);
            }
        }
        if let Ok(op) = base.get::<f32>("opacity") {
            bg.base.opacity = op.clamp(0.0, 1.0);
        }
        if let Ok(grad_tbl) = base.get::<Table>("gradient") {
            bg.base.gradient = parse_gradient_table(&grad_tbl);
        }
    }

    if let Ok(fx_tbl) = tbl.get::<Table>("effects") {
        bundle.effect_specs.clear();
        for pair in fx_tbl.sequence_values::<Table>() {
            if let Ok(ft) = pair
                && let Some(spec) = LuaEffectSpec::from_lua_table(&ft)
            {
                bundle.effect_specs.push(spec);
            }
        }
        if let Some(first) = bundle.effect_specs.first() {
            bg.enabled = !first.name.eq_ignore_ascii_case("none");
            bg.speed = first.speed;
            bg.density = first.density;
            bg.effect = match first.name.to_ascii_lowercase().as_str() {
                "matrix" | "matrix_rain" => crate::config::EffectKind::Matrix,
                "water" | "waves" => crate::config::EffectKind::Water,
                "particles" => crate::config::EffectKind::Particles,
                "scanlines" => crate::config::EffectKind::Scanlines,
                _ => crate::config::EffectKind::None,
            };
        } else {
            bg.enabled = false;
            bg.effect = crate::config::EffectKind::None;
        }
    }
    if let Ok(layers_tbl) = tbl.get::<Table>("layers") {
        bg.layers.clear();
        for lt in layers_tbl.sequence_values::<Table>().flatten() {
            if let Ok(ef) = lt.get::<String>("effect")
                && let Some(kind) = crate::animation::BackgroundLayerKind::parse(&ef)
            {
                let mut spec = crate::animation::BackgroundLayerSpec {
                    effect: kind,
                    ..Default::default()
                };
                if let Ok(sp) = lt.get::<f32>("speed") {
                    spec.speed = sp;
                }
                if let Ok(den) = lt.get::<f32>("density") {
                    spec.density = den;
                }
                if let Ok(zi) = lt.get::<i32>("z_index") {
                    spec.z_index = zi;
                }
                if let Ok(asset) = lt.get::<Option<String>>("asset") {
                    spec.asset = asset;
                }
                bg.layers.push(spec);
            }
        }
    }
    if let Some(en) = tbl.get::<Option<bool>>("enabled").ok().flatten() {
        bg.enabled = en;
    }
    if let Ok(preset) = tbl.get::<Option<String>>("preset") {
        bg.preset = preset;
    }
}

fn apply_footer_config(tbl: &Table, bundle: &mut LuaConfigBundle) {
    let f = &mut bundle.config.footer;
    if let Some(v) = tbl.get::<Option<bool>>("enabled").ok().flatten() {
        f.enabled = v;
    }
    if let Ok(v) = tbl.get::<String>("text") {
        f.text = v;
    }
    if let Ok(v) = tbl.get::<Option<String>>("icon") {
        f.icon = v;
    }
    if let Ok(v) = tbl.get::<Option<String>>("color") {
        f.color = v;
    }
    if let Ok(v) = tbl.get::<String>("align") {
        f.align = parse_align(&v);
    }
    if let Ok(v) = tbl.get::<Option<String>>("animation") {
        f.animation = v;
    }
    if let Ok(v) = tbl.get::<u32>("fps") {
        f.fps = v.clamp(1, 60);
    }
}

fn parse_size(val: Value) -> ContainerSize {
    match val {
        Value::Integer(n) => ContainerSize::Fixed(n.max(0) as u16),
        Value::Number(n) => ContainerSize::Fixed(n.max(0.0) as u16),
        Value::String(s) => {
            let txt = s.to_str().map(|x| x.trim().to_ascii_lowercase()).unwrap_or_default();
            if txt == "fill" {
                ContainerSize::Fill(1)
            } else if let Some(pct) = txt.strip_suffix('%').and_then(|n| n.trim().parse::<u16>().ok()) {
                ContainerSize::Percent(pct.min(100))
            } else if let Ok(px) = txt.parse::<u16>() {
                ContainerSize::Fixed(px)
            } else {
                ContainerSize::Fill(1)
            }
        }
        _ => ContainerSize::Fill(1),
    }
}

fn parse_align(raw: &str) -> Align {
    match raw.trim().to_ascii_lowercase().as_str() {
        "center" => Align::Center,
        "right" => Align::Right,
        _ => Align::Left,
    }
}

fn parse_gradient_table(tbl: &Table) -> Option<ContainerGradient> {
    let direction = match tbl
        .get::<String>("direction")
        .unwrap_or_else(|_| "horizontal".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "vertical" => GradientDirection::Vertical,
        _ => GradientDirection::Horizontal,
    };
    let mut colors = Vec::new();
    if let Ok(list) = tbl.get::<Table>("colors") {
        for c in list.sequence_values::<String>().flatten() {
            colors.push(c);
        }
    }
    if colors.is_empty()
        && let (Ok(from), Ok(to)) = (tbl.get::<String>("from"), tbl.get::<String>("to"))
    {
        colors.push(from);
        colors.push(to);
    }
    if colors.len() >= 2 {
        Some(ContainerGradient { direction, colors })
    } else {
        None
    }
}

fn parse_border_table(tbl: &Table) -> ContainerBorderConfig {
    let enabled = tbl
        .get::<Option<bool>>("enabled")
        .ok()
        .flatten()
        .unwrap_or(true);
    let style = tbl
        .get::<String>("style")
        .ok()
        .and_then(|s| parse_border_style(&s).ok())
        .unwrap_or(BorderStyle::Rounded);
    let color = tbl.get::<Option<String>>("color").unwrap_or(None);
    let title = tbl.get::<Option<String>>("title").unwrap_or(None);
    ContainerBorderConfig {
        enabled: enabled && style != BorderStyle::None,
        style,
        color,
        title,
    }
}

fn parse_container_style_table(tbl: &Table) -> crate::ui::container::ContainerStyle {
    let mut style = crate::ui::container::ContainerStyle::default();
    if let Ok(bg) = tbl.get::<String>("background") {
        let low = bg.trim().to_ascii_lowercase();
        style.background = match low.as_str() {
            "inherit" => crate::ui::container::BackgroundFill::Inherit,
            "transparent" | "reset" => crate::ui::container::BackgroundFill::Transparent,
            _ => crate::ui::container::BackgroundFill::Solid(bg),
        };
    }
    style.foreground = tbl.get::<Option<String>>("foreground").unwrap_or(None);
    style.border_color = tbl.get::<Option<String>>("border_color").unwrap_or(None);
    if let Ok(bs) = tbl.get::<String>("border_style")
        && let Ok(parsed) = parse_border_style(&bs)
    {
        style.border_style = Some(parsed);
    }
    if let Ok(gtbl) = tbl.get::<Table>("gradient") {
        style.gradient = parse_gradient_table(&gtbl);
    }
    if let Ok(op) = tbl.get::<f32>("opacity") {
        style.opacity = Some(op.clamp(0.0, 1.0));
    }
    if let Ok(ef) = tbl.get::<String>("effects") {
        style.effects = match ef.trim().to_ascii_lowercase().as_str() {
            "allow" => EffectPolicy::Allow,
            "isolated" => EffectPolicy::Isolated,
            _ => EffectPolicy::Exclude,
        };
    }
    style
}

fn parse_container_table(tbl: &Table) -> ContainerConfig {
    let mut c = ContainerConfig::default();
    if let Ok(id) = tbl.get::<String>("id") {
        c.id = id;
    }
    if let Some(v) = tbl.get::<Option<bool>>("visible").ok().flatten() {
        c.visible = v;
    }
    if let Some(v) = tbl.get::<Option<bool>>("transparent").ok().flatten() {
        c.transparent = v;
    }
    if let Ok(d) = tbl.get::<String>("direction") {
        c.direction = match d.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "row" => ContainerDirection::Horizontal,
            _ => ContainerDirection::Vertical,
        };
    }
    if let Ok(w) = tbl.get::<Value>("width")
        && !w.is_nil()
    {
        c.width = parse_size(w);
    }
    if let Ok(h) = tbl.get::<Value>("height")
        && !h.is_nil()
    {
        c.height = parse_size(h);
    }
    c.min_width = tbl.get::<Option<u16>>("min_width").unwrap_or(None);
    c.max_width = tbl.get::<Option<u16>>("max_width").unwrap_or(None);
    c.min_height = tbl.get::<Option<u16>>("min_height").unwrap_or(None);
    c.max_height = tbl.get::<Option<u16>>("max_height").unwrap_or(None);
    c.padding = tbl.get::<u16>("padding").unwrap_or(0);
    c.gap = tbl.get::<u16>("gap").unwrap_or(0);
    if let Ok(btbl) = tbl.get::<Table>("border") {
        c.border = parse_border_table(&btbl);
    }
    if let Ok(bg) = tbl.get::<String>("background") {
        if bg.eq_ignore_ascii_case("transparent") {
            c.transparent = true;
            c.background = None;
        } else {
            c.transparent = false;
            c.background = Some(bg);
        }
    }
    if let Ok(gtbl) = tbl.get::<Table>("gradient") {
        c.gradient = parse_gradient_table(&gtbl);
        c.transparent = false;
    }
    if let Ok(op) = tbl.get::<f32>("opacity") {
        c.opacity = Some(op.clamp(0.0, 1.0));
    }
    if let Ok(children) = tbl.get::<Table>("children") {
        for child_tbl in children.sequence_values::<Table>().flatten() {
            c.children.push(parse_node_table(&child_tbl));
        }
    }
    c
}

fn parse_node_table(tbl: &Table) -> ContainerNode {
    let node_kind = tbl
        .get::<String>("node")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let has_children = tbl.get::<Table>("children").is_ok();
    let has_comp_kind = tbl.get::<String>("kind").is_ok();
    if node_kind == "component" || (!has_children && has_comp_kind) {
        ContainerNode::Component(parse_component_table(tbl))
    } else {
        ContainerNode::Container(parse_container_table(tbl))
    }
}

fn parse_component_table(tbl: &Table) -> ComponentConfig {
    let mut comp = ComponentConfig::default();
    if let Ok(id) = tbl.get::<String>("id") {
        comp.id = id;
    }
    if let Some(v) = tbl.get::<Option<bool>>("visible").ok().flatten() {
        comp.visible = v;
    }
    if let Ok(cat) = tbl.get::<String>("category") {
        comp.category = match cat.trim().to_ascii_lowercase().as_str() {
            "static" => ComponentCategory::Static,
            "animated" => ComponentCategory::Animated,
            _ => ComponentCategory::Dynamic,
        };
    }
    if let Ok(kind_str) = tbl.get::<String>("kind") {
        comp.kind = parse_component_kind(&kind_str);
    }
    if let Ok(w) = tbl.get::<Value>("width")
        && !w.is_nil()
    {
        comp.width = parse_size(w);
    }
    if let Ok(h) = tbl.get::<Value>("height")
        && !h.is_nil()
    {
        comp.height = parse_size(h);
    }
    comp.min_width = tbl.get::<Option<u16>>("min_width").unwrap_or(None);
    comp.max_width = tbl.get::<Option<u16>>("max_width").unwrap_or(None);
    comp.min_height = tbl.get::<Option<u16>>("min_height").unwrap_or(None);
    comp.max_height = tbl.get::<Option<u16>>("max_height").unwrap_or(None);

    if let Ok(a) = tbl.get::<String>("align") {
        comp.visual.align = parse_align(&a);
    }
    comp.visual.padding = tbl.get::<u16>("padding").unwrap_or(0);
    if let Ok(btbl) = tbl.get::<Table>("border") {
        comp.visual.border = parse_border_table(&btbl);
    }
    if let Ok(bg) = tbl.get::<String>("background") {
        comp.visual.background = Some(bg);
    }
    if let Ok(fg) = tbl.get::<String>("color") {
        comp.visual.color = Some(fg);
    }
    if let Ok(f) = tbl.get::<String>("font") {
        comp.visual.font = Some(f);
    }
    if let Ok(gs) = tbl.get::<String>("graph_style") {
        comp.visual.graph_style = Some(match gs.trim().to_ascii_lowercase().as_str() {
            "bar" => GraphStyle::Bar,
            _ => GraphStyle::Sparkline,
        });
    }
    if let Ok(t) = tbl.get::<String>("title") {
        comp.content.title = Some(t);
    }
    if let Ok(t) = tbl.get::<String>("text") {
        comp.content.text = Some(t);
    }
    if let Ok(fields) = tbl.get::<Table>("fields") {
        let vec: Vec<String> = fields.sequence_values::<String>().flatten().collect();
        if !vec.is_empty() {
            comp.content.fields = Some(vec);
        }
    }
    if let Ok(atbl) = tbl.get::<Table>("animation") {
        let mut anim = crate::ui::container::ComponentAnimationConfig::default();
        if let Ok(fps) = atbl.get::<u32>("fps") {
            anim.fps = Some(fps);
        }
        if let Ok(st) = atbl.get::<Option<String>>("style") {
            anim.style = st;
        }
        if let Ok(sp) = atbl.get::<f32>("speed") {
            anim.speed = sp;
        }
        comp.animation = Some(anim);
    }
    // Note: `Function` unused warning safeguard if additional callbacks are passed
    let _: Option<Function> = tbl.get::<Function>("on_render").ok();
    let _: Option<EffectPolicy> = None;
    comp
}

fn parse_component_kind(raw: &str) -> ComponentKind {
    match raw.trim().to_ascii_lowercase().as_str() {
        "header" => ComponentKind::Header,
        "greeting" => ComponentKind::Greeting,
        "system" => ComponentKind::System,
        "hardware" => ComponentKind::Hardware,
        "environment" => ComponentKind::Environment,
        "terminal" => ComponentKind::Terminal,
        "storage" => ComponentKind::Storage,
        "activity" => ComponentKind::Activity,
        "cpu" => ComponentKind::Cpu,
        "memory" => ComponentKind::Memory,
        "temperature" => ComponentKind::Temperature,
        "processes" => ComponentKind::Processes,
        "network" => ComponentKind::Network,
        "uptime" => ComponentKind::Uptime,
        "session" => ComponentKind::Session,
        "graph" => ComponentKind::Graph,
        "ascii_logo" => ComponentKind::AsciiLogo,
        "separator" => ComponentKind::Separator,
        "spinner" => ComponentKind::Spinner,
        "typewriter" => ComponentKind::Typewriter,
        "frames" => ComponentKind::Frames,
        "matrix" => ComponentKind::Matrix,
        "particles" => ComponentKind::Particles,
        "ascii_object" => ComponentKind::AsciiObject,
        _ => ComponentKind::Text,
    }
}
