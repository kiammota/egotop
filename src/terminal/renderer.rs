use crate::animation::{BackgroundEffect, TransitionPlayer};
use crate::command::CommandLineState;
use crate::config::{Config, ThemeEntry};
use crate::ui::customizer::{
    CustomizerState, render_command_line, render_customizer, render_help_overlay,
};
use crate::ui::layout::compute_layout;
use crate::ui::widgets::{RenderContext, render_dashboard, render_window_frame};
use crate::window::WindowManager;
use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;

#[allow(clippy::too_many_arguments)]
pub fn draw_frame(
    frame: &mut Frame,
    config: &Config,
    effect: &dyn BackgroundEffect,
    transition: &TransitionPlayer,
    windows: &mut WindowManager,
    customizer: &mut CustomizerState,
    command_line: &CommandLineState,
    themes: &[ThemeEntry],
    animations: &crate::animation::AnimationCatalog,
    ctx: &RenderContext,
) {
    let area = frame.area();

    // Base terminal background (if window is transparent and no bg effect, keep Color::Reset)
    let base_bg = if ctx.window_transparent && !config.background.enabled {
        Style::default().fg(ctx.theme.text).bg(Color::Reset)
    } else {
        ctx.theme.base_style()
    };
    frame.render_widget(Block::default().style(base_bg), area);

    // Draw background animation directly onto the buffer
    effect.render(frame.buffer_mut(), area, &config.background, ctx.theme);

    // Resolve and render windows in z-index order
    windows.resolve_all(area);
    for win in windows.windows_sorted() {
        if !win.config.visible {
            continue;
        }
        render_window_frame(frame, win, ctx);
        let inner = win.inner_area();
        if inner.width >= 10 && inner.height >= 4 {
            let layout = compute_layout(inner, config, ctx.active_page, ctx.view_mode);
            render_dashboard(frame, &layout, ctx);
        }
        // Apply startup/transition animation onto this window rectangle
        transition.apply_to_buffer(frame.buffer_mut(), win.resolved_rect, ctx.theme);
    }

    if customizer.open {
        render_customizer(
            frame,
            area,
            customizer,
            config,
            &windows.main_window().config,
            themes,
            animations,
            effect,
            ctx.theme,
        );
    } else if ctx.show_help {
        render_help_overlay(frame, area, ctx.theme);
    }

    // Command-line prompt (only visible when ':' is active or transient toast is shown)
    render_command_line(frame, area, command_line, ctx.status_toast, ctx.theme);
}
