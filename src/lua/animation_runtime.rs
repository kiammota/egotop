use crate::animation::EasingCurve;
use crate::config::paths;
use crate::lua::canvas::{CanvasBuffer, LuaCanvasHandle, attach_math_helpers, build_theme_lua_table};
use crate::lua::configure_lua_vm;
use crate::ui::theme::{Theme, blend_color, parse_color};
use mlua::{Function, Lua, RegistryKey, Table, Value};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationLayer {
    Background,
    Boxed,
    #[default]
    Overlay,
}

struct ScheduledEvery {
    interval_ms: f32,
    accum_ms: f32,
    callback: RegistryKey,
}

struct ScheduledAfter {
    delay_ms: f32,
    fired: bool,
    callback: RegistryKey,
}

struct ActiveTween {
    from: f32,
    to: f32,
    duration_ms: f32,
    elapsed_ms: f32,
    easing: EasingCurve,
    on_update: Option<RegistryKey>,
    on_complete: Option<RegistryKey>,
    completed: bool,
}

struct SequenceStep {
    duration_ms: f32,
    easing: EasingCurve,
    started: bool,
    on_start: Option<RegistryKey>,
    on_update: Option<RegistryKey>,
}

struct ActiveSequence {
    steps: Vec<SequenceStep>,
    current_idx: usize,
    step_elapsed_ms: f32,
    on_complete: Option<RegistryKey>,
    completed: bool,
}

#[derive(Clone)]
struct RuntimeShared {
    name: String,
    description: String,
    screen_w: u16,
    screen_h: u16,
    elapsed_ms: f32,
    dt_secs: f32,
    reveal_progress: Arc<Mutex<f32>>,
    finished: Arc<Mutex<bool>>,
    paused: bool,
    layer: Arc<Mutex<AnimationLayer>>,
    box_title: Arc<Mutex<Option<String>>>,
    canvas_buf: Arc<Mutex<CanvasBuffer>>,
    every_tasks: Arc<Mutex<Vec<ScheduledEvery>>>,
    after_tasks: Arc<Mutex<Vec<ScheduledAfter>>>,
    tweens: Arc<Mutex<Vec<ActiveTween>>>,
    sequences: Arc<Mutex<Vec<ActiveSequence>>>,
    shader_module: Arc<Mutex<Option<RegistryKey>>>,
    shader_state: Arc<Mutex<Option<RegistryKey>>>,
    shader_init_called: Arc<Mutex<bool>>,
    duration_limit_ms: Arc<Mutex<Option<f32>>>,
}

/// Isolated, throttled Lua Animation Runtime.
/// Each independent animation owns a private Lua VM and an exclusive `CanvasBuffer`
/// with no access to global coordinates or other containers' canvases.
pub struct LuaAnimationRuntime {
    pub active_id: String,
    pub description: String,
    lua: Option<Lua>,
    shared: RuntimeShared,
    target_fps: u32,
    accum_dt: f32,
    pub last_error: Option<String>,
}

impl LuaAnimationRuntime {
    pub fn start(name: &str, screen_w: u16, screen_h: u16, target_fps: u32) -> Self {
        let desc = format!("Independent Lua animation '{name}'");
        let mut rt = Self {
            active_id: name.to_string(),
            description: desc.clone(),
            lua: None,
            shared: RuntimeShared {
                name: name.to_string(),
                description: desc.clone(),
                screen_w,
                screen_h,
                elapsed_ms: 0.0,
                dt_secs: 0.0,
                reveal_progress: Arc::new(Mutex::new(0.0)),
                finished: Arc::new(Mutex::new(false)),
                paused: false,
                layer: Arc::new(Mutex::new(AnimationLayer::Overlay)),
                box_title: Arc::new(Mutex::new(None)),
                canvas_buf: Arc::new(Mutex::new(CanvasBuffer::new_named(
                    name, desc, screen_w, screen_h,
                ))),
                every_tasks: Arc::new(Mutex::new(Vec::new())),
                after_tasks: Arc::new(Mutex::new(Vec::new())),
                tweens: Arc::new(Mutex::new(Vec::new())),
                sequences: Arc::new(Mutex::new(Vec::new())),
                shader_module: Arc::new(Mutex::new(None)),
                shader_state: Arc::new(Mutex::new(None)),
                shader_init_called: Arc::new(Mutex::new(false)),
                duration_limit_ms: Arc::new(Mutex::new(None)),
            },
            target_fps: target_fps.clamp(1, 60),
            accum_dt: 0.0,
            last_error: None,
        };
        rt.load_and_setup(name);
        rt
    }

