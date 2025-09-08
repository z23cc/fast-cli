use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use fast_core::llm::ModelClient as _;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use unicode_segmentation::UnicodeSegmentation;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub mod chat;
pub mod history;
pub mod input;
pub mod search;
pub mod sessions;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user<S: Into<String>>(s: S) -> Self {
        Self {
            role: Role::User,
            content: s.into(),
        }
    }
    pub fn assistant<S: Into<String>>(s: S) -> Self {
        Self {
            role: Role::Assistant,
            content: s.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Input,
    Sidebar,
    Context,
}

pub struct RenameState {
    pub index: usize,
    pub buffer: String,
    pub cursor: usize,
}

#[derive(Clone)]
pub struct ConfirmState {
    pub action: ConfirmAction,
}

#[derive(Clone)]
pub enum ConfirmAction {
    DeleteSession(usize),
}

pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub input_cursor: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub sessions: Vec<String>,
    pub current_session: usize,
    pub should_quit: bool,
    pub chat_scroll: u16,
    tick: u64,
    stream: Option<StreamState>,
    pub show_sidebar: bool,
    pub show_help: bool,
    pub chat_area: Option<Rect>,
    pub sidebar_area: Option<Rect>,
    pub sidebar_scroll: u16,
    pub focus: Focus,
    pub rename: Option<RenameState>,
    pub confirm: Option<ConfirmState>,
    pub chat_wrap_width: u16,
    pub chat_cache: Vec<WrappedMsg>,
    pub chat_total_lines: usize,
    pub collapsed: Vec<bool>,
    pub collapse_preview_lines: usize,
    pub collapse_threshold_lines: usize,
    pub search_input: Option<SearchInput>,
    pub search_query: Option<String>,
    pub search_hits: Vec<SearchHit>,
    pub search_current: usize,
    pub stick_to_bottom: bool,
    pub chat_viewport: u16,
    pub input_visible_lines: u16,
    pub input_max_lines: u16,
    pub dirty: bool,
    // Context pane
    pub show_context: bool,
    pub context_items: Vec<String>,
    pub context_area: Option<ratatui::layout::Rect>,
    pub context_scroll: u16,
    pub context_current: usize,
    pub palette: Option<PaletteState>,
    pub model_picker: Option<ModelPickerState>,
    pub llm_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    pub llm_cancel: Option<Arc<AtomicBool>>,
    // Provider/model info for status bar
    pub provider_label: String,
    pub model_label: String,
    pub wire_label: String,
}

