use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEvent};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub fn poll_event(timeout: Duration) -> std::io::Result<Option<InputEvent>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }

    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(InputEvent::Key(key))),
        Event::Mouse(mouse) => Ok(Some(InputEvent::Mouse(mouse))),
        Event::Resize(w, h) => Ok(Some(InputEvent::Resize(w, h))),
        _ => Ok(None),
    }
}
