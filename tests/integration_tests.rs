use egotop::animation::{
    AnimationCatalog, AnimationCatalogEntry, BackgroundEffect, BackgroundLayerKind,
    BackgroundLayerSpec, BackgroundPipeline, EasingCurve, GeometryRenderer, LoopConfig, LoopMode,
    MORPH_SAMPLE_COUNT, NativeTransition, ShapeDrawParams, ShapePrimitive, ShapeSpec,
    TextDirection, TextEffectKind, TextRenderParams, TimelineClock, TransitionPlayer,
    create_effect, evaluate_text_cells, morph_contours, rasterize_contour, resolve_region,
};
use egotop::command::{
    AnimationAction, BackgroundCommand, CommandLineState, CommandRegistry, CompletionContext,
    CompletionEngine, LayoutSelection, ParsedCommand, WriteTarget, parse_command,
};
use egotop::config::{
    AnimationDefinition, AnimationPackage, BackgroundConfig, BorderStyle, Config,
    DeclarativeAnimation, EffectKind, PackageOrigin, RegionConfig, RegionUnit, StartupMode,
    ThemeEntry, ViewMode, load_from_path, load_or_create, load_packages_from_dir,
    validate_safe_asset_path,
};
use egotop::system::SystemCollector;
use egotop::system::os::parse_os_release;
use egotop::system::uptime::format_uptime;
use egotop::terminal::renderer::{CursorStartupState, draw_frame};
use egotop::ui::borders::{BorderChars, parse_border_style, validate_border_char};
use egotop::runtime::{
    ChangeDebouncer, ConfigurationRuntime, LoadStatus, ManagedAppTarget, ResourceId, ResourceKind,
    ResourceRegistry, RuntimePaths, RuntimeResourceManager, ValidationStatus, WatchEventKind,
};
use egotop::ui::components::render_clipped;
use egotop::ui::customizer::{
    CustomizerState, CustomizerTab, DiagnosticsOverlayState, DiagnosticsViewMode, render_customizer,
    render_diagnostics_overlay,
};
use egotop::ui::fonts::{FigletFont, available_fonts, load_font, normalize_ascii_text};
use egotop::ui::theme::{Theme, adapt_color, parse_color, rgb_to_ansi256};
use egotop::ui::typography::{render_ascii_heading, render_figlet_lines, slice_marquee_lines};
use egotop::ui::widgets::{
    RenderContext, allocate_three_column_widths, pad_display_width, render_command_line,
    truncate_display_width,
};
use egotop::ui::{
    ComponentAnimationStore, ComponentCategory, ComponentConfig, ComponentKind,
    ContainerBorderConfig, ContainerConfig, ContainerDirection, ContainerNode, ContainerSize,
    RevealState, compute_container_layout, compute_dashboard_layout,
};
use unicode_width::UnicodeWidthStr;
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
fn container_tree_layout_fixed_relative_fill_padding_gap_visibility() {
    let root = ContainerConfig {
        id: "root".to_string(),
        visible: true,
        direction: ContainerDirection::Vertical,
        width: ContainerSize::Percent(100),
        height: ContainerSize::Percent(100),
        padding: 1,
        gap: 1,
        border: ContainerBorderConfig::titled("Root"),
        children: vec![
            ContainerNode::Component(ComponentConfig {
                id: "top_fixed".to_string(),
                visible: true,
                category: ComponentCategory::Static,
                kind: ComponentKind::Text,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fixed(3),
                ..ComponentConfig::default()
            }),
            ContainerNode::Component(ComponentConfig {
                id: "hidden_node".to_string(),
                visible: false,
                category: ComponentCategory::Static,
                kind: ComponentKind::Text,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fixed(10),
                ..ComponentConfig::default()
            }),
            ContainerNode::Container(ContainerConfig {
                id: "row".to_string(),
                visible: true,
                direction: ContainerDirection::Horizontal,
                width: ContainerSize::Fill(1),
                height: ContainerSize::Fill(1),
                gap: 2,
                children: vec![
                    ContainerNode::Component(ComponentConfig {
                        id: "left_pct".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::Cpu,
                        width: ContainerSize::Percent(30),
                        max_width: Some(25),
                        height: ContainerSize::Fill(1),
                        ..ComponentConfig::default()
                    }),
                    ContainerNode::Component(ComponentConfig {
                        id: "right_fill".to_string(),
                        visible: true,
                        category: ComponentCategory::Dynamic,
                        kind: ComponentKind::Memory,
                        width: ContainerSize::Fill(1),
                        height: ContainerSize::Fill(1),
                        ..ComponentConfig::default()
                    }),
                ],
                ..ContainerConfig::default()
            }),
        ],
        ..ContainerConfig::default()
    };

    let viewport = Rect::new(0, 0, 100, 30);
    let layout = compute_container_layout(viewport, &root);

    // Hidden node must not be placed or consume gap
    assert!(layout.find_component("hidden_node").is_none());

    let root_placed = layout.find_container("root").unwrap();
    // 1 cell border + 1 cell padding on all sides => inset 2
    assert_eq!(root_placed.inner_rect, Rect::new(2, 2, 96, 26));

    let top = layout.find_component("top_fixed").unwrap();
    assert_eq!(top.outer_rect, Rect::new(2, 2, 96, 3));

    let row = layout.find_container("row").unwrap();
    // y = 2 + 3 (top) + 1 (gap) = 6; height = 26 - 3 - 1 = 22
    assert_eq!(row.outer_rect, Rect::new(2, 6, 96, 22));

    let left = layout.find_component("left_pct").unwrap();
    // 30% of (96 - 2 gap) = 28, clamped by max_width=25
    assert_eq!(left.outer_rect, Rect::new(2, 6, 25, 22));

    let right = layout.find_component("right_fill").unwrap();
    // x = 2 + 25 + 2 (gap) = 29; width = 96 - 2 - 25 = 69
    assert_eq!(right.outer_rect, Rect::new(29, 6, 69, 22));
}