impl App {
    // Returns true if a supported slash command was handled
    fn try_handle_slash_command(&mut self, text: &str) -> bool {
        let s = text.trim();
        if !s.starts_with('/') {
            return false;
        }
        // Very small parser: /model <name> | /wire <responses|chat|auto>
        let rest = &s[1..];
        let mut parts = rest.splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or("").to_lowercase();
        let arg = parts.next().unwrap_or("").trim();
        match cmd.as_str() {
            "model" => {
                if arg.is_empty() {
                    self.open_model_picker();
                    self.dirty = true;
                    return true;
                }
                self.model_label = arg.to_string();
                let _ = crate::persist::save_state(self);
                // Show an inline info line to the user
                self.messages.push(Message::assistant(format!(
                    "[info] model set to '{}'",
                    self.model_label
                )));
                self.collapsed.push(false);
                true
            }
            "wire" => {
                let v = arg.to_lowercase();
                if matches!(v.as_str(), "responses" | "chat" | "auto") {
                    self.wire_label = v;
                    let _ = crate::persist::save_state(self);
                    self.messages.push(Message::assistant(format!(
                        "[info] wire set to '{}'",
                        self.wire_label
                    )));
                    self.collapsed.push(false);
                }
                true
            }
            _ => true, // Unknown slash cmd: consume it quietly
        }
    }
    pub fn new() -> Self {
        let mut s = Self {
            messages: vec![Message::assistant("Welcome to fast TUI (preview). Enter: send; Shift+Enter: newline; Esc/Ctrl-C: quit.")],
            input: String::new(),
            input_cursor: 0,
            history: Vec::new(),
            history_index: None,
            sessions: vec!["default".to_string()],
            current_session: 0,
            should_quit: false,
            chat_scroll: 0,
            tick: 0,
            stream: None,
            show_sidebar: true,
            show_help: false,
            chat_area: None,
            sidebar_area: None,
            sidebar_scroll: 0,
            focus: Focus::Input,
            rename: None,
            confirm: None,
            chat_wrap_width: 0,
            chat_cache: Vec::new(),
            chat_total_lines: 0,
            collapsed: Vec::new(),
            collapse_preview_lines: 8,
            collapse_threshold_lines: 40,
            search_input: None,
            search_query: None,
            search_hits: Vec::new(),
            search_current: 0,
            stick_to_bottom: true,
            chat_viewport: 0,
            input_visible_lines: 1,
            input_max_lines: 6,
            dirty: true,
            show_context: false,
            context_items: Vec::new(),
            context_area: None,
            context_scroll: 0,
            context_current: 0,
            palette: None,
            model_picker: None,
            llm_rx: None,
            llm_cancel: None,
            provider_label: String::from("OpenAI"),
            model_label: String::from("gpt-5"),
            wire_label: String::from("responses"),
        };
        // Try to read provider config for status
        if let Ok(cfg) = providers::openai::config::OpenAiConfig::from_env_and_file() {
            s.model_label = cfg.model.clone();
            s.wire_label = cfg.wire_api.clone();
        }
        if let Ok(Some(p)) = crate::persist::load_state() {
            if !p.sessions.is_empty() {
                s.sessions = p.sessions;
            }
            if !s.sessions.is_empty() {
                s.current_session = p.current_session.min(s.sessions.len() - 1);
            }
            s.show_sidebar = p.show_sidebar;
            s.sidebar_scroll = p.sidebar_scroll;
            if let Some(m) = p.model {
                s.model_label = m;
            }
            if let Some(w) = p.wire_api {
                s.wire_label = w;
            }
        }
        if !s.sessions.is_empty() {
            if let Ok(msgs) = crate::persist::load_session(&s.sessions[s.current_session]) {
                if !msgs.is_empty() {
                    s.messages = msgs;
                }
            }
        }
        s
    }

    pub fn submit(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }

        // Slash commands (e.g., /model <name>, /wire <responses|chat|auto>)
        if self.try_handle_slash_command(&text) {
            self.input.clear();
            self.input_cursor = 0;
            self.dirty = true;
            return;
        }

        self.record_history_entry(&text);
        self.messages.push(Message::user(text.clone()));
        self.collapsed.push(false);

