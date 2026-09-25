//! The main loop's work, one step at a time, apart from the terminal: send
//! what the app queued, fold in Linear's answers, draw, record the view,
//! and carry out what an agent asks on the control channel.
//!
//! `main` drives a [`Runtime`] against the real terminal, or — headless —
//! against an in-memory one that only agents read.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::{Terminal, backend::Backend};
use tokio::sync::{mpsc, oneshot};
use unicode_width::UnicodeWidthStr;

use crate::adapter::api::client::LinearClient;
use crate::adapter::control::{Asked, Command, Reply};
use crate::adapter::dispatch;
use crate::adapter::herdr::AgentWatch;
use crate::adapter::snapshot::{Recorder, timestamp_now};
use crate::entity::Origin;
use crate::interface::app::App;
use crate::interface::message::Message;
use crate::interface::{event, notation, palette, screen, ui};
use crate::usecase::{Request, agent, favorite, issue, notes, project};

/// Spinner advance interval.
const TICK: Duration = Duration::from_millis(80);

/// How long an agent's command waits for Linear before it is answered with
/// the screen as it is, still loading.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Runtime {
    pub app: App,
    cache: ui::Cache,
    client: Arc<LinearClient>,
    tx: mpsc::UnboundedSender<Message>,
    rx: mpsc::UnboundedReceiver<Message>,
    origin: Origin,
    recorder: Option<Recorder>,
    /// What the herdr plugin says its agents work on; only inside herdr.
    agents: Option<AgentWatch>,
    last_tick: Instant,
    /// Whether the next [`Runtime::draw`] has anything new to show.
    dirty: bool,
    /// Commands from agents, when the control channel is open.
    control: Option<mpsc::UnboundedReceiver<Asked>>,
    /// Commands carried out and waiting for Linear before they are answered.
    waiting: Vec<(oneshot::Sender<Reply>, Instant)>,
    /// The frame last drawn, as text, for agents.
    frame: Vec<String>,
    size: (u16, u16),
    /// What would act outside linear-tui — a browser, herdr — is held
    /// rather than done, when nobody is at the desktop (headless).
    hold_external: bool,
    held: Vec<String>,
}

impl Runtime {
    pub fn new(app: App, client: LinearClient, origin: Origin, recorder: Option<Recorder>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        // Only the herdr plugin writes the agents file.
        let agents = app.herdr.then(AgentWatch::new).flatten();
        Self {
            app,
            cache: ui::Cache::default(),
            client: Arc::new(client),
            tx,
            rx,
            origin,
            recorder,
            agents,
            last_tick: Instant::now(),
            dirty: true,
            control: None,
            waiting: Vec::new(),
            frame: Vec::new(),
            size: (0, 0),
            hold_external: false,
            held: Vec::new(),
        }
    }

