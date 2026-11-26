use crate::animation::BackgroundEffect;
use crate::config::{BackgroundConfig, EffectKind};
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct NoneEffect;

impl BackgroundEffect for NoneEffect {
    fn kind(&self) -> EffectKind {
        EffectKind::None
    }

    fn resize(&mut self, _width: u16, _height: u16, _cfg: &BackgroundConfig) {}

    fn update(&mut self, _dt_secs: f32, _cfg: &BackgroundConfig) {}

    fn render(&self, _buf: &mut Buffer, _area: Rect, _cfg: &BackgroundConfig, _theme: &Theme) {}
}