        let _assistant_index = self.messages.len();
        self.messages.push(Message::assistant(String::new()));
        self.collapsed.push(false);
        // Start real LLM streaming in a background thread
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        self.llm_rx = Some(rx);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.llm_cancel = Some(cancel_flag.clone());
        // Build snapshot for provider: drop any assistant messages before the
        // first user message (e.g., the initial welcome banner), and skip
        // empty assistant placeholders we append for streaming.
        let first_user_idx = self
            .messages
            .iter()
            .position(|m| matches!(m.role, Role::User))
            .unwrap_or(0);
        let msgs_snapshot = self.messages[first_user_idx..]
            .iter()
            .filter(|m| !(matches!(m.role, Role::Assistant) && m.content.trim().is_empty()))
            .map(|m| fast_core::llm::Message {
                role: match m.role {
                    Role::User => fast_core::llm::Role::User,
                    Role::Assistant => fast_core::llm::Role::Assistant,
                },
                content: m.content.clone(),
            })
            .collect::<Vec<_>>();
        // Log submit intent (model/wire)
        info!(target: "tui", "submit: model={} wire={} input_len={} chars", self.model_label, self.wire_label, text.len());
        // Capture runtime selections for this request
        let selected_model = self.model_label.clone();
        let selected_wire = self.wire_label.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("rt");
            let _ = rt.block_on(async move {
                let cfg = match providers::openai::config::OpenAiConfig::from_env_and_file() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(format!("config: {}", e)));
                        error!(target: "tui", "submit config error: {}", e);
                        return;
                    }
                };
                let client = match providers::openai::OpenAiClient::new(cfg.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(format!("client: {}", e)));
                        error!(target: "tui", "submit client build error: {}", e);
                        return;
                    }
                };
                let opts = fast_core::llm::ChatOpts {
                    model: selected_model.clone(),
                    temperature: None,
                    top_p: None,
                    max_tokens: None,
                };
                let wire = match selected_wire.as_str() {
                    "chat" => fast_core::llm::ChatWire::Chat,
                    "responses" => fast_core::llm::ChatWire::Responses,
                    "auto" => fast_core::llm::ChatWire::Auto,
                    _ => fast_core::llm::ChatWire::Responses,
                };
                let res = client.stream_chat(msgs_snapshot, opts, wire).await;
                match res {
                    Ok(mut s) => {
                        use futures::StreamExt;
                        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
                        loop {
                            tokio::select! {
                                _ = tick.tick() => {
                                    if cancel_flag.load(Ordering::Relaxed) {
                                        let _ = tx.send(Err("canceled".into()));
                                        break;
                                    }
                                }
                                it = s.next() => {
                                    match it {
                                        Some(Ok(fast_core::llm::ChatDelta::Text(t))) => { let _ = tx.send(Ok(t)); }
                                        Some(Ok(fast_core::llm::ChatDelta::Finish(_))) => { break; }
                                        Some(Ok(_)) => { /* ignore other events for now */ }
                                        Some(Err(e)) => {
                                            let _ = tx.send(Err(format!("{}", e)));
                                            error!(target: "tui", "stream delta error: {}", e);
                                            break;
                                        }
                                        None => { break; }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("{}", e)));
                        error!(target: "tui", "stream start error: {}", e);
                    }
                }
            });
        });
        self.input.clear();
        self.input_cursor = 0;
        self.stick_to_bottom = true;
        self.chat_scroll = 0;
        self.dirty = true;
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if let KeyEventKind::Press = key.kind {
            if let Some(p) = &mut self.palette {
                match key.code {
                    KeyCode::Esc => {
                        self.palette = None;
                    }
                    KeyCode::Enter => {
                        if let Some(act) = p.filtered.get(p.selected).cloned() {
                            self.execute_palette_action(&act);
                            self.palette = None;
                        }
                    }
                    KeyCode::Up => {
                        if p.selected > 0 {
                            p.selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if p.selected + 1 < p.filtered.len() {
                            p.selected += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if p.cursor > 0 {
                            let mut parts: Vec<&str> = p.buffer.graphemes(true).collect();
                            let c = p.cursor.min(parts.len());
                            parts.remove(c - 1);
                            p.buffer = parts.concat();
                            p.cursor -= 1;
                            App::palette_filter(p);
                        }
                    }
                    KeyCode::Delete => {
                        let mut parts: Vec<&str> = p.buffer.graphemes(true).collect();
                        let c = p.cursor.min(parts.len());
                        if c < parts.len() {
                            parts.remove(c);
                            p.buffer = parts.concat();
                            App::palette_filter(p);
                        }
                    }
                    KeyCode::Left => {
                        if p.cursor > 0 {
                            p.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let l = p.buffer.graphemes(true).count();
                        if p.cursor < l {
                            p.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        p.cursor = 0;
                    }
                    KeyCode::End => {
                        p.cursor = p.buffer.graphemes(true).count();
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            let mut parts: Vec<&str> = p.buffer.graphemes(true).collect();
                            let c = p.cursor.min(parts.len());
                            let mut buf = [0u8; 4];
                            parts.insert(c, ch.encode_utf8(&mut buf));
                            p.buffer = parts.concat();
                            p.cursor += 1;
                            App::palette_filter(p);
                        }
                    }
                    _ => {}
                }
                return;
            }

            if self.model_picker.is_some() {
                let model_all = self.recommended_models();
                let st = match &mut self.model_picker {
                    Some(s) => s,
                    None => unreachable!(),
                };
                match key.code {
                    KeyCode::Esc => {
                        self.model_picker = None;
                    }
                    KeyCode::Enter => {
                        if let Some(sel) = st.filtered.get(st.selected).cloned() {
                            self.model_label = sel;
                            self.model_picker = None;
                            let _ = crate::persist::save_state(self);
                            self.messages.push(Message::assistant(format!(
                                "[info] model set to '{}'",
                                self.model_label
                            )));
                            self.collapsed.push(false);
                        }
                    }
                    KeyCode::Up => {
                        if st.selected > 0 {
                            st.selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if st.selected + 1 < st.filtered.len() {
                            st.selected += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if st.cursor > 0 {
                            let mut parts: Vec<&str> = st.buffer.graphemes(true).collect();
                            let c = st.cursor.min(parts.len());
                            parts.remove(c - 1);
                            st.buffer = parts.concat();
                            st.cursor -= 1;
                            App::model_filter(&model_all, st);
                        }
                    }
                    KeyCode::Delete => {
                        let mut parts: Vec<&str> = st.buffer.graphemes(true).collect();
                        let c = st.cursor.min(parts.len());
                        if c < parts.len() {
                            parts.remove(c);
                            st.buffer = parts.concat();
                            App::model_filter(&model_all, st);
                        }
                    }
                    KeyCode::Left => {
                        if st.cursor > 0 {
                            st.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let l = st.buffer.graphemes(true).count();
                        if st.cursor < l {
                            st.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        st.cursor = 0;
                    }
                    KeyCode::End => {
                        st.cursor = st.buffer.graphemes(true).count();
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            let mut parts: Vec<&str> = st.buffer.graphemes(true).collect();
                            let c = st.cursor.min(parts.len());
                            let mut buf = [0u8; 4];
                            parts.insert(c, ch.encode_utf8(&mut buf));
                            st.buffer = parts.concat();
                            st.cursor += 1;
                            App::model_filter(&model_all, st);
                        }
                    }
                    _ => {}
                }
                return;
            }

            if self.show_help {
                match key.code {
                    KeyCode::Esc | KeyCode::F(1) => {
                        self.show_help = false;
                    }
                    KeyCode::Char('?') => {
                        self.show_help = false;
                    }
                    _ => {}
                }
                return;
            }

            if let Some(state) = &mut self.search_input {
                match key.code {
                    KeyCode::Esc => {
                        self.search_input = None;
                    }
                    KeyCode::Enter => {
                        self.commit_search();
                    }
                    KeyCode::Backspace => {
                        if state.cursor > 0 {
                            let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                            let c = state.cursor.min(parts.len());
                            parts.remove(c - 1);
                            state.buffer = parts.concat();
                            state.cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                        let c = state.cursor.min(parts.len());
                        if c < parts.len() {
                            parts.remove(c);
                            state.buffer = parts.concat();
                        }
                    }
                    KeyCode::Left => {
                        if state.cursor > 0 {
                            state.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let l = state.buffer.graphemes(true).count();
                        if state.cursor < l {
                            state.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        state.cursor = 0;
                    }
                    KeyCode::End => {
                        state.cursor = state.buffer.graphemes(true).count();
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                            let c = state.cursor.min(parts.len());
                            let mut buf = [0u8; 4];
                            parts.insert(c, ch.encode_utf8(&mut buf));
                            state.buffer = parts.concat();
                            state.cursor += 1;
                        }
                    }
                    _ => {}
                }
                return;
            }

            if let Some(state) = &mut self.rename {
                match key.code {
                    KeyCode::Esc => {
                        self.rename = None;
                    }
                    KeyCode::Enter => {
                        let idx = state.index.min(self.sessions.len().saturating_sub(1));
                        if !state.buffer.trim().is_empty() {
                            let old = self.sessions[idx].clone();
                            let new_name = state.buffer.trim().to_string();
                            if new_name != old {
                                let _ = crate::persist::rename_session(&old, &new_name);
                                self.sessions[idx] = new_name;
                            }
                            self.current_session = idx;
                        }
                        self.rename = None;
                        let _ = crate::persist::save_state(self);
                    }
                    KeyCode::Backspace => {
                        if state.cursor > 0 {
                            let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                            let c = state.cursor.min(parts.len());
                            parts.remove(c - 1);
                            state.buffer = parts.concat();
                            state.cursor -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                        let c = state.cursor.min(parts.len());
                        if c < parts.len() {
                            parts.remove(c);
                            state.buffer = parts.concat();
                        }
                    }
                    KeyCode::Left => {
                        if state.cursor > 0 {
                            state.cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        let l = state.buffer.graphemes(true).count();
                        if state.cursor < l {
                            state.cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        state.cursor = 0;
                    }
                    KeyCode::End => {
                        state.cursor = state.buffer.graphemes(true).count();
                    }
                    KeyCode::Char(ch) => {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            let mut parts: Vec<&str> = state.buffer.graphemes(true).collect();
                            let c = state.cursor.min(parts.len());
                            let mut buf = [0u8; 4];
                            parts.insert(c, ch.encode_utf8(&mut buf));
                            state.buffer = parts.concat();
                            state.cursor += 1;
                        }
                    }
                    _ => {}
                }
                return;
            }

            if let Some(confirm) = self.confirm.clone() {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        match confirm.action {
                            ConfirmAction::DeleteSession(idx) => {
                                if idx < self.sessions.len() {
                                    let name = self.sessions.remove(idx);
                                    let _ = crate::persist::delete_session(&name);
                                    if self.sessions.is_empty() {
                                        self.sessions.push("default".to_string());
                                    }
                                    let new_idx = idx.min(self.sessions.len() - 1);
                                    self.current_session = new_idx;
                                }
                            }
                        }
                        self.confirm = None;
                        let _ = crate::persist::save_state(self);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        self.confirm = None;
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+C: cancel active stream if any; otherwise quit
                    if self.llm_rx.is_some() {
                        if let Some(cancel) = &self.llm_cancel { cancel.store(true, Ordering::Relaxed); }
                    } else {
                        self.should_quit = true;
                    }
                }
                KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.open_palette();
                }
                KeyCode::F(1) => {
                    self.show_help = true;
                }

                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.open_search();
                }
                KeyCode::F(3) if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.prev_search_hit();
                }
                KeyCode::F(3) => {
                    self.next_search_hit();
                }
                KeyCode::Tab => {
                    // Cycle focus across visible panes: Input -> Sidebar? -> Context? -> Input
                    let mut order = Vec::new();
                    order.push(Focus::Input);
                    if self.show_sidebar {
                        order.push(Focus::Sidebar);
                    }
                    if self.show_context {
                        order.push(Focus::Context);
                    }
                    // find next
                    if let Some(pos) = order.iter().position(|f| *f == self.focus) {
                        let next = (pos + 1) % order.len();
                        self.focus = order[next];
                    } else {
                        self.focus = Focus::Input;
                    }
                }

                KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    info!(target: "tui", "on_key: Shift+Enter => newline");
                    self.insert_text("\n");
                }
                KeyCode::Enter => {
                    if matches!(self.focus, Focus::Input) {
                        info!(target: "tui", "on_key: Enter => submit");
                        self.submit();
                    }
                }
                KeyCode::Backspace if matches!(self.focus, Focus::Input) => {
                    self.delete_left_grapheme();
                }
                KeyCode::Delete if matches!(self.focus, Focus::Input) => {
                    self.delete_right_grapheme();
                }
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.delete_prev_word();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.kill_to_line_start();
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.kill_to_line_end();
                }
                KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_cursor_line_start();
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_cursor_line_end();
                }
                KeyCode::Char(ch) => {
                    if matches!(self.focus, Focus::Context) {
                        match ch {
                            'a' | 'A' => {
                                self.open_context_add();
                            }
                            _ => {}
                        }
                    } else if matches!(self.focus, Focus::Sidebar) {
                        match ch {
                            'n' | 'N' => {
                                self.sidebar_new_session();
                            }
                            'r' | 'R' => {
                                self.sidebar_rename_current();
                            }
                            'd' | 'D' => {
                                self.sidebar_delete_current();
                            }
                            _ => {}
                        }
                    } else {
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        self.insert_text(s);
                    }
                }
                KeyCode::Left if key.modifiers.is_empty() && matches!(self.focus, Focus::Input) => {
                    if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                    }
                }
                KeyCode::Right
                    if key.modifiers.is_empty() && matches!(self.focus, Focus::Input) =>
                {
                    let len = self.input.graphemes(true).count();
                    if self.input_cursor < len {
                        self.input_cursor += 1;
                    }
                }
                KeyCode::Left
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(self.focus, Focus::Input) =>
                {
                    self.move_cursor_word_left();
                }
                KeyCode::Right
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(self.focus, Focus::Input) =>
                {
                    self.move_cursor_word_right();
                }
                KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.chat_scroll = u16::MAX;
                }
                KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.chat_scroll = 0;
                }
                KeyCode::Up if key.modifiers.is_empty() && matches!(self.focus, Focus::Input) => {
                    if self.history.is_empty() {
                        return;
                    }
                    let idx = match self.history_index {
                        None => self.history.len().saturating_sub(1),
                        Some(0) => 0,
                        Some(i) => i.saturating_sub(1),
                    };
                    self.history_index = Some(idx);
                    self.input = self.history[idx].clone();
                    self.input_cursor = self.input.graphemes(true).count();
                }
                KeyCode::Down if key.modifiers.is_empty() && matches!(self.focus, Focus::Input) => {
                    if let Some(i) = self.history_index {
                        if i + 1 < self.history.len() {
                            self.history_index = Some(i + 1);
                            self.input = self.history[i + 1].clone();
                            self.input_cursor = self.input.graphemes(true).count();
                        } else {
                            self.history_index = None;
                            self.input.clear();
                            self.input_cursor = 0;
                        }
                    }
                }
                KeyCode::Up if matches!(self.focus, Focus::Sidebar) => {
                    self.sidebar_select_up();
                }
                KeyCode::Down if matches!(self.focus, Focus::Sidebar) => {
                    self.sidebar_select_down();
                }
                KeyCode::PageUp if matches!(self.focus, Focus::Sidebar) => {
                    let step = self.sidebar_inner_height().max(1);
                    for _ in 0..step {
                        self.sidebar_select_up();
                    }
                }
                KeyCode::PageDown if matches!(self.focus, Focus::Sidebar) => {
                    let step = self.sidebar_inner_height().max(1);
                    for _ in 0..step {
                        self.sidebar_select_down();
                    }
                }
                KeyCode::Home if matches!(self.focus, Focus::Sidebar) => {
                    self.current_session = 0;
                    self.ensure_sidebar_visible();
                    let _ = crate::persist::save_state(self);
                }
                KeyCode::End if matches!(self.focus, Focus::Sidebar) => {
                    if !self.sessions.is_empty() {
                        self.current_session = self.sessions.len() - 1;
                    }
                    self.ensure_sidebar_visible();
                    let _ = crate::persist::save_state(self);
                }
                KeyCode::PageUp if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    let step = self.chat_viewport.saturating_mul(2).max(1);
                    self.chat_scroll = self.chat_scroll.saturating_add(step);
                    self.stick_to_bottom = false;
                }
                KeyCode::PageDown if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    let step = self.chat_viewport.saturating_mul(2).max(1);
                    self.chat_scroll = self.chat_scroll.saturating_sub(step);
                    if self.chat_scroll == 0 {
                        self.stick_to_bottom = true;
                    }
                }
                KeyCode::PageUp => {
                    let step = self.chat_viewport.max(1);
                    self.chat_scroll = self.chat_scroll.saturating_add(step);
                    self.stick_to_bottom = false;
                }
                KeyCode::PageDown => {
                    let step = self.chat_viewport.max(1);
                    self.chat_scroll = self.chat_scroll.saturating_sub(step);
                    if self.chat_scroll == 0 {
                        self.stick_to_bottom = true;
                    }
                }
                KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.chat_scroll = self.chat_scroll.saturating_add(1);
                    self.stick_to_bottom = false;
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.chat_scroll = self.chat_scroll.saturating_sub(1);
                    if self.chat_scroll == 0 {
                        self.stick_to_bottom = true;
                    }
                }
                KeyCode::F(2) => {
                    self.show_sidebar = !self.show_sidebar;
                    let _ = crate::persist::save_state(self);
                }
                KeyCode::F(6) => {
                    self.show_context = !self.show_context;
                    self.dirty = true;
                }
                KeyCode::Delete if matches!(self.focus, Focus::Sidebar) => {
                    self.sidebar_delete_current();
                }
                // Context pane shortcuts
                KeyCode::Up if matches!(self.focus, Focus::Context) => {
                    if self.context_current > 0 {
                        self.context_current -= 1;
                    }
                }
                KeyCode::Down if matches!(self.focus, Focus::Context) => {
                    if self.context_current + 1 < self.context_items.len() {
                        self.context_current += 1;
                    }
                }
                KeyCode::Delete if matches!(self.focus, Focus::Context) => {
                    if self.context_current < self.context_items.len() {
                        self.context_items.remove(self.context_current);
                        if self.context_current >= self.context_items.len()
                            && !self.context_items.is_empty()
                        {
                            self.context_current = self.context_items.len() - 1;
                        }
                    }
                }
                _ => {}
            }
            // Mark dirty on any handled key press path.
            self.dirty = true;
        }
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if let Some(stream) = &mut self.stream {
            let graphemes: Vec<&str> =
                UnicodeSegmentation::graphemes(stream.content.as_str(), true).collect();
            if stream.pos < graphemes.len() {
                let end = (stream.pos + 2).min(graphemes.len());
                let slice = graphemes[stream.pos..end].join("");
                stream.pos = end;
                if let Some(msg) = self.messages.get_mut(stream.target_index) {
                    msg.content.push_str(&slice);
                }
            }
            if stream.pos >= graphemes.len() {
                self.stream = None;
                self.stick_to_bottom = true;
                let _ = crate::persist::save_session(self.current_session_name(), &self.messages);
            }
            self.dirty = true;
        }
        // Drain LLM streaming receiver
        if let Some(rx) = &self.llm_rx {
            for _ in 0..64 {
                match rx.try_recv() {
                    Ok(Ok(s)) => {
                        if let Some(msg) = self.messages.last_mut() {
                            msg.content.push_str(&s);
                        }
                        self.dirty = true;
                        self.stick_to_bottom = true;
                    }
                    Ok(Err(e)) => {
                        if let Some(msg) = self.messages.last_mut() {
                            msg.content.push_str(&format!("\n[error] {}", e));
                        }
                        self.llm_rx = None;
                        self.llm_cancel = None;
                        let _ = crate::persist::save_session(self.current_session_name(), &self.messages);
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.llm_rx = None;
                        self.llm_cancel = None;
                        let _ = crate::persist::save_session(self.current_session_name(), &self.messages);
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct SearchInput {
    pub buffer: String,
    pub cursor: usize,
}

#[derive(Clone)]
pub struct SearchHit {
    pub msg_idx: usize,
    pub line_idx: usize,
    pub start: usize,
    pub end: usize,
}

struct StreamState {
    target_index: usize,
    content: String,
    pos: usize,
}

#[derive(Clone)]
pub struct PaletteState {
    pub buffer: String,
    pub cursor: usize,
    pub filtered: Vec<PaletteAction>,
    pub selected: usize,
}

#[derive(Clone)]
pub enum PaletteAction {
    ToggleSidebar,
    ToggleContext,
    NewSession,
    RenameSession,
    DeleteSession,
    OpenSearch,
    Quit,
}

impl PaletteAction {
    pub fn label(&self) -> &'static str {
        match self {
            PaletteAction::ToggleSidebar => "Toggle sidebar",
            PaletteAction::ToggleContext => "Toggle context",
            PaletteAction::NewSession => "New session",
            PaletteAction::RenameSession => "Rename session",
            PaletteAction::DeleteSession => "Delete session",
            PaletteAction::OpenSearch => "Open search",
            PaletteAction::Quit => "Quit",
        }
    }
}

impl App {
    pub fn open_palette(&mut self) {
        let mut st = PaletteState {
            buffer: String::new(),
            cursor: 0,
            filtered: Vec::new(),
            selected: 0,
        };
        self.refresh_palette_filtered(&mut st);
        self.palette = Some(st);
    }

    fn refresh_palette_filtered(&self, st: &mut PaletteState) {
        let all = vec![
            PaletteAction::ToggleSidebar,
            PaletteAction::ToggleContext,
            PaletteAction::NewSession,
            PaletteAction::RenameSession,
            PaletteAction::DeleteSession,
            PaletteAction::OpenSearch,
            PaletteAction::Quit,
        ];
        let q = st.buffer.to_lowercase();
        st.filtered = if q.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|a| a.label().to_lowercase().contains(&q))
                .collect()
        };
        st.selected = st.selected.min(st.filtered.len().saturating_sub(1));
    }

    fn execute_palette_action(&mut self, act: &PaletteAction) {
        match act {
            PaletteAction::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
                let _ = crate::persist::save_state(self);
            }
            PaletteAction::ToggleContext => {
                self.show_context = !self.show_context;
            }
            PaletteAction::NewSession => {
                self.sidebar_new_session();
            }
            PaletteAction::RenameSession => {
                self.sidebar_rename_current();
            }
            PaletteAction::DeleteSession => {
                self.sidebar_delete_current();
            }
            PaletteAction::OpenSearch => {
                self.open_search();
            }
            PaletteAction::Quit => {
                self.should_quit = true;
            }
        }
        self.dirty = true;
    }

    pub fn open_context_add(&mut self) {
        // Reuse search input as simple line editor for context entry (e.g., file path or note)
        self.search_input = Some(SearchInput {
            buffer: String::new(),
            cursor: 0,
        });
    }
}

