use textwrap::{wrap, Options};
use unicode_width::UnicodeWidthStr;

use crate::strings::{PREFIX_ASSISTANT, PREFIX_USER};

use super::{App, Message, Role, WrappedMsg};

impl App {
    pub fn ensure_chat_wrapped(&mut self, width: u16) {
        let width = width.max(1);
        if self.chat_wrap_width != width || self.chat_cache.len() != self.messages.len() {
            self.chat_cache.clear();
            for m in &self.messages {
                self.chat_cache.push(Self::wrap_message(m, width));
            }
            self.chat_total_lines = self.chat_cache.iter().map(|w| w.lines.len()).sum();
            self.chat_wrap_width = width;
            if self.collapsed.len() != self.messages.len() {
                let old_len = self.collapsed.len();
                self.collapsed.resize(self.messages.len(), false);
                for i in old_len..self.messages.len() {
                    let lines = self.chat_cache.get(i).map(|w| w.lines.len()).unwrap_or(0);
                    self.collapsed[i] = lines > self.collapse_threshold_lines;
                }
            }
            return;
        }
        if let (Some(last_msg), Some(last_wrap)) = (self.messages.last(), self.chat_cache.last()) {
            if last_msg.content.len() != last_wrap.content_len {
                let idx = self.messages.len() - 1;
                self.chat_cache[idx] = Self::wrap_message(last_msg, width);
                self.chat_total_lines = self.chat_cache.iter().map(|w| w.lines.len()).sum();
            }
        }
    }

    pub fn toggle_collapse_at(&mut self, idx: usize) {
        if idx < self.collapsed.len() {
            self.collapsed[idx] = !self.collapsed[idx];
        }
    }

    fn wrap_message(m: &Message, width: u16) -> WrappedMsg {
        let prefix = match m.role {
            Role::User => PREFIX_USER,
            Role::Assistant => PREFIX_ASSISTANT,
        };
        let full = format!("{}{}", prefix, m.content);
        let indent_width = UnicodeWidthStr::width(prefix);
        let indent = " ".repeat(indent_width);
        let opts = Options::new(width as usize).subsequent_indent(&indent);
        let lines = wrap(&full, opts)
            .into_iter()
            .map(|c| c.into_owned())
            .collect::<Vec<_>>();
        WrappedMsg {
            role: m.role.clone(),
            content_len: m.content.len(),
            lines,
        }
    }
}

// tests removed as requested