    pub fn layer(&self) -> AnimationLayer {
        *self.shared.layer.lock().unwrap()
    }

    pub fn canvas_dims(&self) -> (u16, u16) {
        let guard = self.shared.canvas_buf.lock().unwrap();
        (guard.width, guard.height)
    }

    pub fn boxed_rect(&self, screen: Rect) -> Rect {
        let w = screen.width.saturating_sub(6).clamp(24, 72).min(screen.width);
        let h = screen.height.saturating_sub(4).clamp(14, 30).min(screen.height);
        let x = screen.x + screen.width.saturating_sub(w) / 2;
        let y = screen.y + screen.height.saturating_sub(h) / 2;
        Rect::new(x, y, w, h)
    }

    fn effective_canvas_dims(&self) -> (u16, u16) {
        if self.layer() == AnimationLayer::Boxed {
            let rect = self.boxed_rect(Rect::new(0, 0, self.shared.screen_w, self.shared.screen_h));
            (rect.width.saturating_sub(2), rect.height.saturating_sub(2))
        } else {
            (self.shared.screen_w, self.shared.screen_h)
        }
    }

    pub fn is_finished(&self) -> bool {
        *self.shared.finished.lock().unwrap()
    }

    pub fn reveal_progress(&self) -> f32 {
        *self.shared.reveal_progress.lock().unwrap()
    }

    pub fn pause(&mut self) {
        self.shared.paused = true;
    }

    pub fn resume(&mut self) {
        self.shared.paused = false;
    }

    pub fn stop(&mut self) {
        *self.shared.finished.lock().unwrap() = true;
        *self.shared.reveal_progress.lock().unwrap() = 1.0;
        self.shared.canvas_buf.lock().unwrap().begin_frame();
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.shared.screen_w = w;
        self.shared.screen_h = h;
        let (cw, ch) = self.effective_canvas_dims();
        self.shared.canvas_buf.lock().unwrap().resize(cw, ch);

        if let Some(lua) = self.lua.as_ref()
            && let Some(mod_key) = self.shared.shader_module.lock().unwrap().as_ref()
            && let Ok(mod_tbl) = lua.registry_value::<Table>(mod_key)
            && let Some(ctx) = self.build_shader_ctx(lua, cw, ch, 0.0)
        {
            let result = match mod_tbl.get::<Value>("on_resize") {
                Ok(Value::Function(f)) => f.call::<()>((cw, ch, ctx.clone())),
                _ => match mod_tbl.get::<Value>("resize") { Ok(Value::Function(f)) => f.call::<()>(ctx), _ => Ok(()) },
            };
            if let Err(e) = result { self.last_error = Some(e.to_string()); }
        }
    }

    fn load_and_setup(&mut self, name: &str) {
        let lua = Lua::new();
        if let Err(e) = configure_lua_vm(&lua, None) {
            self.last_error = Some(e.to_string());
            self.stop();
            return;
        }

        let api = match self.build_api_table(&lua) {
            Ok(t) => t,
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.stop();
                return;
            }
        };

