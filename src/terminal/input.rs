use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    Refresh,
    TogglePause,
    ToggleHelp,
    ReloadConfig,
    CycleViewMode,
    CycleEffect,
    SetPage(u8),
    Resize(u16, u16),
}

pub fn poll_action(timeout: Duration) -> std::io::Result<Option<AppAction>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }

    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(Some(AppAction::Quit));
            }
            let action = match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(AppAction::Quit),
                KeyCode::Char('r') | KeyCode::Char('R') => Some(AppAction::Refresh),
                KeyCode::Char('p') | KeyCode::Char('P') => Some(AppAction::TogglePause),
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => {
                    Some(AppAction::ToggleHelp)
                }
                KeyCode::Char('c') | KeyCode::Char('C') => Some(AppAction::ReloadConfig),
                KeyCode::Char('m') | KeyCode::Char('M') => Some(AppAction::CycleViewMode),
                KeyCode::Char('e') | KeyCode::Char('E') => Some(AppAction::CycleEffect),
                KeyCode::Char(d @ '1'..='9') => {
                    let page = (d as u8) - b'0';
                    Some(AppAction::SetPage(page))
                }
                _ => None,
            };
            Ok(action)
        }
        Event::Resize(w, h) => Ok(Some(AppAction::Resize(w, h))),
        _ => Ok(None),
    }
}
