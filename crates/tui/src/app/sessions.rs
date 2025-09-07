use unicode_segmentation::UnicodeSegmentation;

use super::App;

impl App {
    pub fn sidebar_inner_height(&self) -> u16 {
        self.sidebar_area
            .map(|a| a.height.saturating_sub(2))
            .unwrap_or(0)
    }

    pub fn sidebar_max_scroll(&self) -> u16 {
        let h = self.sidebar_inner_height() as usize;
        if h == 0 {
            0
        } else {
            self.sessions.len().saturating_sub(h) as u16
        }
    }

    pub fn sidebar_select_up(&mut self) {
        if self.current_session > 0 {
            self.current_session -= 1;
        }
        self.ensure_sidebar_visible();
        let _ = crate::persist::save_state(self);
        self.load_current_session_messages();
    }

    pub fn sidebar_select_down(&mut self) {
        if self.current_session + 1 < self.sessions.len() {
            self.current_session += 1;
        }
        self.ensure_sidebar_visible();
        let _ = crate::persist::save_state(self);
        self.load_current_session_messages();
    }

    pub fn ensure_sidebar_visible(&mut self) {
        let start = self.sidebar_scroll as usize;
        let h = self.sidebar_inner_height() as usize;
        if h == 0 {
            return;
        }
        let end = start + h.saturating_sub(1);
        if self.current_session < start {
            self.sidebar_scroll = self.current_session as u16;
        } else if self.current_session > end {
            self.sidebar_scroll = (self.current_session + 1 - h) as u16;
        }
        self.sidebar_scroll = self.sidebar_scroll.min(self.sidebar_max_scroll());
    }

    pub fn sidebar_new_session(&mut self) {
        let idx = self.sessions.len() + 1;
        let name = format!("session-{}", idx);
        self.sessions.push(name);
        self.current_session = self.sessions.len() - 1;
        self.ensure_sidebar_visible();
        let _ = crate::persist::save_state(self);
        self.messages.clear();
        let _ = crate::persist::save_session(self.current_session_name(), &self.messages);
    }

    pub fn sidebar_rename_current(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.current_session.min(self.sessions.len() - 1);
        let buffer = self.sessions[idx].clone();
        let cursor = buffer.graphemes(true).count();
        self.rename = Some(super::RenameState {
            index: idx,
            buffer,
            cursor,
        });
    }

    pub fn sidebar_delete_current(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let idx = self.current_session.min(self.sessions.len() - 1);
        self.confirm = Some(super::ConfirmState {
            action: super::ConfirmAction::DeleteSession(idx),
        });
    }

    pub fn current_session_name(&self) -> &str {
        &self.sessions[self.current_session]
    }

    pub fn load_current_session_messages(&mut self) {
        if let Ok(msgs) = crate::persist::load_session(self.current_session_name()) {
            self.messages = msgs;
            self.chat_wrap_width = 0;
            self.chat_cache.clear();
            self.chat_total_lines = 0;
            self.collapsed.clear();
            self.chat_scroll = 0;
        }
    }
}
