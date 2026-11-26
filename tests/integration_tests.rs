use egotop::animation::{
    AnimationCatalog, AnimationCatalogEntry, AnimationSource, BackgroundEffect, NativeTransition,
    TransitionPlayer, create_effect, resolve_region,
};
use egotop::command::{
    AnimationAction, CommandLineState, CommandRegistry, CompletionContext, CompletionEngine,
    LayoutSelection, ParsedCommand, WindowCommand, WriteTarget, parse_command,
};
use egotop::config::{
    Anchor, BackgroundConfig, BorderStyle, Config, DeclarativeAnimation, EffectKind, LayoutMode,
    RegionConfig, RegionUnit, SizeSpec, StartupMode, ThemeEntry, ViewMode, WidgetConfig, WidgetId,
    load_from_path, load_or_create,
};
use egotop::system::SystemCollector;
use egotop::system::os::parse_os_release;
use egotop::system::uptime::format_uptime;
use egotop::terminal::renderer::{CursorStartupState, draw_frame};
use egotop::ui::RevealState;
use egotop::ui::borders::{BorderChars, parse_border_style, validate_border_char};
use egotop::ui::customizer::{CustomizerState, CustomizerTab, render_customizer};
use egotop::ui::fonts::{FigletFont, available_fonts, load_font, normalize_ascii_text};
use egotop::ui::layout::compute_layout;
use egotop::ui::theme::{Theme, parse_color};
use egotop::ui::typography::render_ascii_heading;
use egotop::ui::widgets::{
    RenderContext, allocate_three_column_widths, pad_display_width, render_command_line,
    truncate_display_width,
};
use unicode_width::UnicodeWidthStr;
use egotop::window::{
    PositionMode, WindowBackgroundMode, WindowConfig, WindowManager, resolve_window_rect,
};
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
fn config_creates_default_and_loads_valid_toml_in_english() {
    let path = temp_config_path("create_default");
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }

    let first = load_or_create(Some(&path)).expect("should create default config");
    assert!(first.created_default);
    assert!(path.exists());
    assert_eq!(first.config.general.name, "egotop");
    assert_eq!(first.config.general.greeting, "Hello, {user}!");
    assert_eq!(first.config.startup.mode, StartupMode::Normal);

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

    std::fs::write(&path, "[theme.borders]\nhorizontal = \"❌\"\n").unwrap();
    let err = load_from_path(&path).unwrap_err();
    assert!(err.to_string().contains("theme.borders.horizontal"));

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

    let r = mgr.main_window().resolved_rect;
    assert!(mgr.begin_mouse_down(r.right() - 1, r.bottom() - 1, viewport));
    assert!(mgr.update_mouse_drag(r.right() + 5, r.bottom() + 2, viewport));
    mgr.end_mouse_drag();
    assert!(mgr.main_window().resolved_rect.width >= 60);
}

