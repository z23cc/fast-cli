use textwrap::{wrap, Options};
use unicode_width::UnicodeWidthStr;

use crate::strings::{PREFIX_ASSISTANT, PREFIX_USER};

use super::{App, Message, Role, WrappedMsg};

impl App {
    // Compute displayed lines for a message considering collapse/threshold rules.
    pub fn message_display_info(&self, idx: usize) -> (usize, bool) {
        let base = self.chat_cache.get(idx).map(|w| w.lines.len()).unwrap_or(0);
        let collapsed = self.collapsed.get(idx).copied().unwrap_or(false);
        let preview = self.collapse_preview_lines;
        let threshold = self.collapse_threshold_lines;
        let display = if collapsed && base > preview {
            preview
        } else {
            base
        };
        let has_indicator = if collapsed && base > preview {
            true
        } else {
            !collapsed && base > threshold
        };
        (display, has_indicator)
    }

    // Total effective lines including indicators.
    pub fn effective_total_lines(&self) -> usize {
        let mut total = 0usize;
        for i in 0..self.chat_cache.len() {
            let (d, ind) = self.message_display_info(i);
            total += d + if ind { 1 } else { 0 };
        }
        total
    }

    // Compute viewport, max_scroll, and start_offset from current scroll state.
    pub fn compute_chat_layout(&self, inner_height: u16) -> (usize, u16, usize, usize) {
        let viewport = inner_height.saturating_sub(0).max(1) as usize;
        let effective_total = self.effective_total_lines();
        let max_scroll = effective_total.saturating_sub(viewport) as u16;
        let distance_from_bottom = if self.stick_to_bottom {
            0
        } else {
            self.chat_scroll.min(max_scroll)
        };
        let start_offset = max_scroll.saturating_sub(distance_from_bottom) as usize;
        (viewport, max_scroll, start_offset, effective_total)
    }

    // Adjust chat_scroll to bring a global line index into view.
    pub fn set_scroll_to_show_global(&mut self, inner_height: u16, global_line: usize) {
        let (viewport, max_scroll, _start, effective_total) =
            self.compute_chat_layout(inner_height);
        let _ = viewport; // not used further, but kept for clarity
        if effective_total == 0 {
            self.chat_scroll = 0;
            self.stick_to_bottom = true;
            return;
        }
        let y_offset = global_line.min(effective_total.saturating_sub(1));
        self.chat_scroll = max_scroll.saturating_sub(y_offset as u16).min(max_scroll);
        self.stick_to_bottom = self.chat_scroll == 0;
    }
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
