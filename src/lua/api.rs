use crate::config::schema::BackgroundBaseType;
use crate::config::{Align, BorderStyle, GraphStyle, StartupTrigger, ThemeConfig, ViewMode};
use crate::lua::{LuaConfigBundle, LuaEffectSpec};
use crate::ui::borders::parse_border_style;
use crate::ui::container::{
    ComponentAnimationConfig, ComponentCategory, ComponentKind, ContainerBorderConfig,
    ContainerConfig, ContainerGradient, ContainerSize, EffectPolicy, GradientDirection,
    InlineLuaScript, LayoutRegion,
};
use mlua::{Function, Lua, Table, Value};
use std::collections::HashSet;
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

    // egotop.layout({ padding = 1, gap = 1, containers = { ... } })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "layout",
            lua.create_function(move |_, tbl: Table| {
                if tbl.contains_key("children")? {
                    return Err(mlua::Error::RuntimeError(
                        "Recursive 'children' trees are removed; declare a flat 'containers' queue in egotop.layout({ containers = { ... } })"
                            .to_string(),
                    ));
                }
                let mut ui = crate::config::UiConfig {
                    padding: tbl.get::<u16>("padding").unwrap_or(1),
                    gap: tbl.get::<u16>("gap").unwrap_or(1),
                    min_container_width: tbl.get::<u16>("min_container_width").unwrap_or(22),
                    containers: Vec::new(),
                };

                let list_tbl = if let Ok(ctbl) = tbl.get::<Table>("containers") {
                    ctbl
                } else {
                    tbl.clone()
                };

                let mut seen_names = HashSet::new();
                for (idx, ctbl) in list_tbl.sequence_values::<Table>().enumerate() {
                    let ctbl = ctbl?;
                    let c = parse_container_table(&ctbl, idx)?;
                    if !seen_names.insert(c.name.clone()) {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Duplicate container name '{}' in egotop.layout",
                            c.name
                        )));
                    }
                    ui.containers.push(c);
                }

                let mut bundle = st.lock().unwrap();
                bundle.config.ui = ui;
                Ok(())
            })?,
        )?;
    }

    // egotop.container_styles({ [name] = { background = ..., foreground = ..., ... } })
    {
        let st = Arc::clone(&state);
        egotop.set(
            "container_styles",
            lua.create_function(move |_, tbl: Table| {
                let mut bundle = st.lock().unwrap();
                for pair in tbl.pairs::<String, Table>().flatten() {
                    let (name, stbl) = pair;
                    let cs = parse_container_style_table(&stbl);
                    bundle.config.container_styles.insert(name, cs);
                }
                Ok(())
            })?,
        )?;
    }

    // egotop.math.* (pure math & color utilities; no canvas registry or global UI state)
    if let Ok(math_tbl) = crate::lua::canvas::create_math_table(lua) {
        egotop.set("math", math_tbl)?;
    }

    // egotop.container(tbl): validates flat container declaration (cannot nest containers)
    egotop.set(
        "container",
        lua.create_function(|_, tbl: Table| {
            if tbl.contains_key("children")? {
                return Err(mlua::Error::RuntimeError(
                    "A container cannot contain another container ('children' is forbidden)"
                        .to_string(),
                ));
            }
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
        if let Some(first) = bundle
            .effect_specs
            .iter()
            .find(|s| !s.name.eq_ignore_ascii_case("none"))
        {
            bg.enabled = true;
            bg.speed = first.speed;
            bg.density = first.density;
            let mapped = match first.name.to_ascii_lowercase().as_str() {
                "matrix" | "matrix_rain" => Some(crate::config::EffectKind::Matrix),
                "water" | "waves" => Some(crate::config::EffectKind::Water),
                "particles" => Some(crate::config::EffectKind::Particles),
                "scanlines" => Some(crate::config::EffectKind::Scanlines),
                _ => None,
            };
            if let Some(k) = mapped {
                bg.effect = k;
                bg.preset = None;
            } else {
                bg.effect = crate::config::EffectKind::None;
                bg.preset = Some(first.name.clone());
            }
        } else {
            bg.enabled = false;
            bg.preset = None;
            bg.effect = crate::config::EffectKind::None;
        }
    }
    if let Ok(btbl) = tbl.get::<Table>("border") {
        bg.border = parse_border_table(&btbl);
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
            let txt = s
                .to_str()
                .map(|x| x.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if txt.is_empty() || txt == "auto" {
                ContainerSize::Auto
            } else if txt == "fill" {
                ContainerSize::Fill(1)
            } else if let Some(inner) = txt.strip_prefix("fill(").and_then(|v| v.strip_suffix(')')) {
                let w = inner.trim().parse::<u16>().unwrap_or(1).max(1);
                ContainerSize::Fill(w)
            } else if let Some(pct) = txt
                .strip_suffix('%')
                .and_then(|n| n.trim().parse::<u16>().ok())
            {
                ContainerSize::Percent(pct.min(100))
            } else if let Ok(px) = txt.parse::<u16>() {
                ContainerSize::Fixed(px)
            } else {
                ContainerSize::Auto
            }
        }
        _ => ContainerSize::Auto,
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
        .and_then(|s| parse_border_style(&s).ok());
    let color = tbl.get::<Option<String>>("color").unwrap_or(None);
    let title = tbl.get::<Option<String>>("title").unwrap_or(None);
    ContainerBorderConfig {
        enabled: enabled && style != Some(BorderStyle::None),
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
    if let Ok(anim) = tbl.get::<Option<String>>("animation") {
        style.animation = anim;
    }
    style
}

fn parse_container_table(tbl: &Table, index: usize) -> mlua::Result<ContainerConfig> {
    if tbl.contains_key("children")? {
        return Err(mlua::Error::RuntimeError(
            "A container cannot contain another container ('children' is forbidden)".to_string(),
        ));
    }

    let mut c = ContainerConfig::default();
    c.name = tbl
        .get::<String>("name")
        .or_else(|_| tbl.get::<String>("id"))
        .unwrap_or_else(|_| format!("container_{}", index + 1));
    c.description = tbl.get::<String>("description").unwrap_or_default();

    if let Some(v) = tbl.get::<Option<bool>>("visible").ok().flatten() {
        c.visible = v;
    }
    if let Ok(reg_str) = tbl.get::<String>("region")
        && let Some(reg) = LayoutRegion::parse(&reg_str)
    {
        c.region = reg;
    }
    if let Ok(order) = tbl.get::<i32>("order") {
        c.order = order;
    }
    c.row = tbl.get::<Option<u16>>("row").unwrap_or(None);

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

    if let Some(v) = tbl.get::<Option<bool>>("transparent").ok().flatten() {
        c.transparent = v;
    }
    if let Ok(a) = tbl.get::<String>("align") {
        c.align = parse_align(&a);
    }
    if let Ok(btbl) = tbl.get::<Table>("border") {
        c.border = parse_border_table(&btbl);
    }
    if let Ok(fg) = tbl.get::<String>("color") {
        c.color = Some(fg);
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
    if let Ok(f) = tbl.get::<String>("font") {
        c.font = Some(f);
    }
    if let Ok(gs) = tbl.get::<String>("graph_style") {
        c.graph_style = Some(match gs.trim().to_ascii_lowercase().as_str() {
            "bar" => GraphStyle::Bar,
            _ => GraphStyle::Sparkline,
        });
    }
    if let Ok(ef) = tbl.get::<String>("effects") {
        c.effects = match ef.trim().to_ascii_lowercase().as_str() {
            "allow" => EffectPolicy::Allow,
            "isolated" => EffectPolicy::Isolated,
            _ => EffectPolicy::Exclude,
        };
    }

    let has_explicit_cat = tbl.get::<String>("category").is_ok();
    if let Ok(cat) = tbl.get::<String>("category") {
        c.category = match cat.trim().to_ascii_lowercase().as_str() {
            "static" => ComponentCategory::Static,
            "animated" => ComponentCategory::Animated,
            _ => ComponentCategory::Dynamic,
        };
    }
    if let Ok(kind_str) = tbl.get::<String>("kind").or_else(|_| tbl.get::<String>("type")) {
        c.kind = parse_component_kind(&kind_str);
        if !has_explicit_cat {
            c.category = c.kind.expected_category();
        }
    }

    if let Ok(t) = tbl.get::<String>("title") {
        c.content.title = Some(t);
    }
    if let Ok(t) = tbl.get::<String>("text") {
        c.content.text = Some(t);
    }
    if let Ok(p) = tbl.get::<String>("preset") {
        c.content.preset = Some(p);
    }
    if let Ok(s) = tbl.get::<String>("script") {
        c.content.script = Some(s);
    }
    if let Ok(m) = tbl.get::<String>("metric") {
        c.content.metric = Some(m);
    }
    if let Ok(sc) = tbl.get::<String>("separator_char") {
        c.content.separator_char = Some(sc);
    }
    if let Ok(frames) = tbl.get::<Table>("frames") {
        c.content.frames = frames.sequence_values::<String>().flatten().collect();
    }
    if let Ok(fields) = tbl.get::<Table>("fields") {
        let vec: Vec<String> = fields.sequence_values::<String>().flatten().collect();
        if !vec.is_empty() {
            c.content.fields = Some(vec);
        }
    }
    if let Ok(anim_str) = tbl.get::<String>("animation") {
        c.content.script = Some(anim_str.clone());
        c.animation = Some(ComponentAnimationConfig {
            style: Some(anim_str),
            ..Default::default()
        });
    } else if let Ok(atbl) = tbl.get::<Table>("animation") {
        let mut anim = ComponentAnimationConfig::default();
        if let Ok(fps) = atbl.get::<u32>("fps") {
            anim.fps = Some(fps);
        }
        if let Ok(st) = atbl.get::<Option<String>>("style") {
            anim.style = st;
        }
        if let Ok(sp) = atbl.get::<f32>("speed") {
            anim.speed = sp;
        }
        if let Ok(den) = atbl.get::<f32>("density") {
            anim.density = Some(den);
        }
        if let Some(la) = atbl.get::<Option<bool>>("loop_anim").ok().flatten() {
            anim.loop_anim = la;
        }
        if let Ok(scr) = atbl.get::<Option<String>>("script")
            && c.content.script.is_none()
        {
            c.content.script = scr;
        }
        c.animation = Some(anim);
    }

    // Inline isolated canvas callbacks: init / update / render (or on_render) / resize
    let init_fn = tbl.get::<Function>("init").ok();
    let update_fn = tbl.get::<Function>("update").ok();
    let render_fn = tbl
        .get::<Function>("render")
        .or_else(|_| tbl.get::<Function>("on_render"))
        .ok();
    let resize_fn = tbl.get::<Function>("resize").ok();
    if init_fn.is_some() || update_fn.is_some() || render_fn.is_some() || resize_fn.is_some() {
        c.inline_lua = Some(InlineLuaScript {
            init_chunk: init_fn.map(|f| f.dump(false)),
            update_chunk: update_fn.map(|f| f.dump(false)),
            render_chunk: render_fn.map(|f| f.dump(false)),
            resize_chunk: resize_fn.map(|f| f.dump(false)),
        });
        c.kind = ComponentKind::Canvas;
        c.category = ComponentCategory::Animated;
    }

    if c.kind == ComponentKind::Canvas && c.content.script.is_none() && c.inline_lua.is_none() {
        if let Ok(raw_kind) = tbl.get::<String>("kind")
            && raw_kind.trim().eq_ignore_ascii_case("christmas_tree")
        {
            c.content.script = Some("effects.christmas_tree".to_string());
        } else if let Some(preset) = c.content.preset.clone() {
            c.content.script = Some(preset);
        }
    }

    Ok(c)
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
        "canvas" | "lua" | "shader" | "christmas_tree" => ComponentKind::Canvas,
        _ => ComponentKind::Text,
    }
}