#[test]
fn command_parser_and_contextual_completion_engine() {
    // Scenarios 8 & 9: :w / :write and :q / :quit (and :save must NOT exist)
    assert_eq!(parse_command(":w").unwrap(), ParsedCommand::Write(WriteTarget::All));
    assert_eq!(parse_command(":write").unwrap(), ParsedCommand::Write(WriteTarget::All));
    assert_eq!(
        parse_command(":write window").unwrap(),
        ParsedCommand::Write(WriteTarget::Window)
    );
    assert_eq!(parse_command(":q").unwrap(), ParsedCommand::Quit);
    assert_eq!(parse_command(":quit").unwrap(), ParsedCommand::Quit);
    assert!(parse_command(":save").is_err());

    assert_eq!(
        parse_command(":colorscheme bamboo").unwrap(),
        ParsedCommand::Colorscheme(Some("bamboo".to_string()))
    );
    assert_eq!(parse_command("reload").unwrap(), ParsedCommand::Reload);
    assert_eq!(
        parse_command(":set border.horizontal ━").unwrap(),
        ParsedCommand::Set {
            key: "border.horizontal".to_string(),
            value: "━".to_string(),
        }
    );
    assert_eq!(
        parse_command(":animation play crt_power_on").unwrap(),
        ParsedCommand::Animation {
            action: AnimationAction::Play,
            name: Some("crt_power_on".to_string()),
        }
    );
    assert_eq!(
        parse_command(":window border plain").unwrap(),
        ParsedCommand::Window(WindowCommand::SetBorderStyle(BorderStyle::Plain))
    );
    assert_eq!(
        parse_command(":window bg transparent").unwrap(),
        ParsedCommand::Window(WindowCommand::SetBackground(WindowBackgroundMode::Transparent))
    );
    assert_eq!(
        parse_command(":layout load compact").unwrap(),
        ParsedCommand::Layout(Some(LayoutSelection::View(ViewMode::Compact)))
    );

    let registry = CommandRegistry::new();
    assert!(registry.find("save").is_none());
    assert_eq!(registry.find("w").unwrap().name, "write");
    assert_eq!(registry.find("q").unwrap().name, "quit");

    let cfg = Config::default();
    let base_theme = Theme::from_config(&cfg.theme);
    let themes = vec![
        ThemeEntry {
            name: "matrix".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        ThemeEntry {
            name: "bamboo".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        ThemeEntry {
            name: "nord".to_string(),
            theme: base_theme,
            source_path: None,
        },
    ];
    let animations = AnimationCatalog::load();
    let layouts = vec!["my_grid".to_string()];
    let ctx = CompletionContext {
        themes: &themes,
        animations: &animations,
        layouts: &layouts,
    };

    // No duplicate entries for command aliases (:w / :write, :q / :quit)
    let all_cmds = CompletionEngine::complete("", 0, &registry, &ctx);
    assert_eq!(
        all_cmds
            .items
            .iter()
            .filter(|i| i.insert_text == "write")
            .count(),
        1
    );
    assert_eq!(
        all_cmds
            .items
            .iter()
            .filter(|i| i.insert_text == "quit")
            .count(),
        1
    );
    assert!(!all_cmds.items.iter().any(|i| i.insert_text == "save"));

    // Contextual completions
    let res = CompletionEngine::complete("animation play crt", 18, &registry, &ctx);
    assert!(res.items.iter().any(|i| i.insert_text == "crt_power_on"));

    let res = CompletionEngine::complete("layout load my_", 15, &registry, &ctx);
    assert!(res.items.iter().any(|i| i.insert_text == "my_grid"));

    let res = CompletionEngine::complete("set border.", 11, &registry, &ctx);
    assert!(res.items.iter().any(|i| i.insert_text == "border.horizontal"));
    assert!(res.items.iter().any(|i| i.insert_text == "border.style"));

    let res = CompletionEngine::complete("set border.style ", 17, &registry, &ctx);
    assert!(res.items.iter().any(|i| i.insert_text == "double"));

    let res = CompletionEngine::complete("set background ", 15, &registry, &ctx);
    assert_eq!(res.items.len(), 2);

    // Scenarios 1, 2, 3, 4, 5: Shell-style Tab / Shift+Tab / Space without Enter
    let mut cli = CommandLineState::default();
    cli.open(&registry, &ctx);
    for ch in "color".chars() {
        cli.insert_char(ch, &registry, &ctx);
    }
    // Scenario 1: Tab immediately completes `:color` -> `:colorscheme`
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme");

    // Scenario 4: Space commits token and offers themes
    cli.insert_char(' ', &registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme ");
    assert_eq!(cli.suggestions.len(), 3);
    assert_eq!(cli.suggestions[0].insert_text, "matrix");

    // Scenarios 2, 3, 5: Cycle forward & backward through themes with Tab / Shift+Tab
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme matrix");
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme bamboo");
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme nord");
    cli.prev_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme bamboo");
    cli.prev_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme matrix");

    // Scenario 6: Edit partially completed command manually
    cli.move_left(&registry, &ctx);
    cli.backspace(&registry, &ctx);
    cli.insert_char('x', &registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme matrxx");

    // Scenario 7: Enter executes directly via submit()
    let submitted = cli.submit();
    assert_eq!(submitted, "colorscheme matrxx");
    assert!(!cli.active);

    // History recall when reopening with empty buffer
    cli.open(&registry, &ctx);
    cli.handle_up(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme matrxx");
}

#[test]
fn border_chars_validation_and_runtime_application() {
    assert!(validate_border_char("━").is_ok());
    assert!(validate_border_char("╔").is_ok());
    // Reject empty, multi-char, control, zero-width, and double-width characters
    assert!(validate_border_char("").is_err());
    assert!(validate_border_char("ab").is_err());
    assert!(validate_border_char("\n").is_err());
    assert!(validate_border_char("\u{200B}").is_err());
    assert!(validate_border_char("🔥").is_err());

    let mut borders = BorderChars::from_style(BorderStyle::Rounded);
    assert_eq!(borders.top_left, "╭");
    borders.set_property("horizontal", "━").unwrap();
    assert_eq!(borders.horizontal, "━");
    // Invalid update must fail and keep previous state
    assert!(borders.set_property("vertical", "🔥🔥").is_err());
    assert_eq!(borders.vertical, "│");

    assert_eq!(parse_border_style("solid").unwrap(), BorderStyle::Plain);
    assert_eq!(parse_border_style("double").unwrap(), BorderStyle::Double);
}

#[test]
fn animation_engine_native_cursor_expand_and_declarative_toml() {
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    let catalog = AnimationCatalog::load();

    for native in NativeTransition::all() {
        let mut player = TransitionPlayer::new(native.id(), &catalog, 600);
        player.origin = Some((40, 12));
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
fn full_dashboard_and_cursor_startup_render_via_test_backend() {
    let mut cfg = Config::default();
    let mut theme = Theme::from_config(&cfg.theme);
    theme.borders.set_property("horizontal", "━").unwrap();

    let mut collector = SystemCollector::new();
    let snapshot = collector.collect(120, 36);
    let mut effect = create_effect(EffectKind::Matrix, 120, 36, &cfg.background);
    effect.update(0.05, &cfg.background);

    let mut windows = WindowManager::new(WindowConfig::default());
    let catalog = AnimationCatalog::load();
    let transition = TransitionPlayer::new("none", &catalog, 0);
    let mut customizer = CustomizerState::default();
    let registry = CommandRegistry::new();
    let mut cmd_line = CommandLineState::default();
    let themes = vec![ThemeEntry {
        name: "matrix".to_string(),
        theme: theme.clone(),
        source_path: None,
    }];
    let layouts = vec![];
    let cctx = CompletionContext {
        themes: &themes,
        animations: &catalog,
        layouts: &layouts,
    };

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Cursor startup mode renders only the blinking cursor (no dashboard panels)
    cfg.startup.mode = StartupMode::Cursor;
    let mut startup = CursorStartupState::new(true);
    startup.pos = Some((30, 15));
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
                &startup,
                &themes,
                &catalog,
                &ctx,
            );
        })
        .unwrap();

    let cursor_buf = terminal.backend().buffer();
    assert_eq!(cursor_buf[(30, 15)].symbol(), "▌");

    // 2. Normal dashboard + autocomplete popup + custom border character '━'
    startup.pending = false;
    cmd_line.open(&registry, &cctx);
    cmd_line.insert_char('c', &registry, &cctx);
    cmd_line.insert_char('o', &registry, &cctx);

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
                &startup,
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
    assert!(dumped.contains("System"));
    assert!(dumped.contains("Hardware"));
    assert!(dumped.contains("Activity"));
    assert!(dumped.contains("Session & Uptime"));
    assert!(dumped.contains(":colorscheme"));
    assert!(dumped.contains("━"));

    // Scenarios 10, 11, 12: Runtime theme change + border char change + terminal resize with popup open
    let mut nord_theme = egotop::ui::theme::preset_theme("nord");
    nord_theme.borders.set_property("top_left", "╔").unwrap();
    nord_theme.borders.set_property("horizontal", "═").unwrap();
    let nord_ctx = RenderContext {
        theme: &nord_theme,
        ..ctx
    };

    for (w, h) in [(100, 30), (50, 14), (28, 8)] {
        let mut resized_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        resized_term
            .draw(|frame| {
                draw_frame(
                    frame,
                    &cfg,
                    effect.as_ref(),
                    &transition,
                    &mut windows,
                    &mut customizer,
                    &cmd_line,
                    &startup,
                    &themes,
                    &catalog,
                    &nord_ctx,
                );
            })
            .unwrap();
        let r_buf = resized_term.backend().buffer();
        let mut r_dump = String::new();
        for y in 0..r_buf.area.height {
            for x in 0..r_buf.area.width {
                r_dump.push_str(r_buf[(x, y)].symbol());
            }
        }
        assert!(r_dump.contains("╔"));
        assert!(r_dump.contains("═"));
        // Verify popup border uses Nord theme border color at bottom area
        let popup_border_color_found = (0..r_buf.area.height).any(|y| {
            (0..r_buf.area.width).any(|x| {
                r_buf[(x, y)].symbol() == "╔" && r_buf[(x, y)].fg == nord_theme.border
            })
        });
        assert!(popup_border_color_found);
    }
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
        let lines = font.render_lines("I'm kimas.");
        assert!(!lines.is_empty(), "font {font_name} produced empty output");
        assert!(lines.len() >= 3);
        let expected_w = lines[0].chars().count();
        for l in &lines {
            assert_eq!(
                l.chars().count(),
                expected_w,
                "font {font_name} must pad every row uniformly"
            );
        }
    }

    let narrow = render_ascii_heading("Ola, Kiam!", "ANSI Shadow", false, 18);
    assert_eq!(narrow, vec!["Ola, Kiam!"]);

    let mut kern_flf = String::from("flf2a$ 2 2 5 0 1\ncomment\n");
    for code in 32u32..=126u32 {
        if code == 65 {
            kern_flf.push_str("A$@\nA$@@\n");
        } else if code == 66 {
            kern_flf.push_str(" B@\n B@@\n");
        } else {
            kern_flf.push_str(" @\n @@\n");
        }
    }
    let parsed = FigletFont::from_flf_str("kern", &kern_flf).expect("valid flf");
    let rendered = parsed.render_lines("AB");
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0], "A B");

    let mut smush_flf = String::from("flf2a$ 2 2 5 3 1\ncomment\n");
    for code in 32u32..=126u32 {
        if code == 65 {
            smush_flf.push_str("_@\n|@@\n");
        } else if code == 66 {
            smush_flf.push_str("/@\n|@@\n");
        } else {
            smush_flf.push_str(" @\n @@\n");
        }
    }
    let smush_font = FigletFont::from_flf_str("smush", &smush_flf).expect("valid smush flf");
    let smushed = smush_font.render_lines("AB");
    assert_eq!(smushed[0], "/");
    assert_eq!(smushed[1], "|");

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

#[test]
fn completion_excludes_exact_matches_and_preserves_alternatives() {
    let registry = CommandRegistry::new();
    let base_theme = Theme::from_config(&Config::default().theme);
    let themes = vec![
        ThemeEntry {
            name: "matrix".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        ThemeEntry {
            name: "matrix_dark".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        // Duplicate theme name to verify deduplication
        ThemeEntry {
            name: "matrix".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        ThemeEntry {
            name: "bamboo".to_string(),
            theme: base_theme.clone(),
            source_path: None,
        },
        ThemeEntry {
            name: "nord".to_string(),
            theme: base_theme,
            source_path: None,
        },
    ];
    let animations = AnimationCatalog::load();
    let layouts = vec!["my_grid".to_string(), "compact".to_string()];
    let ctx = CompletionContext {
        themes: &themes,
        animations: &animations,
        layouts: &layouts,
    };

    // Case A: partial command suggests full command
    let res = CompletionEngine::complete("color", 5, &registry, &ctx);
    assert_eq!(res.items.len(), 1);
    assert_eq!(res.items[0].insert_text, "colorscheme");

    // Case B: exact command excludes itself from suggestions
    let res = CompletionEngine::complete("colorscheme", 11, &registry, &ctx);
    assert!(
        res.items.is_empty(),
        "exact command match must not suggest itself: {:?}",
        res.items
    );

    // Exact command alias `:w` excludes `write`, preserves `window`
    let res = CompletionEngine::complete("w", 1, &registry, &ctx);
    assert!(!res.items.iter().any(|i| i.insert_text == "write"));
    assert!(res.items.iter().any(|i| i.insert_text == "window"));

    // Case C: partial argument suggests non-exact candidates without duplicates
    let res = CompletionEngine::complete("colorscheme ma", 14, &registry, &ctx);
    let names: Vec<&str> = res.items.iter().map(|i| i.insert_text.as_str()).collect();
    assert_eq!(names, vec!["matrix", "matrix_dark"]);

    // Case D / E: exact argument excludes itself while preserving longer alternatives
    let res = CompletionEngine::complete("colorscheme matrix", 18, &registry, &ctx);
    let names: Vec<&str> = res.items.iter().map(|i| i.insert_text.as_str()).collect();
    assert_eq!(names, vec!["matrix_dark"]);

    // Unique exact argument yields zero suggestions
    let res = CompletionEngine::complete("colorscheme bamboo", 18, &registry, &ctx);
    assert!(res.items.is_empty());

    // Exact argument completed mid-line evaluates only the token under cursor
    let res = CompletionEngine::complete("colorscheme matrix trailing", 18, &registry, &ctx);
    let names: Vec<&str> = res.items.iter().map(|i| i.insert_text.as_str()).collect();
    assert_eq!(names, vec!["matrix_dark"]);
    assert_eq!(res.replace_start, 12);
    assert_eq!(res.replace_end, 18);

    // Exact match on animation argument
    let res = CompletionEngine::complete("animation play crt_power_on", 27, &registry, &ctx);
    assert!(res.items.is_empty());

    // Pressing Tab when no suggestions exist leaves buffer untouched (no duplication)
    let mut cli = CommandLineState::default();
    cli.open(&registry, &ctx);
    for ch in "colorscheme bamboo".chars() {
        cli.insert_char(ch, &registry, &ctx);
    }
    assert!(cli.suggestions.is_empty());
    assert!(!cli.popup_open);
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme bamboo");

    // Deleting back to partial prefix recalculates suggestions immediately
    for _ in 0..3 {
        cli.backspace(&registry, &ctx);
    }
    assert_eq!(cli.buffer, "colorscheme bam");
    assert_eq!(cli.suggestions.len(), 1);
    assert_eq!(cli.suggestions[0].insert_text, "bamboo");
    cli.next_suggestion(&registry, &ctx);
    assert_eq!(cli.buffer, "colorscheme bamboo");
    assert_eq!(cli.submit(), "colorscheme bamboo");
}

#[test]
fn popup_layout_prevents_overflow_handles_unicode_and_resizes_cleanly() {
    // 1. Display-width helpers handle ASCII and wide CJK/emoji cells accurately
    let cjk = "日本語テスト🔥AnimationDescription";
    let trunc = truncate_display_width(cjk, 12);
    assert!(
        trunc.width() <= 12,
        "truncated width {} exceeded 12: {trunc:?}",
        trunc.width()
    );
    assert!(trunc.ends_with('…'));

    let padded = pad_display_width("日本", 8);
    assert_eq!(padded.width(), 8);

    let (n, t, d) = allocate_three_column_widths(20, 16, 10, 9);
    assert!(n + t + d <= 20);
    assert_eq!(d, 0, "description must hide when narrow width only fits name + tag");

    // 2. Customizer popup (especially Animations tab) stays strictly inside borders across resizes
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    let themes = vec![
        ThemeEntry {
            name: "very_long_custom_theme_name_for_overflow_check".to_string(),
            theme: theme.clone(),
            source_path: Some(std::path::PathBuf::from("/tmp/long_theme.toml")),
        },
        ThemeEntry {
            name: "matrix".to_string(),
            theme: theme.clone(),
            source_path: None,
        },
    ];
    let catalog = AnimationCatalog {
        entries: vec![
            AnimationCatalogEntry {
                id: "ultra_long_animation_identifier_test".to_string(),
                display_name: "UltraLongDisplayName🔥日本語".to_string(),
                description: "中文説明文🔥 This extraordinarily long animation description previously overflowed the right popup border on standard and narrow terminals."
                    .to_string(),
                is_community: false,
                source: AnimationSource::Native(NativeTransition::CrtPowerOn),
            },
            AnimationCatalogEntry {
                id: "community_wave".to_string(),
                display_name: "Community Wave".to_string(),
                description: "Another long TOML preset description extending far past normal window bounds."
                    .to_string(),
                is_community: true,
                source: AnimationSource::Native(NativeTransition::Fade),
            },
        ],
    };

    let bg = create_effect(EffectKind::None, 80, 24, &cfg.background);
    let win_cfg = WindowConfig::default();

    for (term_w, term_h) in [(90u16, 28u16), (52, 18), (34, 16)] {
        for tab in [
            CustomizerTab::Animations,
            CustomizerTab::Themes,
            CustomizerTab::Window,
            CustomizerTab::Layout,
        ] {
            let mut state = CustomizerState {
                open: true,
                tab,
                selected: 0,
                ..CustomizerState::default()
            };
            let mut terminal = Terminal::new(TestBackend::new(term_w, term_h)).unwrap();
            terminal
                .draw(|frame| {
                    render_customizer(
                        frame,
                        frame.area(),
                        &mut state,
                        &cfg,
                        &win_cfg,
                        &themes,
                        &catalog,
                        bg.as_ref(),
                        &theme,
                    );
                })
                .unwrap();

            let rect = state.modal_rect;
            assert!(rect.width >= 30);
            assert!(rect.right() <= term_w);
            let buf = terminal.backend().buffer();

            // Borders around modal must stay intact (never overwritten by overflowing spans)
            assert_eq!(buf[(rect.x, rect.y)].symbol(), theme.borders.top_left);
            assert_eq!(buf[(rect.right() - 1, rect.y)].symbol(), theme.borders.top_right);
            assert_eq!(buf[(rect.x, rect.bottom() - 1)].symbol(), theme.borders.bottom_left);
            assert_eq!(
                buf[(rect.right() - 1, rect.bottom() - 1)].symbol(),
                theme.borders.bottom_right
            );
            for y in (rect.y + 1)..(rect.bottom() - 1) {
                assert_eq!(
                    buf[(rect.x, y)].symbol(),
                    theme.borders.vertical,
                    "left border overwritten on row {y} ({tab:?}, {term_w}x{term_h})"
                );
                assert_eq!(
                    buf[(rect.right() - 1, y)].symbol(),
                    theme.borders.vertical,
                    "right border overwritten on row {y} ({tab:?}, {term_w}x{term_h})"
                );
                // Outside modal right edge must remain clean empty space
                if rect.right() < term_w {
                    assert_eq!(
                        buf[(rect.right(), y)].symbol(),
                        " ",
                        "cell leaked outside right border on row {y} ({tab:?}, {term_w}x{term_h})"
                    );
                }
            }

            if tab == CustomizerTab::Animations {
                let mut dumped = String::new();
                for y in rect.y..rect.bottom() {
                    for x in rect.x..rect.right() {
                        dumped.push_str(buf[(x, y)].symbol());
                    }
                    dumped.push('\n');
                }
                assert!(!dumped.contains("NATIVA"));
                assert!(dumped.contains("[NATIVE]"));
                assert!(dumped.contains("[TOML]"));
                if term_w >= 52 {
                    assert!(
                        dumped.contains('…'),
                        "long animation description should truncate with ellipsis at {term_w}x{term_h}:\n{dumped}"
                    );
                }
            }
        }
    }

    // 3. Completion popup stays cleanly within borders when resized to narrow terminals
    let registry = CommandRegistry::new();
    let layouts = Vec::new();
    let cctx = CompletionContext {
        themes: &themes,
        animations: &catalog,
        layouts: &layouts,
    };
    let mut cmd_line = CommandLineState::default();
    cmd_line.open(&registry, &cctx);
    cmd_line.next_suggestion(&registry, &cctx);

    let mut collector = SystemCollector::new();
    let snap = collector.collect(80, 24);
    let reveal = RevealState::complete();
    let render_ctx = RenderContext {
        config: &cfg,
        theme: &theme,
        snapshot: &snap,
        reveal: &reveal,
        active_page: 0,
        view_mode: ViewMode::Compact,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        window_transparent: false,
    };

    for (term_w, term_h) in [(80u16, 20u16), (34u16, 14u16)] {
        let mut terminal = Terminal::new(TestBackend::new(term_w, term_h)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let anchor = Rect::new(1, 1, area.width.saturating_sub(2), area.height.saturating_sub(3));
                render_command_line(frame, area, anchor, &cmd_line, &render_ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut dumped = String::new();
        for y in 0..term_h {
            for x in 0..term_w {
                dumped.push_str(buf[(x, y)].symbol());
            }
            dumped.push('\n');
        }
        assert!(
            dumped.contains(theme.borders.vertical.as_str()),
            "completion popup border missing at {term_w}x{term_h}:\n{dumped}"
        );
    }
}
