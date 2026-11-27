use crate::animation::EasingCurve;
use crate::config::paths;
use crate::lua::configure_lua_vm;
use crate::ui::theme::{blend_color, parse_color};
use mlua::{Function, Lua, RegistryKey, Table, UserData, UserDataMethods, Value};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LuaSceneElement {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub lines: Vec<String>,
    pub fg: Color,
    pub bg: Option<Color>,
    pub bold: bool,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct GreetingCellTransform {
    pub ch: Option<String>,
    pub fg: Option<Color>,
    pub dy: i16,
    pub bold: bool,
}

#[derive(Clone)]
struct ElementHandle {
    id: String,
    elements: Arc<Mutex<Vec<LuaSceneElement>>>,
}

impl UserData for ElementHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("move", |_, this, (x, y): (i32, i32)| {
            let mut list = this.elements.lock().unwrap();
            if let Some(el) = list.iter_mut().find(|e| e.id == this.id) {
                el.x = x;
                el.y = y;
            }
            Ok(())
        });
        methods.add_method("set_text", |_, this, text: String| {
            let mut list = this.elements.lock().unwrap();
            if let Some(el) = list.iter_mut().find(|e| e.id == this.id) {
                el.lines = text.lines().map(|l| l.to_string()).collect();
                if el.lines.is_empty() && !text.is_empty() {
                    el.lines.push(text);
                }
            }
            Ok(())
        });
        methods.add_method("set_lines", |_, this, lines: Vec<String>| {
            let mut list = this.elements.lock().unwrap();
            if let Some(el) = list.iter_mut().find(|e| e.id == this.id) {
                el.lines = lines;
            }
            Ok(())
        });
        methods.add_method(
            "set_color",
            |_, this, (fg, bg): (Option<String>, Option<String>)| {
                let mut list = this.elements.lock().unwrap();
                if let Some(el) = list.iter_mut().find(|e| e.id == this.id) {
                    if let Some(f) = fg {
                        el.fg = parse_color(&f, el.fg);
                    }
                    if let Some(b) = bg {
                        el.bg = Some(parse_color(&b, Color::Reset));
                    }
                }
                Ok(())
            },
        );
        methods.add_method("set_visible", |_, this, visible: bool| {
            let mut list = this.elements.lock().unwrap();
            if let Some(el) = list.iter_mut().find(|e| e.id == this.id) {
                el.visible = visible;
            }
            Ok(())
        });
        methods.add_method("destroy", |_, this, ()| {
            let mut list = this.elements.lock().unwrap();
            list.retain(|e| e.id != this.id);
            Ok(())
        });
    }
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
    screen_w: u16,
    screen_h: u16,
    elapsed_ms: f32,
    dt_secs: f32,
    reveal_progress: Arc<Mutex<f32>>,
    finished: Arc<Mutex<bool>>,
    paused: bool,
    elements: Arc<Mutex<Vec<LuaSceneElement>>>,
    every_tasks: Arc<Mutex<Vec<ScheduledEvery>>>,
    after_tasks: Arc<Mutex<Vec<ScheduledAfter>>>,
    tweens: Arc<Mutex<Vec<ActiveTween>>>,
    sequences: Arc<Mutex<Vec<ActiveSequence>>>,
    greeting_fn: Arc<Mutex<Option<RegistryKey>>>,
    next_elem_id: Arc<Mutex<usize>>,
}

/// Robust, throttled, non-blocking Lua Animation Runtime hosting animations with `setup(api)`.
/// Supports arbitrary durations, multi-element scenes, sequences, tweens, scheduled tasks,
/// and per-character greeting transforms without blocking the UI event loop.
pub struct LuaAnimationRuntime {
    pub active_id: String,
    lua: Option<Lua>,
    shared: RuntimeShared,
    target_fps: u32,
    accum_dt: f32,
    pub last_error: Option<String>,
}