    /// Start a new session in place: another workspace's `client`, and a
    /// fresh `app`, since nothing loaded from one workspace means anything in
    /// the next. Answers still on their way from the last session have
    /// nowhere to land. The view recorder, the control channel, and the
    /// commands waiting on it carry over.
    pub fn restart(&mut self, app: App, client: LinearClient) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.app = app;
        self.client = Arc::new(client);
        self.tx = tx;
        self.rx = rx;
        self.cache = ui::Cache::default();
        self.dirty = true;
    }

    /// Take commands from agents.
    pub fn accept_control(&mut self, commands: mpsc::UnboundedReceiver<Asked>) {
        self.control = Some(commands);
    }

    /// Hold what would act outside linear-tui instead of doing it.
    pub fn hold_external(&mut self) {
        self.hold_external = true;
    }

    /// Say, in the screen agents read, what was not done at the desktop.
    pub fn note_held(&mut self, what: String) {
        self.held.push(what);
    }

    /// Spawn everything the UI has queued since the last pass, including a
    /// palette query that has rested until `now`. Each request runs on the
    /// tokio runtime, so the UI never blocks on the network.
    pub fn send_queued(&mut self, now: Instant) {
        if self.app.flush_palette_search(now) {
            self.dirty = true;
        }
        while let Some(req) = self.app.outbox.requests.pop_front() {
            self.app.outbox.inflight += 1;
            self.dirty = true;
            if self.hold_external
                && let Some((what, done)) = external(&req)
            {
                self.held.push(what);
                let _ = self.tx.send(Message::Mutated(done));
                continue;
            }
            let client = Arc::clone(&self.client);
            let tx = self.tx.clone();
            let per_page = self.app.items_per_page;
            tokio::spawn(async move {
                let msg = dispatch::execute_request(&client, req, per_page).await;
                let _ = tx.send(msg);
            });
        }
    }

    /// Draw a frame if anything changed since the last one.
    pub fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B: Backend,
        B::Error: Send + Sync + 'static,
    {
        if !self.dirty {
            return Ok(());
        }
        let completed = terminal.draw(|f| ui::draw(f, &mut self.app, &mut self.cache))?;
        if self.control.is_some() {
            self.frame = text_of(completed.buffer);
            self.size = (completed.area.width, completed.area.height);
        }
        self.dirty = false;
        Ok(())
    }

    /// Handle every answer that has arrived, without waiting for more.
    /// Returns whether there was one.
    pub fn receive(&mut self) -> bool {
        let mut moved = false;
        while let Ok(msg) = self.rx.try_recv() {
            self.app.outbox.inflight = self.app.outbox.inflight.saturating_sub(1);
            self.app.handle_message(msg);
            moved = true;
        }
        moved
    }

    /// Carry out the commands agents have sent. Returns whether any came.
    pub fn serve_control(&mut self) -> bool {
        let mut asked = Vec::new();
        if let Some(control) = &mut self.control {
            while let Ok(one) = control.try_recv() {
                asked.push(one);
            }
        }
        let moved = !asked.is_empty();
        for (command, answer) in asked {
            match self.carry_out(command) {
                Ok(()) => self.waiting.push((answer, Instant::now())),
                Err(reply) => {
                    let _ = answer.send(reply);
                }
            }
        }
        if moved {
            self.dirty = true;
        }
        moved
    }

    /// Do what a command asks. An error is answered at once; anything else
    /// is answered with the screen once Linear has caught up.
    fn carry_out(&mut self, command: Command) -> Result<(), Reply> {
        match command {
            Command::Screen => {}
            Command::Press { keys } => {
                let keys = notation::parse(&keys).map_err(|e| Reply::error(e.to_string()))?;
                for key in keys {
                    event::handle(&mut self.app, Event::Key(key));
                }
            }
            Command::Type { text } => {
                for c in text.chars() {
                    event::handle(&mut self.app, Event::Key(notation::char_key(c)));
                }
            }
            Command::Run { title } => {
                self.app.cancel_restore();
                palette::run_command(&mut self.app, &title).map_err(Reply::error)?;
            }
            Command::Open { issue } => {
                let key = crate::adapter::cli::issue_key(&issue)
                    .map_err(|e| Reply::error(e.to_string()))?;
                self.app.cancel_restore();
                self.app.open_issue_by_identifier(&key);
            }
            Command::Quit => {
                self.app.quit();
                return Err(Reply::done());
            }
        }
        Ok(())
    }

    /// Whether everything asked of Linear has been answered and drawn.
    fn settled(&self) -> bool {
        self.app.outbox.requests.is_empty()
            && self.app.outbox.inflight == 0
            && self.app.view.palette.search_due.is_none()
            && !self.dirty
    }

    /// Answer the commands waiting on Linear, once it has answered — or once
    /// they have waited long enough, with the screen still loading.
    pub fn answer_waiting(&mut self, now: Instant) {
        if self.waiting.is_empty() {
            return;
        }
        let settled = self.settled();
        let (ready, still): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|(_, since)| settled || now.duration_since(*since) >= SETTLE_TIMEOUT);
        self.waiting = still;
        if ready.is_empty() {
            return;
        }
        // The agent may read `linear-tui context` next: record the view it
        // is about to be told of, rather than after the usual rest.
        if let Some(recorder) = &mut self.recorder
            && let Some(snapshot) = self.app.snapshot(&self.origin, timestamp_now())
        {
            recorder.record(snapshot);
        }
        let report = screen::report(&self.app, self.frame.clone(), self.size, self.held.clone());
        for (answer, _) in ready {
            let _ = answer.send(Reply::screen(&report));
        }
    }

    /// End a pass: record the view once it has rested, follow herdr's
    /// agents, and advance the spinner. `moved` says whether input or an
    /// answer was handled in this pass.
    pub fn settle(&mut self, moved: bool, now: Instant) {
        // This is the only place a snapshot is written — never from rendering.
        if let Some(recorder) = &mut self.recorder {
            if moved {
                recorder.touch(now);
            }
            if recorder.is_due(now)
                && let Some(snapshot) = self.app.snapshot(&self.origin, timestamp_now())
            {
                recorder.record(snapshot);
            }
        }
        self.dirty |= moved;

        if let Some(watch) = &mut self.agents
            && let Some(list) = watch.poll(now)
        {
            self.app.set_agents(list);
            self.dirty = true;
        }

        if self.app.loading() && self.last_tick.elapsed() >= TICK {
            self.app.tick_spinner();
            self.last_tick = Instant::now();
            self.dirty = true;
        }
    }

    /// Draw the next frame even if nothing seems to have changed.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// The instance is quitting: write the view one last time.
    pub fn close(&mut self) {
        if let Some(recorder) = &mut self.recorder
            && let Some(snapshot) = self.app.snapshot(&self.origin, timestamp_now())
        {
            recorder.close(snapshot);
        }
    }
}