        let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
        let custom_path = paths::animations_path().join(format!("{normalized}.lua"));
        let res: mlua::Result<()> = (|| {
            let mod_tbl: Value = if custom_path.exists() {
                let raw = std::fs::read_to_string(&custom_path).unwrap_or_default();
                lua.load(&raw)
                    .set_name(custom_path.display().to_string())
                    .eval()?
            } else {
                let require: Function = lua.globals().get("require")?;
                let mod_name = format!("animations.{normalized}");
                require.call(mod_name.as_str())?
            };

            if let Value::Table(t) = mod_tbl {
                let returned_shader: Option<Table> = if let Ok(f) = t.get::<Function>("setup") {
                    match f.call::<Value>(api)? {
                        Value::Table(ret) => Some(ret),
                        _ => None,
                    }
                } else if t.get::<Function>("render").is_ok() || t.get::<Function>("update").is_ok() {
                    Some(t)
                } else {
                    None
                };

                if let Some(stbl) = returned_shader {
                    if let Ok(layer_str) = stbl.get::<String>("layer") {
                        *self.shared.layer.lock().unwrap() = parse_animation_layer(&layer_str);
                    }
                    if let Ok(title) = stbl.get::<String>("title") {
                        *self.shared.box_title.lock().unwrap() = Some(title);
                    }
                    if let Ok(desc) = stbl.get::<String>("description") {
                        self.description = desc.clone();
                        self.shared
                            .canvas_buf
                            .lock()
                            .unwrap()
                            .set_identity(&self.active_id, &desc);
                    }
                    if let Ok(fps) = stbl.get::<u32>("fps") {
                        self.target_fps = fps.clamp(1, 60);
                    }
                    if let Ok(dur_s) = stbl.get::<f32>("duration") {
                        *self.shared.duration_limit_ms.lock().unwrap() =
                            (dur_s > 0.0).then_some(dur_s * 1000.0);
                    } else if let Ok(dur_ms) = stbl.get::<f32>("duration_ms") {
                        *self.shared.duration_limit_ms.lock().unwrap() =
                            (dur_ms > 0.0).then_some(dur_ms);
                    }
                    if let Ok(mod_key) = lua.create_registry_value(stbl)
                        && let Ok(state_tbl) = lua.create_table()
                        && let Ok(state_key) = lua.create_registry_value(state_tbl)
                    {
                        *self.shared.shader_module.lock().unwrap() = Some(mod_key);
                        *self.shared.shader_state.lock().unwrap() = Some(state_key);
                    }
                }
            }
            Ok(())
        })();

