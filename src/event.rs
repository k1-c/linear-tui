use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};

use crate::app::App;
use crate::keys::handle_key;

pub fn poll_and_handle(app: &mut App) -> Result<()> {
    // Tick spinner animation
    if app.loading {
        app.tick_spinner();
    }

    if event::poll(Duration::from_millis(100))?
        && let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
    {
        handle_key(app, key);
    }
    Ok(())
}