impl LuaAnimationRuntime {
    pub fn start(name: &str, screen_w: u16, screen_h: u16, target_fps: u32) -> Self {
        let mut rt = Self {
            active_id: name.to_string(),
            lua: None,
            shared: RuntimeShared {
                screen_w,
                screen_h,
                elapsed_ms: 0.0,
                dt_secs: 0.0,
                reveal_progress: Arc::new(Mutex::new(0.0)),
                finished: Arc::new(Mutex::new(false)),
                paused: false,
                elements: Arc::new(Mutex::new(Vec::new())),
                every_tasks: Arc::new(Mutex::new(Vec::new())),
                after_tasks: Arc::new(Mutex::new(Vec::new())),
                tweens: Arc::new(Mutex::new(Vec::new())),
                sequences: Arc::new(Mutex::new(Vec::new())),
                greeting_fn: Arc::new(Mutex::new(None)),
                next_elem_id: Arc::new(Mutex::new(1)),
            },
            target_fps: target_fps.clamp(1, 60),
            accum_dt: 0.0,
            last_error: None,
        };
        rt.load_and_setup(name);
        rt
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
        self.shared.elements.lock().unwrap().clear();
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.shared.screen_w = w;
        self.shared.screen_h = h;
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
            if custom_path.exists() {
                let raw = std::fs::read_to_string(&custom_path).unwrap_or_default();
                let val: Value = lua
                    .load(&raw)
                    .set_name(custom_path.display().to_string())
                    .eval()?;
                if let Value::Table(t) = val
                    && let Ok(f) = t.get::<Function>("setup")
                {
                    f.call::<()>(api)?;
                }
            } else {
                let require: Function = lua.globals().get("require")?;
                let mod_name = format!("animations.{normalized}");
                let mod_tbl: Table = require.call(mod_name.as_str())?;
                let setup: Function = mod_tbl.get("setup")?;
                setup.call::<()>(api)?;
            }
            Ok(())
        })();

        if let Err(e) = res {
            self.last_error = Some(format!("Lua animation '{name}' error: {e}"));
            self.stop();
            return;
        }
        self.lua = Some(lua);
    }

    fn build_api_table(&self, lua: &Lua) -> mlua::Result<Table> {
        let api = lua.create_table()?;
        let shared = self.shared.clone();

        // api.screen.width() / height()
        let screen = lua.create_table()?;
        {
            let sw = shared.screen_w;
            let sh = shared.screen_h;
            screen.set("width", lua.create_function(move |_, ()| Ok(sw))?)?;
            screen.set("height", lua.create_function(move |_, ()| Ok(sh))?)?;
        }
        api.set("screen", screen)?;

        // api.elapsed_ms() / api.dt() / api.finish()
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

        let finished_flag = Arc::clone(&shared.finished);
        {
            let ff = Arc::clone(&finished_flag);
            let el = Arc::clone(&shared.elements);
            let rp = Arc::clone(&shared.reveal_progress);
            api.set(
                "finish",
                lua.create_function(move |_, ()| {
                    *ff.lock().unwrap() = true;
                    *rp.lock().unwrap() = 1.0;
                    el.lock().unwrap().clear();
                    Ok(())
                })?,
            )?;
        }

        // api.figlet(text, font_name)
        api.set(
            "figlet",
            lua.create_function(|_, (text, font_name): (String, Option<String>)| {
                let fname = font_name.as_deref().unwrap_or("Standard");
                let lines = crate::ui::typography::render_figlet_lines(&text, fname, false);
                Ok(lines)
            })?,
        )?;

        // api.lerp_color(c1, c2, t)
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

        // api.element({ id, x, y, text, lines, fg, bg, bold, visible })
        {
            let els = Arc::clone(&shared.elements);
            let next_id = Arc::clone(&shared.next_elem_id);
            api.set(
                "element",
                lua.create_function(move |_, spec: Table| {
                    let mut counter = next_id.lock().unwrap();
                    let id = spec
                        .get::<String>("id")
                        .unwrap_or_else(|_| format!("elem_{}", *counter));
                    *counter += 1;
                    let x = spec.get::<i32>("x").unwrap_or(0);
                    let y = spec.get::<i32>("y").unwrap_or(0);
                    let lines = if let Ok(lv) = spec.get::<Vec<String>>("lines") {
                        lv
                    } else if let Ok(txt) = spec.get::<String>("text") {
                        let mut v: Vec<String> = txt.lines().map(|l| l.to_string()).collect();
                        if v.is_empty() && !txt.is_empty() {
                            v.push(txt);
                        }
                        v
                    } else {
                        Vec::new()
                    };
                    let fg = spec
                        .get::<String>("fg")
                        .map(|s| parse_color(&s, Color::White))
                        .unwrap_or(Color::White);
                    let bg = spec
                        .get::<Option<String>>("bg")
                        .unwrap_or(None)
                        .map(|s| parse_color(&s, Color::Reset));
                    let bold = spec.get::<bool>("bold").unwrap_or(false);
                    let visible = spec.get::<bool>("visible").unwrap_or(true);

                    els.lock().unwrap().push(LuaSceneElement {
                        id: id.clone(),
                        x,
                        y,
                        lines,
                        fg,
                        bg,
                        bold,
                        visible,
                    });
                    Ok(ElementHandle {
                        id,
                        elements: Arc::clone(&els),
                    })
                })?,
            )?;
        }

        // api.every(interval_ms, fn)
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

        // api.after(delay_ms, fn)
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

        // api.tween({ from, to, duration_ms, easing, on_update, on_complete })
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

        // api.sequence(steps, on_complete)
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

        // api.parallel(fns)
        api.set(
            "parallel",
            lua.create_function(|_, tasks: Table| {
                for f in tasks.sequence_values::<Function>().flatten() {
                    let _ = f.call::<()>(());
                }
                Ok(())
            })?,
        )?;

        // api.ui.reveal(progress) & api.ui.set_greeting_transform(fn)
        let ui = lua.create_table()?;
        {
            let rc = Arc::clone(&shared.reveal_progress);
            ui.set(
                "reveal",
                lua.create_function(move |_, p: f32| {
                    *rc.lock().unwrap() = p.clamp(0.0, 1.0);
                    Ok(())
                })?,
            )?;
        }
        {
            let gfn = Arc::clone(&shared.greeting_fn);
            ui.set(
                "set_greeting_transform",
                lua.create_function(move |lua, cb: Option<Function>| {
                    let mut guard = gfn.lock().unwrap();
                    *guard = cb.and_then(|f| lua.create_registry_value(f).ok());
                    Ok(())
                })?,
            )?;
        }
        api.set("ui", ui)?;

        Ok(api)
    }

    pub fn transform_greeting_char(
        &self,
        ch: char,
        col: usize,
        row: usize,
    ) -> Option<GreetingCellTransform> {
        let lua = self.lua.as_ref()?;
        let key_guard = self.shared.greeting_fn.lock().unwrap();
        let key = key_guard.as_ref()?;
        let f = lua.registry_value::<Function>(key).ok()?;
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let val: Value = f.call((s.to_string(), col, row)).ok()?;
        let Value::Table(tbl) = val else {
            return None;
        };
        let out_ch = tbl.get::<Option<String>>("char").unwrap_or(None);
        let fg = tbl
            .get::<Option<String>>("fg")
            .unwrap_or(None)
            .map(|c| parse_color(&c, Color::White));
        let dy = tbl.get::<i16>("dy").unwrap_or(0);
        let bold = tbl.get::<bool>("bold").unwrap_or(false);
        Some(GreetingCellTransform {
            ch: out_ch,
            fg,
            dy,
            bold,
        })
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
    }

    pub fn render(&self, buf: &mut Buffer, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let elements = self.shared.elements.lock().unwrap();
        for el in elements.iter() {
            if !el.visible {
                continue;
            }
            for (row_offset, line) in el.lines.iter().enumerate() {
                let y = el.y + row_offset as i32;
                if y < area.y as i32 || y >= area.bottom() as i32 {
                    continue;
                }
                for (col_offset, ch) in line.chars().enumerate() {
                    let x = el.x + col_offset as i32;
                    if x < area.x as i32 || x >= area.right() as i32 {
                        continue;
                    }
                    if let Some(cell) = buf.cell_mut((x as u16, y as u16)) {
                        let mut s = [0u8; 4];
                        cell.set_symbol(ch.encode_utf8(&mut s));
                        cell.set_fg(el.fg);
                        if let Some(bg) = el.bg {
                            cell.set_bg(bg);
                        }
                        if el.bold {
                            cell.modifier.insert(Modifier::BOLD);
                        }
                    }
                }
            }
        }
    }
}
