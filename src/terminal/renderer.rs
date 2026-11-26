use crate::animation::BackgroundEffect;
use crate::config::Config;
use crate::ui::layout::compute_layout;
use crate::ui::widgets::{RenderContext, render_dashboard};
use ratatui::Frame;
use ratatui::widgets::Block;

pub fn draw_frame(
    frame: &mut Frame,
    config: &Config,
    effect: &dyn BackgroundEffect,
    ctx: &RenderContext,
) {
    let area = frame.area();
    // Fill base background color across the entire terminal canvas
    frame.render_widget(Block::default().style(ctx.theme.base_style()), area);

    // Draw background animation directly onto the buffer before panels
    effect.render(frame.buffer_mut(), area, &config.background, ctx.theme);

    let layout = compute_layout(area, config, ctx.active_page, ctx.view_mode);
    render_dashboard(frame, &layout, ctx);
}