#[test]
fn command_parser_and_contextual_completion_engine() {
    // Scenarios 8 & 9: :w / :write and :q / :quit (and :save must NOT exist)
    assert_eq!(parse_command(":w").unwrap(), ParsedCommand::Write(WriteTarget::All));
    assert_eq!(parse_command(":write").unwrap(), ParsedCommand::Write(WriteTarget::All));
    assert_eq!(
        parse_command(":write config").unwrap(),
        ParsedCommand::Write(WriteTarget::Config)
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
    assert_eq!(parse_border_style("none").unwrap(), BorderStyle::None);
    let mut no_border_cfg = Config::default();
    no_border_cfg.theme.border_type = BorderStyle::None;
    let layout_no_border =
        compute_dashboard_layout(Rect::new(0, 0, 100, 30), &no_border_cfg, ViewMode::Dashboard);
    let sys_no_border = layout_no_border.find_component("system").unwrap();
    // No 1-cell border inset, only 1-cell padding
    assert_eq!(sys_no_border.inner_rect.x, sys_no_border.outer_rect.x + 1);

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

    let mut anim_store = ComponentAnimationStore::new();
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
        view_mode: ViewMode::Dashboard,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        transparent_bg: false,
        config_error_count: 0,
    };
    terminal
        .draw(|frame| {
            draw_frame(
                frame,
                &cfg,
                effect.as_ref(),
                &transition,
                &anim_store,
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
    let layout = compute_dashboard_layout(Rect::new(0, 0, 120, 36), &cfg, ViewMode::Dashboard);
    anim_store.update_all(0.1, &layout, &cfg, &catalog);
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
                &anim_store,
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
                    &anim_store,
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
fn container_layout_small_terminal_resize_no_overlap_and_clipping() {
    let cfg = Config::default();

    for (w, h) in [(0, 0), (6, 3), (10, 4), (20, 8), (45, 16), (80, 24), (200, 60)] {
        let screen = Rect::new(0, 0, w, h);
        for mode in [ViewMode::Dashboard, ViewMode::Compact, ViewMode::Minimal] {
            let layout = compute_dashboard_layout(screen, &cfg, mode);
            for comp in &layout.components {
                assert!(comp.outer_rect.right() <= screen.right());
                assert!(comp.outer_rect.bottom() <= screen.bottom());
                assert!(comp.inner_rect.right() <= comp.outer_rect.right());
                assert!(comp.inner_rect.bottom() <= comp.outer_rect.bottom());
            }
            // Verify zero pairwise overlap among all placed components
            for i in 0..layout.components.len() {
                for j in (i + 1)..layout.components.len() {
                    let a = layout.components[i].outer_rect;
                    let b = layout.components[j].outer_rect;
                    let overlap_x = a.x < b.right() && b.x < a.right();
                    let overlap_y = a.y < b.bottom() && b.y < a.bottom();
                    assert!(
                        !(overlap_x && overlap_y),
                        "Components {} ({a:?}) and {} ({b:?}) overlapped at {w}x{h} ({mode:?})",
                        layout.components[i].id,
                        layout.components[j].id
                    );
                }
            }
        }
    }

    // Verify render_clipped never writes outside target_rect
    let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
    let target = Rect::new(5, 3, 6, 4);
    term.draw(|frame| {
        render_clipped(frame, target, |buf, local| {
            // Attempt to draw inside local bounds (and even beyond via set_stringn)
            for y in 0..local.height {
                buf.set_stringn(0, y, "XXXXXXXXXXXX", local.width as usize, ratatui::style::Style::default());
            }
        });
    })
    .unwrap();
    let buf = term.backend().buffer();
    for y in 0..10u16 {
        for x in 0..20u16 {
            let inside = x >= target.x && x < target.right() && y >= target.y && y < target.bottom();
            if inside {
                assert_eq!(buf[(x, y)].symbol(), "X");
            } else {
                assert_eq!(buf[(x, y)].symbol(), " ", "cell ({x},{y}) leaked outside {target:?}");
            }
        }
    }
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

    // Exact command alias `:w` excludes `write`, while partial `a` suggests `animation`
    let res = CompletionEngine::complete("w", 1, &registry, &ctx);
    assert!(!res.items.iter().any(|i| i.insert_text == "write"));
    let res_a = CompletionEngine::complete("a", 1, &registry, &ctx);
    assert!(res_a.items.iter().any(|i| i.insert_text == "animation"));

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
    let bundled_sample = TransitionPlayer::new("crt_power_on", &AnimationCatalog::load(), 600).package;
    let community_sample = TransitionPlayer::new("fade", &AnimationCatalog::load(), 600).package;
    let catalog = AnimationCatalog {
        entries: vec![
            AnimationCatalogEntry {
                id: "ultra_long_animation_identifier_test".to_string(),
                display_name: "UltraLongDisplayName🔥日本語".to_string(),
                description: "中文説明文🔥 This extraordinarily long animation description previously overflowed the right popup border on standard and narrow terminals."
                    .to_string(),
                is_community: false,
                package: bundled_sample,
            },
            AnimationCatalogEntry {
                id: "community_wave".to_string(),
                display_name: "Community Wave".to_string(),
                description: "Another long TOML preset description extending far past normal window bounds."
                    .to_string(),
                is_community: true,
                package: community_sample,
            },
        ],
    };

    let bg = create_effect(EffectKind::None, 80, 24, &cfg.background);

    for (term_w, term_h) in [(90u16, 28u16), (52, 18), (34, 16)] {
        for tab in [
            CustomizerTab::Animations,
            CustomizerTab::Themes,
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
        view_mode: ViewMode::Compact,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        transparent_bg: false,
        config_error_count: 0,
    };

    for (term_w, term_h) in [(80u16, 20u16), (34u16, 14u16)] {
        let mut terminal = Terminal::new(TestBackend::new(term_w, term_h)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_command_line(frame, area, &cmd_line, &render_ctx);
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

#[test]
fn geometry_morphing_easing_and_subcell_rasterizers() {
    // Easing curve bounds & cubic_bezier solver
    for curve in [
        EasingCurve::Linear,
        EasingCurve::EaseIn,
        EasingCurve::EaseOut,
        EasingCurve::EaseInOut,
        EasingCurve::parse("cubic_bezier(0.25, 0.1, 0.25, 1.0)").unwrap(),
    ] {
        assert!((curve.apply(0.0) - 0.0).abs() < 1e-3);
        assert!((curve.apply(1.0) - 1.0).abs() < 1e-3);
        let mid = curve.apply(0.5);
        assert!(mid > 0.0 && mid < 1.0, "curve {curve:?} mid={mid}");
    }

    // All primitives normalize cleanly to fixed MORPH_SAMPLE_COUNT and interpolate continuously
    let circle = ShapeSpec::from_primitive(ShapePrimitive::Circle, 6, Vec::new()).normalized_contour();
    let square = ShapeSpec::from_primitive(ShapePrimitive::Square, 6, Vec::new()).normalized_contour();
    let triangle =
        ShapeSpec::from_primitive(ShapePrimitive::Triangle, 6, Vec::new()).normalized_contour();
    let hexagon = ShapeSpec::from_primitive(ShapePrimitive::Polygon, 6, Vec::new()).normalized_contour();
    let rect = ShapeSpec::from_primitive(ShapePrimitive::Rectangle, 6, Vec::new()).normalized_contour();
    let line = ShapeSpec::from_primitive(ShapePrimitive::Line, 6, Vec::new()).normalized_contour();
    let path = ShapeSpec::from_primitive(
        ShapePrimitive::Path,
        6,
        vec![(-0.8, -0.5), (0.0, 0.8), (0.8, -0.5)],
    )
    .normalized_contour();

    for c in [&circle, &square, &triangle, &hexagon, &rect, &line, &path] {
        assert_eq!(c.len(), MORPH_SAMPLE_COUNT);
    }

    let morphed = morph_contours(&circle, &square, 0.5);
    assert_eq!(morphed.len(), MORPH_SAMPLE_COUNT);
    // Mid-morph differs smoothly from both endpoints
    assert!(morphed.iter().zip(&circle).any(|(m, c)| (m.0 - c.0).abs() > 0.02));
    assert!(morphed.iter().zip(&square).any(|(m, s)| (m.0 - s.0).abs() > 0.02));

    // Rasterize across Braille, Quadrant/Block, and ASCII inside clip bounds
    let area = Rect::new(0, 0, 32, 16);
    for renderer in [
        GeometryRenderer::Braille,
        GeometryRenderer::Quadrant,
        GeometryRenderer::Block,
        GeometryRenderer::Ascii,
    ] {
        let mut buf = Buffer::empty(area);
        let params = ShapeDrawParams {
            center_x: 16.0,
            center_y: 8.0,
            half_w: 10.0,
            half_h: 6.0,
            rotation_deg: 25.0,
            scale: 1.0,
            stroke: true,
            fill: renderer == GeometryRenderer::Braille,
            closed: true,
            stroke_color: Color::Rgb(57, 255, 136),
            fill_color: Color::Rgb(16, 60, 38),
            opacity: 1.0,
            renderer,
            stroke_char: '*',
            fill_char: '#',
            truecolor: true,
        };
        rasterize_contour(&morphed, &params, area, &mut buf);
        let drawn = buf
            .content
            .iter()
            .filter(|cell| cell.symbol() != " ")
            .count();
        assert!(drawn > 5, "renderer {renderer:?} produced too few glyphs: {drawn}");
    }

    // ANSI-256 color fallback
    let idx = rgb_to_ansi256(57, 255, 136);
    assert!(idx >= 16);
    assert!(matches!(
        adapt_color(Color::Rgb(57, 255, 136), false),
        Color::Indexed(_)
    ));
    assert_eq!(
        adapt_color(Color::Rgb(57, 255, 136), true),
        Color::Rgb(57, 255, 136)
    );
}

#[test]
fn timeline_loops_unified_showcases_text_fx_and_backgrounds() {
    // 1. Loop controller modes: once, repeat, ping_pong, pause/resume/stop/speed
    let mut ping = TimelineClock::new(
        1000,
        LoopConfig {
            mode: LoopMode::PingPong,
            count: 2,
            interval_ms: 0,
        },
        1.0,
        30,
    );
    ping.advance(0.5);
    assert!((ping.normalized_time() - 0.5).abs() < 0.02);
    ping.advance(0.6); // crossed cycle 0 -> reversed! (at 0.1s from end -> 0.9)
    assert!(ping.reverse);
    assert!((ping.normalized_time() - 0.9).abs() < 0.03);
    ping.pause();
    let frozen = ping.normalized_time();
    ping.advance(0.5);
    assert_eq!(ping.normalized_time(), frozen);
    ping.resume();
    ping.set_speed(2.0);
    ping.advance(0.5); // 1.0s scaled -> reaches count limit 2 -> finished
    assert!(ping.finished);

    // 2. All bundled showcase animations load & validate through unified engine
    let catalog = AnimationCatalog::load();
    let expected_showcases = [
        "shape-morph",
        "orbital-prism",
        "neon-banner",
        "aurora-horizon",
        "matrix-overload",
        "starlight-warp",
        "pulse-scanner",
        "crt_power_on",
        "curtains",
        "fade",
        "slide",
        "matrix_boot",
        "cursor-expand",
        "cyber_boot",
        "neon_curtains",
    ];
    let theme = Theme::from_config(&Config::default().theme);
    let area = Rect::new(0, 0, 80, 24);

    for id in expected_showcases {
        assert!(
            catalog.entries.iter().any(|e| e.id == id),
            "missing bundled animation '{id}'"
        );
        let mut player = TransitionPlayer::new(id, &catalog, 600);
        player.update(0.4);
        let mut buf = Buffer::empty(area);
        player.render_scene(&mut buf, area, &theme);
        player.apply_to_buffer(&mut buf, area, &theme);
        if player.has_scene_tracks() {
            let cells = buf.content.iter().filter(|c| c.symbol() != " ").count();
            assert!(cells > 0, "scene '{id}' rendered no cells at 0.4s");
        }
    }

    // Verify shape-morph advances continuously across morph stages
    let mut morph = TransitionPlayer::new("shape-morph", &catalog, 0);
    let mut b0 = Buffer::empty(area);
    morph.render_scene(&mut b0, area, &theme);
    morph.update(1.6);
    let mut b1 = Buffer::empty(area);
    morph.render_scene(&mut b1, area, &theme);
    assert_ne!(
        b0.content.iter().map(|c| c.symbol().to_string()).collect::<Vec<_>>(),
        b1.content.iter().map(|c| c.symbol().to_string()).collect::<Vec<_>>()
    );

    // 3. All 9 Text Animation Engine effects render cleanly inside clip bounds
    let pal = vec![Color::Rgb(57, 255, 136), Color::Rgb(0, 217, 255)];
    for eff in [
        TextEffectKind::Typewriter,
        TextEffectKind::Fade,
        TextEffectKind::Reveal,
        TextEffectKind::Wave,
        TextEffectKind::Glitch,
        TextEffectKind::ColorCycle,
        TextEffectKind::CharByChar,
        TextEffectKind::Scroll,
        TextEffectKind::Pulse,
    ] {
        let cells = evaluate_text_cells(
            &TextRenderParams {
                text: "EGOTOP ENGINE",
                effect: eff,
                direction: TextDirection::Left,
                center_x: 40.0,
                center_y: 12.0,
                progress: 0.65,
                elapsed_secs: 0.75,
                speed: 1.0,
                intensity: 1.0,
                opacity: 1.0,
                base_color: Color::Rgb(57, 255, 136),
                palette: &pal,
                bg_fallback: Color::Rgb(5, 10, 7),
                charset: "01#$",
            },
            area,
        );
        assert!(!cells.is_empty(), "text effect {eff:?} emitted zero cells");
        assert!(cells.iter().all(|c| c.x < area.right() && c.y < area.bottom()));
    }

    // 4. Multi-layer BackgroundPipeline composition, resize, speed & pause
    let cfg = Config::default();
    let mut bg = BackgroundPipeline::from_config(&cfg.background, 80, 24);
    for (idx, kind) in [
        BackgroundLayerKind::Gradient,
        BackgroundLayerKind::Starfield,
        BackgroundLayerKind::Noise,
        BackgroundLayerKind::FloatingChars,
    ]
    .into_iter()
    .enumerate()
    {
        bg.add_layer(
            BackgroundLayerSpec {
                effect: kind,
                z_index: idx as i32 + 1,
                ..BackgroundLayerSpec::default()
            },
            80,
            24,
            &cfg.background,
            &std::collections::BTreeMap::new(),
        );
    }
    assert_eq!(bg.layer_count(), 5);
    bg.set_speed(1.5);
    bg.update(0.2, &cfg.background);
    bg.resize(60, 18, &cfg.background);
    let mut bg_buf = Buffer::empty(Rect::new(0, 0, 60, 18));
    bg.render(&mut bg_buf, Rect::new(0, 0, 60, 18), &cfg.background, &theme);
    assert!(bg_buf.content.iter().any(|c| c.symbol() != " "));
    bg.pause();
    bg.resume();
    bg.clear();
    assert_eq!(bg.layer_count(), 0);
}

#[test]
fn community_package_sandbox_and_runtime_commands() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("egotop_anim_pkgs_{nanos}"));
    let good_pkg = root.join("custom-galaxy");
    std::fs::create_dir_all(good_pkg.join("assets")).unwrap();
    std::fs::write(good_pkg.join("assets/logo.txt"), "  *\n * *\n*****\n").unwrap();
    std::fs::write(
        good_pkg.join("animation.toml"),
        r##"
[animation]
name = "custom-galaxy"
version = 2
duration_ms = 2000
fps = 30

[animation.loop]
mode = "infinite"

[[tracks]]
id = "logo"
type = "ascii"
asset = "assets/logo.txt"
position = ["center", "center"]
color = "#39FF88"
"##,
    )
    .unwrap();

    // Malicious path traversal package must be rejected cleanly
    let bad_pkg = root.join("traversal-attack");
    std::fs::create_dir_all(&bad_pkg).unwrap();
    std::fs::write(
        bad_pkg.join("animation.toml"),
        r#"
[animation]
name = "traversal-attack"
version = 2

[[tracks]]
id = "leak"
type = "ascii"
asset = "../../../etc/passwd"
"#,
    )
    .unwrap();

    // Excess resource limit manifest must fail validation
    assert!(
        AnimationDefinition::parse_toml(
            r#"
[animation]
name = "too-many"
version = 2
[[tracks]]
id = "p"
type = "particles"
count = 9999
"#
        )
        .is_err()
    );
    assert!(validate_safe_asset_path("../secret.txt").is_err());
    assert!(validate_safe_asset_path("/etc/passwd").is_err());
    assert!(validate_safe_asset_path("assets/hero.txt").is_ok());

    let loaded = load_packages_from_dir(&root);
    assert!(loaded.contains_key("custom-galaxy"));
    assert!(!loaded.contains_key("traversal-attack"));
    let pkg: &AnimationPackage = &loaded["custom-galaxy"];
    assert!(matches!(pkg.origin, PackageOrigin::Community { .. }));
    assert!(pkg.assets["assets/logo.txt"].contains("*****"));
    let _ = std::fs::remove_dir_all(&root);

    // Runtime commands & completion for expanded :animation & :background
    assert_eq!(
        parse_command(":animation pause").unwrap(),
        ParsedCommand::Animation {
            action: AnimationAction::Pause,
            name: None
        }
    );
    assert_eq!(
        parse_command(":animation speed 1.5").unwrap(),
        ParsedCommand::Animation {
            action: AnimationAction::Speed,
            name: Some("1.5".to_string())
        }
    );
    assert_eq!(
        parse_command(":background set starfield").unwrap(),
        ParsedCommand::Background(BackgroundCommand::Set("starfield".to_string()))
    );
    assert_eq!(
        parse_command(":background add noise").unwrap(),
        ParsedCommand::Background(BackgroundCommand::Add("noise".to_string()))
    );
    assert_eq!(
        parse_command(":background stop").unwrap(),
        ParsedCommand::Background(BackgroundCommand::Stop)
    );

    let registry = CommandRegistry::new();
    let themes = Vec::new();
    let animations = AnimationCatalog::load();
    let layouts = Vec::new();
    let cctx = CompletionContext {
        themes: &themes,
        animations: &animations,
        layouts: &layouts,
    };
    let res = CompletionEngine::complete("background ", 11, &registry, &cctx);
    assert!(res.items.iter().any(|i| i.insert_text == "starfield"));
    assert!(res.items.iter().any(|i| i.insert_text == "add"));
    let exact_bg = CompletionEngine::complete("background starfield", 20, &registry, &cctx);
    assert!(!exact_bg.items.iter().any(|i| i.insert_text == "starfield"));
}

#[test]
fn live_configuration_runtime_validation_diagnostics_and_last_known_good() {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("egotop_runtime_test_{nanos}"));
    let paths = RuntimePaths::from_root_dir(root.clone());
    std::fs::create_dir_all(&paths.themes_dir).unwrap();
    std::fs::create_dir_all(&paths.animations_dir).unwrap();
    std::fs::create_dir_all(&paths.layouts_dir).unwrap();

    // 1. Valid initial files + custom theme
    let valid_theme = r##"
name = "bamboo"
background = "#050A07"
panel_background = "#08110C"
primary = "#39FF88"
secondary = "#00D9FF"
accent = "#FFC857"
text = "#E5E5E5"
muted = "#5F7D6B"
border = "#1F7A4C"
danger = "#FF5555"
"##;
    let bamboo_path = paths.themes_dir.join("bamboo.toml");
    std::fs::write(&bamboo_path, valid_theme).unwrap();

    let mut initial_cfg = Config::default();
    initial_cfg.theme.name = "bamboo".to_string();
    std::fs::write(&paths.config_path, toml::to_string_pretty(&initial_cfg).unwrap()).unwrap();

    let mut runtime =
        ConfigurationRuntime::start(paths.clone(), initial_cfg.clone(), false);
    assert_eq!(runtime.diagnostics().error_count(), 0);
    assert_eq!(runtime.state().config.theme.name, "bamboo");
    assert!(runtime.state().themes.contains_key("bamboo"));

    // Preserve active Theme & Config cleanly for visual comparison
    let mut active_theme = runtime.state().themes["bamboo"].theme.clone();
    let mut active_cfg = runtime.state().config.clone();
    let mut active_themes = runtime.state().themes.clone();
    let mut active_theme_list: Vec<_> = active_themes.values().cloned().collect();
    let mut active_anims = runtime.state().animations.clone();
    let mut active_layouts_map = BTreeMap::new();
    let mut active_layout_names = Vec::new();
    let mut active_bg = BackgroundPipeline::from_config(&active_cfg.background, 80, 24);
    let mut active_transition = TransitionPlayer::new("none", &active_anims, 0);

    // 2. Invalid TOML syntax (E1001) preserves LastKnownGood
    let broken_syntax = "[general\nname = 'broken'\n";
    std::fs::write(&paths.config_path, broken_syntax).unwrap();
    let commit = runtime.reload_all();
    assert!(commit.had_errors);
    assert!(runtime.diagnostics().error_count() >= 1);
    assert_eq!(runtime.diagnostics().active_errors()[0].code, "E1001");
    assert!(runtime.diagnostics().active_errors()[0].line.is_some());
    assert_eq!(runtime.state().config.theme.name, "bamboo");

    // 3. Unknown field (E1002) with exact location
    let mut with_unknown = toml::to_string_pretty(&initial_cfg).unwrap();
    with_unknown.push_str("\n[general]\nnonexistent_option = 42\n");
    // Merge unknown key inside [general] cleanly
    let unknown_cfg = "[general]\nname = \"egotop\"\nunknown_setting = true\n";
    std::fs::write(&paths.config_path, unknown_cfg).unwrap();
    let commit = runtime.reload_all();
    assert!(commit.had_errors);
    assert!(
        runtime
            .diagnostics()
            .active_errors()
            .iter()
            .any(|d| d.code == "E1002" && d.field_path.as_deref() == Some("general.unknown_setting"))
    );

    // 4. Wrong field type (E1003)
    std::fs::write(&paths.config_path, "[general]\nanimation_fps = \"fast\"\n").unwrap();
    let commit = runtime.reload_all();
    assert!(commit.had_errors);
    assert!(
        runtime
            .diagnostics()
            .active_errors()
            .iter()
            .any(|d| d.code == "E1003" || d.code == "E1001")
    );

    // 5. Invalid color in bamboo theme (E1004) with exact field, expected/actual, & LKG preservation
    let bad_color_theme = r##"
name = "bamboo"
background = "#050A07"
primary = "#39FF88"
border = "invalid-color"
"##;
    std::fs::write(&bamboo_path, bad_color_theme).unwrap();
    std::fs::write(&paths.config_path, toml::to_string_pretty(&initial_cfg).unwrap()).unwrap();
    let commit = runtime.reload_all();
    assert!(commit.had_errors);
    let color_err = runtime
        .diagnostics()
        .active_errors()
        .iter()
        .find(|d| d.code == "E1004")
        .copied()
        .expect("expected E1004 color diagnostic");
    assert_eq!(color_err.field_path.as_deref(), Some("theme.border"));
    assert_eq!(color_err.actual.as_deref(), Some("invalid-color"));
    assert!(color_err.line.is_some());
    assert!(color_err.column.is_some());
    // Active theme borders color MUST stay original #1F7A4C!
    assert_eq!(active_theme.border, Color::Rgb(31, 122, 76));

    // Apply commit check — refused cleanly when had_errors is true
    {
        let mut target = ManagedAppTarget {
            config: &mut active_cfg,
            theme: &mut active_theme,
            themes_map: &mut active_themes,
            theme_list: &mut active_theme_list,
            animations: &mut active_anims,
            layout_names: &mut active_layout_names,
            layouts_map: &mut active_layouts_map,
            background: &mut active_bg,
            transition: &mut active_transition,
            viewport: Rect::new(0, 0, 80, 24),
        };
        RuntimeResourceManager::apply_commit(&commit, runtime.state(), &mut target);
    }
    assert_eq!(active_theme.border, Color::Rgb(31, 122, 76));

    // 10. Non-existent dependency (E1006: config references missing theme & font)
    std::fs::write(&bamboo_path, valid_theme).unwrap();
    let mut bad_dep_cfg = initial_cfg.clone();
    bad_dep_cfg.theme.name = "missing_theme".to_string();
    bad_dep_cfg.typography.font = "missing_font".to_string();
    std::fs::write(&paths.config_path, toml::to_string_pretty(&bad_dep_cfg).unwrap()).unwrap();
    let commit = runtime.reload_all();
    assert!(commit.had_errors);
    assert!(
        runtime
            .diagnostics()
            .active_errors()
            .iter()
            .filter(|d| d.code == "E1006")
            .count()
            >= 2
    );

    // 12. Valid updated configuration commits & applies cleanly via RuntimeResourceManager
    let mut updated_theme = valid_theme.replace("#39FF88", "#00AAFF");
    updated_theme.push('\n');
    std::fs::write(&bamboo_path, updated_theme).unwrap();
    std::fs::write(&paths.config_path, toml::to_string_pretty(&initial_cfg).unwrap()).unwrap();
    let commit = runtime.reload_all();
    assert!(!commit.had_errors, "errors: {:?}", runtime.diagnostics().active_errors());
    assert_eq!(runtime.diagnostics().error_count(), 0);
    {
        let mut target = ManagedAppTarget {
            config: &mut active_cfg,
            theme: &mut active_theme,
            themes_map: &mut active_themes,
            theme_list: &mut active_theme_list,
            animations: &mut active_anims,
            layout_names: &mut active_layout_names,
            layouts_map: &mut active_layouts_map,
            background: &mut active_bg,
            transition: &mut active_transition,
            viewport: Rect::new(0, 0, 80, 24),
        };
        RuntimeResourceManager::apply_commit(&commit, runtime.state(), &mut target);
    }
    assert_eq!(active_theme.primary, Color::Rgb(0, 170, 255));

    // 13. Out-of-order generation update is rejected deterministically
    let stale_gen = runtime.state().generation - 1;
    let stale_commit = runtime.process_raw_candidate(
        ResourceKind::Theme,
        "bamboo",
        &bamboo_path,
        valid_theme.to_string(),
        stale_gen,
    );
    assert!(stale_commit.had_errors);
    assert_eq!(
        runtime.state().themes["bamboo"].theme.primary,
        Color::Rgb(0, 170, 255)
    );

    // 6 & 7 & 15. Removed file, Renamed file, and Rescan watcher recovery
    let extra_theme = paths.themes_dir.join("temp_neon.toml");
    std::fs::write(&extra_theme, valid_theme.replace("bamboo", "temp_neon")).unwrap();
    runtime.watcher().inject_event(extra_theme.clone(), WatchEventKind::Renamed);
    assert!(runtime.poll().is_none(), "debounce window must delay immediate reload");
    std::thread::sleep(Duration::from_millis(140));
    let commit = runtime.poll().expect("debounced rename event should flush");
    assert!(!commit.had_errors);
    assert!(runtime.state().themes.contains_key("temp_neon"));

    std::fs::remove_file(&extra_theme).unwrap();
    runtime.watcher().inject_event(extra_theme.clone(), WatchEventKind::Removed);
    assert!(runtime.poll().is_none());
    std::thread::sleep(Duration::from_millis(140));
    let commit = runtime.poll().expect("debounced remove event should flush");
    assert!(!commit.had_errors);
    assert!(!runtime.state().themes.contains_key("temp_neon"));

    // Watcher recovery after Rescan
    runtime.watcher().inject_event(std::path::PathBuf::new(), WatchEventKind::Rescan);
    assert!(runtime.poll().is_none());
    std::thread::sleep(Duration::from_millis(140));
    let rescan_commit = runtime.poll().expect("rescan recovery should trigger reload_all");
    assert!(!rescan_commit.had_errors);

    // 8 & 9. Atomic save & rapid burst debouncing inside ChangeDebouncer
    let mut debouncer = ChangeDebouncer::new(Duration::from_millis(80));
    let t0 = Instant::now();
    let sample = paths.themes_dir.join("bamboo.toml");
    debouncer.push(sample.clone(), WatchEventKind::Removed, t0);
    debouncer.push(
        sample.clone(),
        WatchEventKind::Created,
        t0 + Duration::from_millis(10),
    );
    debouncer.push(
        sample.clone(),
        WatchEventKind::Modified,
        t0 + Duration::from_millis(20),
    );
    assert!(debouncer.flush_ready(t0 + Duration::from_millis(40)).0.is_empty());
    let (ready, _) = debouncer.flush_ready(t0 + Duration::from_millis(120));
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].event_count, 3);
    assert_eq!(ready[0].kind, WatchEventKind::Modified);

    // Dependency cycle detection in ResourceRegistry
    let mut reg = ResourceRegistry::new();
    let a = ResourceId::new(ResourceKind::Layout, "a");
    let b = ResourceId::new(ResourceKind::Layout, "b");
    reg.upsert(
        a.clone(),
        paths.layouts_dir.join("a.toml"),
        LoadStatus::Loaded,
        ValidationStatus::Valid,
        vec![b.clone()],
    );
    assert!(reg.detect_cycle(&b, &[a]));

    // 14. Commands :reload, :messages, :errors + Diagnostics overlay UI & header error badge
    assert_eq!(parse_command(":reload").unwrap(), ParsedCommand::Reload);
    assert_eq!(parse_command(":messages").unwrap(), ParsedCommand::Messages);
    assert_eq!(parse_command(":errors").unwrap(), ParsedCommand::Errors);

    // Inject one deliberate error to verify TUI diagnostics presentation renders cleanly
    std::fs::write(&bamboo_path, bad_color_theme).unwrap();
    let err_commit = runtime.reload_all();
    assert!(err_commit.had_errors);
    let mut diag_ui = DiagnosticsOverlayState::default();
    diag_ui.open_mode(DiagnosticsViewMode::Errors);
    let mut term = Terminal::new(TestBackend::new(90, 26)).unwrap();
    term.draw(|frame| {
        render_diagnostics_overlay(
            frame,
            frame.area(),
            &mut diag_ui,
            runtime.diagnostics(),
            &active_theme,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut dump = String::new();
    for y in 0..26 {
        for x in 0..90 {
            dump.push_str(buf[(x, y)].symbol());
        }
        dump.push('\n');
    }
    assert!(dump.contains("E1004"), "overlay output:\n{dump}");
    assert!(dump.contains("theme.border"), "overlay output:\n{dump}");

    // Ensure logs contain structured [Config] entries
    assert!(
        runtime
            .diagnostics()
            .logs()
            .iter()
            .any(|l| l.category == "Config" && l.message.contains("Validation failed"))
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn real_notify_filesystem_watcher_detects_atomic_writes_and_edits() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("egotop_notify_live_{nanos}"));
    let paths = RuntimePaths::from_root_dir(root.clone());
    std::fs::create_dir_all(&paths.themes_dir).unwrap();
    std::fs::create_dir_all(&paths.animations_dir).unwrap();

    let cfg = Config::default();
    std::fs::write(&paths.config_path, toml::to_string_pretty(&cfg).unwrap()).unwrap();

    let mut runtime = ConfigurationRuntime::start(paths.clone(), cfg, true);
    assert!(!runtime.watcher().watched_roots().is_empty());

    // Simulate editor atomic write: write tmp -> rename into themes/live_jade.toml
    let tmp_file = paths.themes_dir.join(".live_jade.toml.tmp");
    let final_file = paths.themes_dir.join("live_jade.toml");
    std::fs::write(
        &tmp_file,
        "name = \"live_jade\"\nprimary = \"#22DD88\"\n",
    )
    .unwrap();
    std::fs::rename(&tmp_file, &final_file).unwrap();

    let mut seen_commit = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if let Some(c) = runtime.poll() {
            seen_commit = Some(c);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(35));
    }

    let commit = seen_commit.expect("real OS notify watcher should deliver debounced theme commit");
    assert!(!commit.had_errors);
    assert!(commit.themes_updated);
    assert_eq!(
        runtime.state().themes["live_jade"].theme.primary,
        Color::Rgb(34, 221, 136)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cli_commands_parse_and_execute_with_appropriate_exit_codes() {
    use clap::Parser;
    use egotop::cli::{Cli, Commands};
    use std::process::Command;

    // 1. Clap derive parsing covers all required administrative subcommands
    assert!(Cli::try_parse_from(["egotop"]).unwrap().command.is_none());
    assert!(matches!(
        Cli::try_parse_from(["egotop", "config"]).unwrap().command,
        Some(Commands::Config(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "config", "show"])
            .unwrap()
            .command,
        Some(Commands::Config(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "config", "path"])
            .unwrap()
            .command,
        Some(Commands::Config(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "config", "validate"])
            .unwrap()
            .command,
        Some(Commands::Config(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "fonts", "list"])
            .unwrap()
            .command,
        Some(Commands::Fonts(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "context", "list"])
            .unwrap()
            .command,
        Some(Commands::Context(_))
    ));
    assert!(matches!(
        Cli::try_parse_from(["egotop", "doctor"]).unwrap().command,
        Some(Commands::Doctor)
    ));

    // 2. End-to-end subprocess checks against compiled binary
    let bin = env!("CARGO_BIN_EXE_egotop");

    let help_out = Command::new(bin).arg("--help").output().unwrap();
    assert!(help_out.status.success());
    let help_str = String::from_utf8_lossy(&help_out.stdout);
    assert!(help_str.contains("config"));
    assert!(help_str.contains("fonts"));
    assert!(help_str.contains("context"));
    assert!(help_str.contains("doctor"));

    let ver_out = Command::new(bin).arg("--version").output().unwrap();
    assert!(ver_out.status.success());
    assert!(String::from_utf8_lossy(&ver_out.stdout).contains("egotop"));

    let cfg_path = temp_config_path("cli_subcmds");
    std::fs::write(&cfg_path, toml::to_string_pretty(&Config::default()).unwrap()).unwrap();

    let summary_out = Command::new(bin)
        .args(["config", "--config", cfg_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(summary_out.status.success());
    assert!(String::from_utf8_lossy(&summary_out.stdout).contains("egotop configuration summary"));

    let show_out = Command::new(bin)
        .args(["--config", cfg_path.to_str().unwrap(), "config", "show"])
        .output()
        .unwrap();
    assert!(show_out.status.success());
    assert!(String::from_utf8_lossy(&show_out.stdout).contains("[general]"));

    let path_out = Command::new(bin)
        .args(["--config", cfg_path.to_str().unwrap(), "config", "path"])
        .output()
        .unwrap();
    assert!(path_out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&path_out.stdout).trim(),
        cfg_path.to_str().unwrap()
    );

    let val_ok = Command::new(bin)
        .args(["--config", cfg_path.to_str().unwrap(), "config", "validate"])
        .output()
        .unwrap();
    assert!(val_ok.status.success());
    assert!(String::from_utf8_lossy(&val_ok.stdout).contains("Configuration OK"));

    // Invalid config must return non-zero exit code and structured diagnostics
    let bad_cfg_path = temp_config_path("cli_bad");
    std::fs::write(&bad_cfg_path, "[theme]\nprimary = \"not-a-color\"\n").unwrap();
    let val_bad = Command::new(bin)
        .args(["--config", bad_cfg_path.to_str().unwrap(), "config", "validate"])
        .output()
        .unwrap();
    assert!(!val_bad.status.success());
    assert!(String::from_utf8_lossy(&val_bad.stderr).contains("E1004"));

    // Missing config file must return non-zero exit code cleanly
    let missing_cfg = temp_config_path("cli_missing_file");
    let _ = std::fs::remove_file(&missing_cfg);
    let val_missing = Command::new(bin)
        .args(["--config", missing_cfg.to_str().unwrap(), "config", "validate"])
        .output()
        .unwrap();
    assert!(!val_missing.status.success());

    let fonts_out = Command::new(bin).args(["fonts", "list"]).output().unwrap();
    assert!(fonts_out.status.success());
    assert!(String::from_utf8_lossy(&fonts_out.stdout).contains("ANSI Shadow"));

    let ctx_out = Command::new(bin).args(["context", "list"]).output().unwrap();
    assert!(ctx_out.status.success());
    assert!(
        String::from_utf8_lossy(&ctx_out.stdout)
            .to_ascii_lowercase()
            .contains("not implemented")
    );

    let doc_ok = Command::new(bin)
        .args(["--config", cfg_path.to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    assert!(doc_ok.status.success());
    assert!(String::from_utf8_lossy(&doc_ok.stdout).contains("egotop doctor"));

    let doc_bad = Command::new(bin)
        .args(["--config", bad_cfg_path.to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    assert!(!doc_bad.status.success());
    assert!(String::from_utf8_lossy(&doc_bad.stdout).contains("[FAIL]"));

    let _ = std::fs::remove_file(&cfg_path);
    let _ = std::fs::remove_file(&bad_cfg_path);
}

#[test]
fn toml_container_tree_and_animated_state_preservation() {
    let toml_cfg = r#"
[ui.root]
id = "root"
direction = "vertical"
width = "100%"
height = "100%"
padding = 1
gap = 1

[[ui.root.children]]
node = "component"
id = "hdr"
category = "dynamic"
type = "header"
width = "fill"
height = 1

[[ui.root.children]]
node = "container"
id = "anim_row"
direction = "horizontal"
width = "fill"
height = "fill"
gap = 1

[[ui.root.children.children]]
node = "component"
id = "spin"
category = "animated"
type = "spinner"
width = "50%"
height = "fill"

[ui.root.children.children.content]
text = "Loading"

[ui.root.children.children.animation]
fps = 10
speed = 1.0
style = "braille"

[[ui.root.children.children]]
node = "component"
id = "typer"
category = "animated"
type = "typewriter"
width = "50%"
height = "fill"

[ui.root.children.children.content]
text = "Hello EgoTop"
"#;

    let parsed_cfg: Config = toml::from_str(toml_cfg).unwrap();
    assert_eq!(parsed_cfg.ui.root.id, "root");
    assert_eq!(parsed_cfg.ui.root.children.len(), 2);

    let parsed_res = egotop::runtime::parse_resource(
        ResourceKind::ApplicationConfig,
        "egotop",
        std::path::Path::new("egotop.toml"),
        toml_cfg.to_string(),
    )
    .unwrap();
    assert!(egotop::runtime::validate_schema(&parsed_res).is_empty());

    let catalog = AnimationCatalog::load();
    let mut anim_store = ComponentAnimationStore::new();
    let layout = compute_container_layout(Rect::new(0, 0, 80, 20), &parsed_cfg.ui.root);

    // Advance animation across multiple frames and verify state is preserved & progresses
    anim_store.update_all(0.15, &layout, &parsed_cfg, &catalog);
    let idx_1 = match anim_store.get("spin") {
        Some(egotop::ui::components::ComponentAnimState::Spinner(s)) => s.frame_idx,
        _ => panic!("expected spinner state"),
    };
    anim_store.update_all(0.25, &layout, &parsed_cfg, &catalog);
    let idx_2 = match anim_store.get("spin") {
        Some(egotop::ui::components::ComponentAnimState::Spinner(s)) => s.frame_idx,
        _ => panic!("expected spinner state"),
    };
    assert_ne!(idx_1, idx_2, "spinner frame index should advance across frames");

    // Verify default dashboard renders centered EGOTOP header without [Pg ...]
    let cfg = Config::default();
    let theme = Theme::from_config(&cfg.theme);
    let mut collector = SystemCollector::new();
    let snap = collector.collect(120, 36);
    let reveal = RevealState::complete();
    let effect = create_effect(EffectKind::None, 120, 36, &cfg.background);
    let transition = TransitionPlayer::new("none", &catalog, 0);
    let mut customizer = CustomizerState::default();
    let cmd_line = CommandLineState::default();
    let startup = CursorStartupState::new(false);
    let themes = vec![];
    let rctx = RenderContext {
        config: &cfg,
        theme: &theme,
        snapshot: &snap,
        reveal: &reveal,
        view_mode: ViewMode::Dashboard,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        transparent_bg: false,
        config_error_count: 0,
    };
    let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
    term.draw(|frame| {
        draw_frame(
            frame,
            &cfg,
            effect.as_ref(),
            &transition,
            &anim_store,
            &mut customizer,
            &cmd_line,
            &startup,
            &themes,
            &catalog,
            &rctx,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut header_row = None;
    for y in 0..6 {
        let mut row = String::new();
        for x in 0..120 {
            row.push_str(buf[(x, y)].symbol());
        }
        if row.contains("EGOTOP") && row.contains("[FX:") {
            header_row = Some(row);
            break;
        }
    }
    let row = header_row.expect("global header row with EGOTOP and [FX: ...] must render");
    let egotop_col = row.find("EGOTOP").unwrap();
    assert!(
        (50..=64).contains(&egotop_col),
        "EGOTOP should be horizontally centered around col 57 (got {egotop_col}):\n{row}"
    );
}

#[test]
fn init_wait_for_input_and_embedded_figlet_marquee() {
    // 1. [init] wait_for_input renders centered message + blinking cursor
    let toml_init = r#"
[init]
wait_for_input = true
cursor = true
message = "Press any key to start"
"#;
    let cfg: Config = toml::from_str(toml_init).unwrap();
    assert!(cfg.init.wait_for_input);
    assert!(cfg.init.cursor);
    assert_eq!(cfg.init.message, "Press any key to start");

    let theme = Theme::from_config(&cfg.theme);
    let mut collector = SystemCollector::new();
    let snap = collector.collect(80, 24);
    let reveal = RevealState::complete();
    let effect = create_effect(EffectKind::None, 80, 24, &cfg.background);
    let catalog = AnimationCatalog::load();
    let transition = TransitionPlayer::new("none", &catalog, 0);
    let mut anim_store = ComponentAnimationStore::new();
    let mut customizer = CustomizerState::default();
    let cmd_line = CommandLineState::default();
    let startup = CursorStartupState::new(true);
    let themes = vec![];
    let rctx = RenderContext {
        config: &cfg,
        theme: &theme,
        snapshot: &snap,
        reveal: &reveal,
        view_mode: ViewMode::Dashboard,
        paused: false,
        show_help: false,
        fps: 30.0,
        status_toast: None,
        transparent_bg: false,
        config_error_count: 0,
    };
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|frame| {
        draw_frame(
            frame,
            &cfg,
            effect.as_ref(),
            &transition,
            &anim_store,
            &mut customizer,
            &cmd_line,
            &startup,
            &themes,
            &catalog,
            &rctx,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut found_init = false;
    for y in 0..24 {
        let row: String = (0..80).map(|x| buf[(x, y)].symbol()).collect();
        if row.contains("Press any key to start") && row.contains("▌") {
            found_init = true;
            break;
        }
    }
    assert!(found_init, "init screen must display message and blinking cursor");

    // 2. Embedded FIGlet rendering + right-to-left continuous marquee slicing
    let figlet = render_figlet_lines("I'm Kimas.", "ANSI Shadow", false);
    assert!(figlet.len() >= 5, "embedded ANSI Shadow should produce multi-line ASCII art");
    let frame0 = slice_marquee_lines(&figlet, 20, 0, 6);
    let frame4 = slice_marquee_lines(&figlet, 20, 4, 6);
    assert_eq!(frame0.len(), figlet.len());
    assert_ne!(frame0, frame4, "marquee offset must shift visible window");

    // 3. Greeting marquee state advances in ComponentAnimationStore
    let layout = compute_dashboard_layout(Rect::new(0, 0, 80, 24), &cfg, ViewMode::Dashboard);
    anim_store.update_all(0.5, &layout, &cfg, &catalog);
    let offset1 = match anim_store.get("greeting") {
        Some(egotop::ui::components::ComponentAnimState::Marquee(m)) => m.offset_cols,
        _ => panic!("expected greeting marquee state"),
    };
    assert!(offset1 > 0.0);
}