        if let Err(e) = res {
            self.last_error = Some(format!("Lua animation '{name}' error: {e}"));
            self.stop();
            return;
        }
        let (cw, ch) = self.effective_canvas_dims();
        self.shared.canvas_buf.lock().unwrap().resize(cw, ch);
        self.lua = Some(lua);
    }

    fn build_api_table(&self, lua: &Lua) -> mlua::Result<Table> {
        let api = lua.create_table()?;
        let shared = self.shared.clone();

        api.set("name", shared.name.clone())?;
        api.set("description", shared.description.clone())?;

        // Isolated canvas bound exclusively to this animation unit
        let theme = Theme::from_config(&crate::config::ThemeConfig::default());
        api.set(
            "canvas",
            LuaCanvasHandle::new(Arc::clone(&shared.canvas_buf), theme),
        )?;

        {
            let sh = shared.clone();
            api.set(
                "elapsed_ms",
                lua.create_function(move |_, ()| Ok(sh.elapsed_ms))?,
            )?;
        }
        {
            let sh = shared.clone();
            api.set("dt", lua.create_function(move |_, ()| Ok(sh.dt_secs))?)?;
        }

        {
            let ff = Arc::clone(&shared.finished);
            let rp = Arc::clone(&shared.reveal_progress);
            let cbuf = Arc::clone(&shared.canvas_buf);
            api.set(
                "finish",
                lua.create_function(move |_, ()| {
                    *ff.lock().unwrap() = true;
                    *rp.lock().unwrap() = 1.0;
                    cbuf.lock().unwrap().begin_frame();
                    Ok(())
                })?,
            )?;
        }

        api.set(
            "figlet",
            lua.create_function(|_, (text, font_name): (String, Option<String>)| {
                let fname = font_name.as_deref().unwrap_or("Standard");
                let lines = crate::ui::typography::render_figlet_lines(&text, fname, false);
                Ok(lines)
            })?,
        )?;

        api.set(
            "lerp_color",
            lua.create_function(|_, (c1, c2, t): (String, String, f32)| {
                let a = parse_color(&c1, Color::White);
                let b = parse_color(&c2, Color::Black);
                match blend_color(a, b, t.clamp(0.0, 1.0)) {
                    Color::Rgb(r, g, b) => Ok(format!("#{r:02X}{g:02X}{b:02X}")),
                    _ => Ok("#FFFFFF".to_string()),
                }
            })?,
        )?;

        {
            let every_tasks = Arc::clone(&shared.every_tasks);
            api.set(
                "every",
                lua.create_function(move |lua, (interval_ms, cb): (f32, Function)| {
                    let key = lua.create_registry_value(cb)?;
                    every_tasks.lock().unwrap().push(ScheduledEvery {
                        interval_ms: interval_ms.max(16.0),
                        accum_ms: 0.0,
                        callback: key,
                    });
                    Ok(())
                })?,
            )?;
        }

        {
            let after_tasks = Arc::clone(&shared.after_tasks);
            api.set(
                "after",
                lua.create_function(move |lua, (delay_ms, cb): (f32, Function)| {
                    let key = lua.create_registry_value(cb)?;
                    after_tasks.lock().unwrap().push(ScheduledAfter {
                        delay_ms: delay_ms.max(0.0),
                        fired: false,
                        callback: key,
                    });
                    Ok(())
                })?,
            )?;
        }

        {
            let tweens = Arc::clone(&shared.tweens);
            api.set(
                "tween",
                lua.create_function(move |lua, spec: Table| {
                    let from = spec.get::<f32>("from").unwrap_or(0.0);
                    let to = spec.get::<f32>("to").unwrap_or(1.0);
                    let duration_ms = spec.get::<f32>("duration_ms").unwrap_or(500.0).max(1.0);
                    let easing = spec
                        .get::<String>("easing")
                        .ok()
                        .and_then(|s| EasingCurve::parse(&s).ok())
                        .unwrap_or(EasingCurve::Linear);
                    let on_update = spec
                        .get::<Function>("on_update")
                        .ok()
                        .and_then(|f| lua.create_registry_value(f).ok());
                    let on_complete = spec
                        .get::<Function>("on_complete")
                        .ok()
                        .and_then(|f| lua.create_registry_value(f).ok());
                    tweens.lock().unwrap().push(ActiveTween {
                        from,
                        to,
                        duration_ms,
                        elapsed_ms: 0.0,
                        easing,
                        on_update,
                        on_complete,
                        completed: false,
                    });
                    Ok(())
                })?,
            )?;
        }

        {
            let seqs = Arc::clone(&shared.sequences);
            api.set(
                "sequence",
                lua.create_function(move |lua, (steps_tbl, on_done): (Table, Option<Function>)| {
                    let mut steps = Vec::new();
                    for step_tbl in steps_tbl.sequence_values::<Table>().flatten() {
                        let duration_ms = step_tbl.get::<f32>("duration_ms").unwrap_or(200.0).max(1.0);
                        let easing = step_tbl
                            .get::<String>("easing")
                            .ok()
                            .and_then(|s| EasingCurve::parse(&s).ok())
                            .unwrap_or(EasingCurve::Linear);
                        let on_start = step_tbl
                            .get::<Function>("on_start")
                            .ok()
                            .and_then(|f| lua.create_registry_value(f).ok());
                        let on_update = step_tbl
                            .get::<Function>("on_update")
                            .ok()
                            .and_then(|f| lua.create_registry_value(f).ok());
                        steps.push(SequenceStep {
                            duration_ms,
                            easing,
                            started: false,
                            on_start,
                            on_update,
                        });
                    }
                    let on_complete = on_done.and_then(|f| lua.create_registry_value(f).ok());
                    seqs.lock().unwrap().push(ActiveSequence {
                        steps,
                        current_idx: 0,
                        step_elapsed_ms: 0.0,
                        on_complete,
                        completed: false,
                    });
                    Ok(())
                })?,
            )?;
        }

        api.set(
            "parallel",
            lua.create_function(|_, tasks: Table| {
                for f in tasks.sequence_values::<Function>().flatten() {
                    let _ = f.call::<()>(());
                }
                Ok(())
            })?,
        )?;

        let ui = lua.create_table()?;
        {
            let rc = Arc::clone(&shared.reveal_progress);
            let reveal_fn = lua.create_function(move |_, p: f32| {
                *rc.lock().unwrap() = p.clamp(0.0, 1.0);
                Ok(())
            })?;
            ui.set("reveal", reveal_fn.clone())?;
            api.set("reveal", reveal_fn)?;
        }
        api.set("ui", ui)?;

        {
            let lr = Arc::clone(&shared.layer);
            api.set(
                "layer",
                lua.create_function(move |_, layer_str: String| {
                    *lr.lock().unwrap() = parse_animation_layer(&layer_str);
                    Ok(())
                })?,
            )?;
        }
        {
            let bt = Arc::clone(&shared.box_title);
            api.set(
                "box_title",
                lua.create_function(move |_, title: Option<String>| {
                    *bt.lock().unwrap() = title;
                    Ok(())
                })?,
            )?;
        }

        attach_math_helpers(lua, &api)?;
        Ok(api)
    }

    fn build_shader_ctx(&self, lua: &Lua, cw: u16, ch: u16, step_dt: f32) -> Option<Table> {
        let t_secs = self.shared.elapsed_ms / 1000.0;
        let theme = Theme::from_config(&crate::config::ThemeConfig::default());
        let state_guard = self.shared.shader_state.lock().unwrap();
        let st = state_guard
            .as_ref()
            .and_then(|k| lua.registry_value::<Table>(k).ok())?;
        let ctx = lua.create_table().ok()?;
        let _ = ctx.set("name", self.active_id.as_str());
        let _ = ctx.set("description", self.description.as_str());
        let _ = ctx.set("width", cw);
        let _ = ctx.set("height", ch);
        let cell_aspect = 0.5_f64;
        let _ = ctx.set("cell_aspect", cell_aspect);
        let _ = ctx.set("aspect", if ch == 0 { 1.0 } else { cw as f64 * cell_aspect / ch as f64 });
        let _ = ctx.set("lod", ((cw as f64 * ch as f64 / 2400.0).sqrt()).clamp(0.0, 1.0));
        let _ = ctx.set("dt", step_dt);
        let _ = ctx.set("t", t_secs);
        let _ = ctx.set("time", t_secs);
        let _ = ctx.set("elapsed", t_secs);
        let _ = ctx.set("fps", self.target_fps);
        let _ = ctx.set("state", st);
        if let Ok(tt) = build_theme_lua_table(lua, &theme) {
            let _ = ctx.set("theme", tt);
        }
        let rc = Arc::clone(&self.shared.reveal_progress);
        if let Ok(rf) = lua.create_function(move |_, p: f32| {
            *rc.lock().unwrap() = p.clamp(0.0, 1.0);
            Ok(())
        }) {
            let _ = ctx.set("reveal", rf);
        }
        let ff = Arc::clone(&self.shared.finished);
        let rp = Arc::clone(&self.shared.reveal_progress);
        if let Ok(fin) = lua.create_function(move |_, ()| {
            *ff.lock().unwrap() = true;
            *rp.lock().unwrap() = 1.0;
            Ok(())
        }) {
            let _ = ctx.set("finish", fin);
        }
        let _ = attach_math_helpers(lua, &ctx);
        Some(ctx)
    }

    pub fn update(&mut self, dt_secs: f32) {
        if *self.shared.finished.lock().unwrap() || self.shared.paused {
            return;
        }
        self.accum_dt += dt_secs;
        let min_step = 1.0 / (self.target_fps.max(1) as f32);
        if self.accum_dt < min_step {
            return;
        }
        let step_dt = self.accum_dt;
        self.accum_dt = 0.0;
        self.shared.dt_secs = step_dt;
        self.shared.elapsed_ms += step_dt * 1000.0;

        let Some(lua) = self.lua.as_ref() else {
            return;
        };

        // 1. Scheduled after_tasks
        let mut afters = self.shared.after_tasks.lock().unwrap();
        for task in afters.iter_mut() {
            if !task.fired && self.shared.elapsed_ms >= task.delay_ms {
                task.fired = true;
                if let Ok(f) = lua.registry_value::<Function>(&task.callback)
                    && let Err(e) = f.call::<()>(())
                {
                    self.last_error = Some(e.to_string());
                }
            }
        }
        drop(afters);

        // 2. Scheduled every_tasks
        let mut everys = self.shared.every_tasks.lock().unwrap();
        for task in everys.iter_mut() {
            task.accum_ms += step_dt * 1000.0;
            if task.accum_ms >= task.interval_ms {
                task.accum_ms -= task.interval_ms;
                if let Ok(f) = lua.registry_value::<Function>(&task.callback)
                    && let Err(e) = f.call::<()>(())
                {
                    self.last_error = Some(e.to_string());
                }
            }
        }
        drop(everys);

        // 3. Active tweens
        let mut completed_tween_cbs = Vec::new();
        {
            let mut tweens = self.shared.tweens.lock().unwrap();
            for tw in tweens.iter_mut() {
                if tw.completed {
                    continue;
                }
                tw.elapsed_ms += step_dt * 1000.0;
                let t = (tw.elapsed_ms / tw.duration_ms).clamp(0.0, 1.0);
                let val = tw.from + (tw.to - tw.from) * tw.easing.apply(t);
                if let Some(ref key) = tw.on_update
                    && let Ok(f) = lua.registry_value::<Function>(key)
                    && let Err(e) = f.call::<()>(val)
                {
                    self.last_error = Some(e.to_string());
                }
                if t >= 1.0 {
                    tw.completed = true;
                    if let Some(key) = tw.on_complete.take() {
                        completed_tween_cbs.push(key);
                    }
                }
            }
        }
        for key in completed_tween_cbs {
            if let Ok(f) = lua.registry_value::<Function>(&key) {
                let _ = f.call::<()>(());
            }
            *self.shared.finished.lock().unwrap() = true;
            *self.shared.reveal_progress.lock().unwrap() = 1.0;
        }

        // 4. Active sequences
        let mut completed_seq_cbs = Vec::new();
        {
            let mut seqs = self.shared.sequences.lock().unwrap();
            for seq in seqs.iter_mut() {
                if seq.completed {
                    continue;
                }
                if seq.current_idx >= seq.steps.len() {
                    seq.completed = true;
                    if let Some(key) = seq.on_complete.take() {
                        completed_seq_cbs.push(key);
                    }
                    continue;
                }
                let step = &mut seq.steps[seq.current_idx];
                if !step.started {
                    step.started = true;
                    if let Some(ref key) = step.on_start
                        && let Ok(f) = lua.registry_value::<Function>(key)
                    {
                        let _ = f.call::<()>(());
                    }
                }
                seq.step_elapsed_ms += step_dt * 1000.0;
                let t = (seq.step_elapsed_ms / step.duration_ms).clamp(0.0, 1.0);
                if let Some(ref key) = step.on_update
                    && let Ok(f) = lua.registry_value::<Function>(key)
                {
                    let _ = f.call::<()>(step.easing.apply(t));
                }
                if t >= 1.0 {
                    seq.current_idx += 1;
                    seq.step_elapsed_ms = 0.0;
                }
            }
        }
        for key in completed_seq_cbs {
            if let Ok(f) = lua.registry_value::<Function>(&key) {
                let _ = f.call::<()>(());
            }
            *self.shared.finished.lock().unwrap() = true;
            *self.shared.reveal_progress.lock().unwrap() = 1.0;
        }

        // 5. Shader lifecycle (`init(ctx)` / `update(dt, t, ctx)` / `render(canvas, ctx)`)
        let shader_mod_key_present = self.shared.shader_module.lock().unwrap().is_some();
        if shader_mod_key_present {
            let (cw, ch) = self.effective_canvas_dims();
            let t_secs = self.shared.elapsed_ms / 1000.0;
            let theme = Theme::from_config(&crate::config::ThemeConfig::default());
            let ctx_opt = self.build_shader_ctx(lua, cw, ch, step_dt);

            let mod_tbl_opt = self
                .shared
                .shader_module
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|k| lua.registry_value::<Table>(k).ok());

            if let (Some(mod_tbl), Some(ctx)) = (mod_tbl_opt, ctx_opt) {
                let mut init_called = self.shared.shader_init_called.lock().unwrap();
                if !*init_called {
                    *init_called = true;
                    if let Ok(Value::Function(init_fn)) = mod_tbl.get::<Value>("init")
                        && let Err(e) = init_fn.call::<()>(ctx.clone())
                    {
                        self.last_error = Some(e.to_string());
                    }
                }
                drop(init_called);

                if let Ok(Value::Function(upd_fn)) = mod_tbl.get::<Value>("update")
                    && let Err(e) = upd_fn.call::<()>((step_dt, t_secs, ctx.clone()))
                {
                    self.last_error = Some(e.to_string());
                }

                {
                    let mut guard = self.shared.canvas_buf.lock().unwrap();
                    guard.resize(cw, ch);
                    guard.begin_frame();
                }
                let canvas_handle =
                    LuaCanvasHandle::new(Arc::clone(&self.shared.canvas_buf), theme);
                if let Ok(Value::Function(ren_fn)) = mod_tbl.get::<Value>("render")
                    && let Err(e) = ren_fn.call::<()>((canvas_handle, ctx))
                {
                    self.last_error = Some(e.to_string());
                }
            }

            if let Some(limit_ms) = *self.shared.duration_limit_ms.lock().unwrap()
                && self.shared.elapsed_ms >= limit_ms
            {
                *self.shared.finished.lock().unwrap() = true;
                *self.shared.reveal_progress.lock().unwrap() = 1.0;
                self.shared.canvas_buf.lock().unwrap().begin_frame();
            }
        }
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let guard = self.shared.canvas_buf.lock().unwrap();
        guard.blit_to_buffer(buf, area, Color::White);
    }

    /// Renders a `Boxed` animation cleanly inside an isolated, bordered modal panel
    /// so zero characters can ever bleed over other dashboard containers or borders.
    pub fn render_boxed(&self, frame: &mut Frame, screen: Rect, theme: &Theme) {
        let popup = self.boxed_rect(screen).intersection(screen);
        if popup.width < 6 || popup.height < 5 {
            return;
        }
        frame.render_widget(Clear, popup);
        let title_str = self
            .shared
            .box_title
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.active_id.to_lowercase());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(theme.borders.as_ratatui_set())
            .border_style(Style::default().fg(theme.primary))
            .title(Line::from(Span::styled(
                format!(" {title_str} "),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().fg(theme.text).bg(theme.panel_background));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        if inner.width > 0 && inner.height > 0 {
            self.render(frame.buffer_mut(), inner);
        }
    }
}

fn parse_animation_layer(s: &str) -> AnimationLayer {
    match s.trim().to_ascii_lowercase().as_str() {
        "background" | "bg" => AnimationLayer::Background,
        "boxed" | "box" | "window" | "panel" => AnimationLayer::Boxed,
        _ => AnimationLayer::Overlay,
    }
}
