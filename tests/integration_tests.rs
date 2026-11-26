use egotop::animation::{
    AnimationCatalog, BackgroundEffect, NativeTransition, TransitionPlayer, create_effect,
    resolve_region,
};
use egotop::command::{
    CommandLineState, LayoutSelection, ParsedCommand, WindowCommand, parse_command,
};
use egotop::config::{
    Anchor, BackgroundConfig, BorderStyle, Config, DeclarativeAnimation, EffectKind, LayoutMode,
    RegionConfig, RegionUnit, SizeSpec, ThemeEntry, ViewMode, WidgetConfig, WidgetId,
    load_from_path, load_or_create,
};
use egotop::system::SystemCollector;
use egotop::system::os::parse_os_release;
use egotop::system::uptime::format_uptime;
use egotop::terminal::renderer::draw_frame;
use egotop::ui::RevealState;
use egotop::ui::customizer::CustomizerState;
use egotop::ui::fonts::{FigletFont, available_fonts, load_font, normalize_ascii_text};
use egotop::ui::layout::compute_layout;
use egotop::ui::theme::{Theme, parse_color};
use egotop::ui::typography::render_ascii_heading;
use egotop::ui::widgets::RenderContext;
use egotop::window::{PositionMode, WindowBackgroundMode, WindowConfig, WindowManager, resolve_window_rect};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_config_path(suffix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("egotop_test_{suffix}_{nanos}.toml"))
}

