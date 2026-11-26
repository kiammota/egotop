pub mod input;
pub mod renderer;

use crossterm::{
    cursor, execute,
    style::ResetColor,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};

static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct TerminalGuard {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    pub fn enter() -> io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

pub fn restore_terminal() -> io::Result<()> {
    if TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, ResetColor, cursor::Show, LeaveAlternateScreen);
    }
    Ok(())
}

fn install_panic_hook() {
    static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));
}