/// For a request that acts outside linear-tui rather than on Linear: what
/// it would do, and the status line it ends with.
fn external(request: &Request) -> Option<(String, &'static str)> {
    let browser = |url: &str| {
        (
            format!("would open {url} in a browser"),
            "Opened in browser",
        )
    };
    match request {
        Request::Issue(issue::Request::OpenInBrowser(url))
        | Request::Project(project::Request::OpenInBrowser(url))
        | Request::Favorite(favorite::Request::OpenInBrowser(url)) => Some(browser(url)),
        Request::Notes(notes::Request::Deliver(handoff)) => {
            Some(("would hand the notes to herdr".into(), handoff.done()))
        }
        Request::Agent(agent::Request::Focus { pane }) => Some((
            format!("would bring herdr pane {pane} to the front"),
            "Switched to the agent",
        )),
        _ => None,
    }
}

/// A frame as text, one line per row, trailing blanks trimmed. A wide
/// character is one `char`, though it fills two cells.
fn text_of(buffer: &Buffer) -> Vec<String> {
    let area = buffer.area;
    (area.y..area.y + area.height)
        .map(|y| {
            let mut line = String::new();
            let mut x = area.x;
            while x < area.x + area.width {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                x += symbol.width().max(1) as u16;
            }
            line.trim_end().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    #[test]
    fn a_frame_reads_as_text_with_wide_characters_whole() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 2));
        buffer.set_string(0, 0, "天気 ok", ratatui::style::Style::default());
        buffer.set_string(0, 1, "row two", ratatui::style::Style::default());
        assert_eq!(text_of(&buffer), ["天気 ok", "row two"]);
    }

    #[test]
    fn a_browser_or_herdr_hand_off_is_external() {
        let open = Request::Issue(issue::Request::OpenInBrowser("https://x".into()));
        assert_eq!(
            external(&open),
            Some((
                "would open https://x in a browser".into(),
                "Opened in browser"
            ))
        );
        let status = Request::Issue(issue::Request::Detail {
            issue_id: "i".into(),
        });
        assert_eq!(external(&status), None);
    }

    /// A test terminal the runtime can draw on.
    fn terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(80, 20)).unwrap()
    }

    #[tokio::test]
    async fn an_agents_keys_are_answered_with_the_screen_they_lead_to() {
        let mut app = App::new(&crate::config::Config::default());
        app.outbox.requests.clear();
        app.store.teams =
            vec![serde_json::from_str(r#"{"id":"t","name":"Engineering","key":"ENG"}"#).unwrap()];
        let client = LinearClient::with_header("unused".into());
        let origin = Origin {
            workspace: "/repo".into(),
            cwd: "/repo".into(),
            pid: 1,
            herdr_pane: None,
        };
        let mut runtime = Runtime::new(app, client, origin, None);
        let (tx, rx) = mpsc::unbounded_channel();
        runtime.accept_control(rx);
        let mut terminal = terminal();

        let (answer, answered) = oneshot::channel();
        tx.send((Command::Press { keys: "?".into() }, answer))
            .unwrap();
        assert!(runtime.serve_control());
        runtime.draw(&mut terminal).unwrap();
        runtime.answer_waiting(Instant::now());
        let reply = answered.await.unwrap();
        let screen = reply.screen.unwrap();
        assert!(screen["overlay"].as_str().unwrap().starts_with("help"));
        assert!(
            screen["lines"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l.as_str().unwrap().contains("Help"))
        );

        let (answer, answered) = oneshot::channel();
        tx.send((
            Command::Press {
                keys: "<Nope>".into(),
            },
            answer,
        ))
        .unwrap();
        runtime.serve_control();
        assert!(
            answered
                .await
                .unwrap()
                .error
                .unwrap()
                .contains("unknown key")
        );
    }
}