impl App {
    fn palette_filter(st: &mut PaletteState) {
        let all = vec![
            PaletteAction::ToggleSidebar,
            PaletteAction::ToggleContext,
            PaletteAction::NewSession,
            PaletteAction::RenameSession,
            PaletteAction::DeleteSession,
            PaletteAction::OpenSearch,
            PaletteAction::Quit,
        ];
        let q = st.buffer.to_lowercase();
        st.filtered = if q.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|a| a.label().to_lowercase().contains(&q))
                .collect()
        };
        st.selected = st.selected.min(st.filtered.len().saturating_sub(1));
    }
}

#[derive(Clone)]
pub struct WrappedMsg {
    pub role: Role,
    pub content_len: usize,
    pub lines: Vec<String>,
}

#[derive(Clone)]
pub struct ModelPickerState {
    pub buffer: String,
    pub cursor: usize,
    pub filtered: Vec<String>,
    pub selected: usize,
}

impl App {
    fn open_model_picker(&mut self) {
        let filtered = self.recommended_models();
        self.model_picker = Some(ModelPickerState {
            buffer: String::new(),
            cursor: 0,
            filtered,
            selected: 0,
        });
    }

    fn recommended_models(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if !self.model_label.trim().is_empty() {
            out.push(self.model_label.clone());
        }
        for m in [
            // Codex presets for GPT-5 family
            "gpt-5",
            "gpt-5-high",
            "gpt-5-medium",
            "gpt-5-low",
            "gpt-5-minimal",
            // Other common models
            "gpt-4o",
            "gpt-4o-mini",
            "o3",
            "o3-mini",
        ]
        .iter()
        {
            if out.iter().all(|x| x != m) {
                out.push((*m).to_string());
            }
        }
        out
    }

    fn model_filter(all: &[String], st: &mut ModelPickerState) {
        let q = st.buffer.to_lowercase();
        if q.is_empty() {
            st.filtered = all.to_vec();
        } else {
            st.filtered = all
                .iter()
                .filter(|m| m.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        st.selected = st.selected.min(st.filtered.len().saturating_sub(1));
    }
}
