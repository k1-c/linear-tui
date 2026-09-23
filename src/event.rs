use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};

use crate::app::App;
use crate::keys::handle_key;

/// How long to block waiting for input before returning to redraw.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Drain all pending terminal events. Returns true if anything changed and the
/// frame should be redrawn.
pub fn poll_and_handle(app: &mut App) -> Result<bool> {
    let mut dirty = false;

    // Block briefly so an idle TUI doesn't spin the CPU, then drain whatever
    // arrived without blocking again — this keeps held-down keys responsive.
    let mut timeout = POLL_INTERVAL;
    while event::poll(timeout)? {
        timeout = Duration::ZERO;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                handle_key(app, key);
                dirty = true;
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => app.wheel(mouse.column, mouse.row, 1),
                    MouseEventKind::ScrollUp => app.wheel(mouse.column, mouse.row, -1),
                    MouseEventKind::Down(MouseButton::Left) => app.click(mouse.column, mouse.row),
                    _ => continue,
                }
                dirty = true;
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
        if app.should_quit {
            break;
        }
    }

    Ok(dirty)
}