#[test]
fn config_creates_default_and_loads_valid_toml() {
    let path = temp_config_path("create_default");
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let first = load_or_create(Some(&path)).expect("should create default config");
    assert!(first.created_default);
    assert!(path.exists());
    assert_eq!(first.config.general.name, "egotop");

    let second = load_or_create(Some(&path)).expect("should load existing config");
    assert!(!second.created_default);
    assert_eq!(first.config, second.config);
    assert_eq!(first.config, Config::default());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn config_rejects_invalid_toml_and_out_of_range_values() {
    let path = temp_config_path("invalid");
    std::fs::write(&path, "[general]\nanimation_fps = 0\n").unwrap();
    let err = load_from_path(&path).unwrap_err();
    assert!(err.to_string().contains("animation_fps"));

    std::fs::write(&path, "[theme]\nprimary = \"not-a-color\"\n").unwrap();
    let err = load_from_path(&path).unwrap_err();
    assert!(err.to_string().contains("theme.primary"));

    std::fs::write(&path, "[[widgets]\nmissing_bracket = true\n").unwrap();
    let err = load_from_path(&path).unwrap_err();
    assert!(err.to_string().contains("Invalid TOML syntax"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn window_engine_anchors_drag_resize_and_clamping() {
    let viewport = Rect::new(0, 0, 120, 40);
    let mut cfg = WindowConfig {
        anchor: Anchor::TopLeft,
        width: SizeSpec::Cells(60),
        height: SizeSpec::Cells(20),
        ..WindowConfig::default()
    };
    assert_eq!(resolve_window_rect(&cfg, viewport), Rect::new(0, 0, 60, 20));

    cfg.anchor = Anchor::BottomRight;
    assert_eq!(resolve_window_rect(&cfg, viewport), Rect::new(60, 20, 60, 20));

    let mut mgr = WindowManager::new(cfg);
    mgr.resolve_all(viewport);
    let top_row = mgr.main_window().resolved_rect.y;
    let left_col = mgr.main_window().resolved_rect.x + 5;
    assert!(mgr.begin_mouse_down(left_col, top_row, viewport));
    assert!(mgr.update_mouse_drag(left_col - 10, top_row - 5, viewport));
    mgr.end_mouse_drag();
    assert_eq!(mgr.main_window().config.position, PositionMode::Absolute);
    assert_eq!(mgr.main_window().resolved_rect.x, 50);
    assert_eq!(mgr.main_window().resolved_rect.y, 15);

    // Resize via bottom-right corner
    let r = mgr.main_window().resolved_rect;
    assert!(mgr.begin_mouse_down(r.right() - 1, r.bottom() - 1, viewport));
    assert!(mgr.update_mouse_drag(r.right() + 5, r.bottom() + 2, viewport));
    mgr.end_mouse_drag();
    assert!(mgr.main_window().resolved_rect.width >= 60);
}

#[test]
fn command_engine_parses_and_autocompletes() {
    assert_eq!(
        parse_command(":colorscheme bamboo").unwrap(),
        ParsedCommand::Colorscheme(Some("bamboo".to_string()))
    );
    assert_eq!(parse_command("reload").unwrap(), ParsedCommand::Reload);
    assert_eq!(
        parse_command(":animation crt_power_on").unwrap(),
        ParsedCommand::Animation(Some("crt_power_on".to_string()))
    );
    assert_eq!(
        parse_command(":window border solid").unwrap(),
        ParsedCommand::Window(WindowCommand::SetBorderStyle(BorderStyle::Plain))
    );
    assert_eq!(
        parse_command(":window bg transparent").unwrap(),
        ParsedCommand::Window(WindowCommand::SetBackground(WindowBackgroundMode::Transparent))
    );
    assert_eq!(
        parse_command(":layout compact").unwrap(),
        ParsedCommand::Layout(Some(LayoutSelection::View(ViewMode::Compact)))
    );
    assert_eq!(parse_command(":q").unwrap(), ParsedCommand::Quit);

    let mut cli = CommandLineState::default();
    cli.open();
    for ch in "colorscheme ba".chars() {
        cli.push_char(ch);
    }
    cli.autocomplete(&["matrix".into(), "bamboo".into()], &[]);
    assert_eq!(cli.buffer, "colorscheme bamboo");
    assert_eq!(cli.submit(), "colorscheme bamboo");
    assert!(!cli.active);
}

#[test]
fn animation_engine_native_and_declarative_toml() {
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    let catalog = AnimationCatalog::load();

    for native in NativeTransition::all() {
        let mut player = TransitionPlayer::new(native.id(), &catalog, 600);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        player.update(0.2);
        player.apply_to_buffer(&mut buf, Rect::new(5, 2, 70, 20), &theme);
        player.update(1.0);
        assert!(player.finished);
    }

    let sample_toml = include_str!("../assets/animations/cyber_boot.toml");
    let spec: DeclarativeAnimation = toml::from_str(sample_toml).unwrap();
    assert!(spec.validate().is_ok());
    assert_eq!(spec.schema_version, 1);
    assert_eq!(spec.steps.len(), 3);
}

#[test]
fn full_dashboard_renders_via_test_backend() {
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    let mut collector = SystemCollector::new();
    let snapshot = collector.collect(120, 36);
    let mut effect = create_effect(EffectKind::Matrix, 120, 36, &cfg.background);
    effect.update(0.05, &cfg.background);

    let mut windows = WindowManager::new(WindowConfig::default());
    let catalog = AnimationCatalog::load();
    let transition = TransitionPlayer::new("none", &catalog, 0);
    let mut customizer = CustomizerState::default();
    let mut cmd_line = CommandLineState::default();
    let themes = vec![ThemeEntry {
        name: "matrix".to_string(),
        theme: theme.clone(),
        source_path: None,
    }];

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    for page in [1u8, 2u8, 3u8] {
        for mode in [ViewMode::Dashboard, ViewMode::Compact, ViewMode::Minimal] {
            let reveal = RevealState::complete();
            let ctx = RenderContext {
                config: &cfg,
                theme: &theme,
                snapshot: &snapshot,
                reveal: &reveal,
                active_page: page,
                view_mode: mode,
                paused: false,
                show_help: false,
                fps: 30.0,
                status_toast: None,
                window_transparent: false,
            };
            terminal
                .draw(|frame| {
                    draw_frame(
                        frame,
                        &cfg,
                        effect.as_ref(),
                        &transition,
                        &mut windows,
                        &mut customizer,
                        &cmd_line,
                        &themes,
                        &catalog,
                        &ctx,
                    );
                })
                .unwrap();
        }
    }

    cmd_line.open();
    cmd_line.push_char('h');
    let reveal = RevealState::complete();
    let ctx = RenderContext {
        config: &cfg,
        theme: &theme,
        snapshot: &snapshot,
        reveal: &reveal,
        active_page: 1,
        view_mode: ViewMode::Dashboard,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        window_transparent: false,
    };
    terminal
        .draw(|frame| {
            draw_frame(
                frame,
                &cfg,
                effect.as_ref(),
                &transition,
                &mut windows,
                &mut customizer,
                &cmd_line,
                &themes,
                &catalog,
                &ctx,
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut dumped = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            dumped.push_str(buf[(x, y)].symbol());
        }
        dumped.push('\n');
    }
    assert!(dumped.contains("Sistema"));
    assert!(dumped.contains("Hardware"));
    assert!(dumped.contains("Atividade"));
    assert!(dumped.contains(":h"));
}

#[test]
fn system_collector_reads_real_linux_data() {
    let mut collector = SystemCollector::new();
    let snap1 = collector.collect(120, 40);
    std::thread::sleep(std::time::Duration::from_millis(60));
    let snap2 = collector.collect(120, 40);

    assert!(snap2.os.kernel.is_some());
    assert!(snap2.os.arch.is_some());
    assert!(snap2.hardware.cpu.cores >= 1);
    assert!(snap2.hardware.memory.total_bytes.unwrap_or(0) > 0);
    assert!(snap2.activity.uptime_secs.is_some());
    assert!(snap2.activity.process_count.unwrap_or(0) > 0);
    assert_eq!(snap2.session.terminal_cols, 120);
    assert_eq!(snap2.session.terminal_rows, 40);
    assert!(snap1.os.username.is_some());
}

#[test]
fn parse_os_release_and_uptime_formatting() {
    let sample = r#"
NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
"#;
    let (pretty, id) = parse_os_release(sample);
    assert_eq!(pretty.as_deref(), Some("Arch Linux"));
    assert_eq!(id.as_deref(), Some("arch"));
    assert_eq!(format_uptime(3665), "1h 1m 5s");
    assert_eq!(format_uptime(90_060), "1d 1h 1m");
}

#[test]
fn layout_adapts_to_resizes_and_respects_z_index_and_anchors() {
    let mut cfg = Config {
        widgets: vec![
            WidgetConfig {
                id: WidgetId::System,
                enabled: true,
                position: Anchor::TopRight,
                width: Some(SizeSpec::Percent(40)),
                z_index: 50,
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::Hardware,
                enabled: true,
                position: Anchor::TopLeft,
                width: Some(SizeSpec::Percent(60)),
                z_index: 10,
                ..WidgetConfig::default()
            },
            WidgetConfig {
                id: WidgetId::Activity,
                enabled: false,
                ..WidgetConfig::default()
            },
        ],
        ..Config::default()
    };

    let wide = Rect::new(0, 0, 140, 40);
    let l_wide = compute_layout(wide, &cfg, 1, ViewMode::Dashboard);
    assert_eq!(l_wide.widgets.len(), 2);
    assert_eq!(l_wide.widgets[0].config.id, WidgetId::Hardware);
    assert_eq!(l_wide.widgets[1].config.id, WidgetId::System);
    assert!(l_wide.widgets[0].area.x < l_wide.widgets[1].area.x);

    for (w, h) in [(20, 8), (45, 16), (80, 24), (200, 60)] {
        let screen = Rect::new(0, 0, w, h);
        let l = compute_layout(screen, &cfg, 1, ViewMode::Dashboard);
        for placed in l.widgets {
            assert!(placed.area.right() <= screen.right());
            assert!(placed.area.bottom() <= screen.bottom());
        }
    }

    cfg.layout.mode = LayoutMode::Free;
    let l_free = compute_layout(Rect::new(0, 0, 100, 40), &cfg, 1, ViewMode::Dashboard);
    let hw = l_free
        .widgets
        .iter()
        .find(|w| w.config.id == WidgetId::Hardware)
        .unwrap();
    let sys = l_free
        .widgets
        .iter()
        .find(|w| w.config.id == WidgetId::System)
        .unwrap();
    assert!(hw.area.x < sys.area.x);
}

#[test]
fn typography_renders_all_builtin_fonts_and_flf_parser() {
    assert_eq!(normalize_ascii_text("Olá, Kiam!"), "Ola, Kiam!");

    for font_name in available_fonts() {
        let font = load_font(font_name, false);
        let lines = font.render_lines("Olá, Kiam!");
        assert!(!lines.is_empty(), "font {font_name} produced empty output");
        assert!(lines.len() >= 3);
    }

    let narrow = render_ascii_heading("Ola, Kiam!", "ANSI Shadow", false, 18);
    assert_eq!(narrow, vec!["Ola, Kiam!"]);

    let mini_flf = "flf2a$ 2 2 5 0 1\ncomment\nA@\nA@@\nB@\nB@@\n";
    let parsed = FigletFont::from_flf_str("mini", mini_flf);
    assert!(parsed.is_some());

    let mut reveal = RevealState::new(1.0);
    assert!(!reveal.is_done());
    let partial = reveal.mask_line("Hello", 0);
    assert_eq!(partial.chars().count(), 5);
    reveal.advance(1.5);
    assert!(reveal.is_done());
    assert_eq!(reveal.mask_line("Hello", 0), "Hello");
}

#[test]
fn animations_update_and_render_within_bounds_and_regions() {
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    assert_eq!(parse_color("#39FF88", Color::Reset), Color::Rgb(57, 255, 136));

    let area = Rect::new(0, 0, 80, 24);
    let region = RegionConfig {
        x: 25,
        y: 25,
        width: 50,
        height: 50,
        unit: RegionUnit::Percent,
    };
    let resolved = resolve_region(area, Some(&region), None);
    assert_eq!(resolved, Rect::new(20, 6, 40, 12));

    let mut bg_cfg = BackgroundConfig::default();
    bg_cfg.matrix.enabled = true;
    bg_cfg.water.enabled = true;
    bg_cfg.particles.enabled = true;
    bg_cfg.scanlines.enabled = true;

    for kind in [
        EffectKind::Matrix,
        EffectKind::Water,
        EffectKind::Particles,
        EffectKind::Scanlines,
        EffectKind::None,
    ] {
        let mut eff: Box<dyn BackgroundEffect> = create_effect(kind, 80, 24, &bg_cfg);
        assert_eq!(eff.kind(), kind);
        for _ in 0..10 {
            eff.update(0.033, &bg_cfg);
        }
        let mut buf = Buffer::empty(area);
        eff.render(&mut buf, area, &bg_cfg, &theme);
        eff.resize(40, 12, &bg_cfg);
        eff.update(0.033, &bg_cfg);
    }
}
