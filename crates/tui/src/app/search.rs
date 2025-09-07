use super::{App, SearchHit};

impl App {
    pub fn open_search(&mut self) {
        self.search_input = Some(super::SearchInput {
            buffer: String::new(),
            cursor: 0,
        });
    }

    pub fn commit_search(&mut self) {
        if let Some(si) = &self.search_input {
            let q = si.buffer.clone();
            self.search_query = if q.is_empty() { None } else { Some(q) };
        }
        self.search_input = None;
        self.recompute_search_hits();
        self.search_current = 0;
        self.reveal_current_search_hit();
    }

    pub fn recompute_search_hits(&mut self) {
        self.search_hits.clear();
        let Some(q) = &self.search_query else {
            return;
        };
        if q.is_empty() {
            return;
        }
        for (mi, w) in self.chat_cache.iter().enumerate() {
            for (li, line) in w.lines.iter().enumerate() {
                let mut start = 0usize;
                while let Some(pos) = line[start..].find(q) {
                    let s = start + pos;
                    let e = s + q.len();
                    self.search_hits.push(SearchHit {
                        msg_idx: mi,
                        line_idx: li,
                        start: s,
                        end: e,
                    });
                    start = e;
                }
            }
        }
    }

    pub fn next_search_hit(&mut self) {
        if self.search_hits.is_empty() {
            return;
        }
        self.search_current = (self.search_current + 1) % self.search_hits.len();
        self.reveal_current_search_hit();
    }

    pub fn prev_search_hit(&mut self) {
        if self.search_hits.is_empty() {
            return;
        }
        if self.search_current == 0 {
            self.search_current = self.search_hits.len() - 1;
        } else {
            self.search_current -= 1;
        }
        self.reveal_current_search_hit();
    }

    pub fn reveal_current_search_hit(&mut self) {
        if self.search_hits.is_empty() {
            return;
        }
        let hit = &self.search_hits[self.search_current];
        if let Some(collapsed) = self.collapsed.get(hit.msg_idx).copied() {
            let base = self
                .chat_cache
                .get(hit.msg_idx)
                .map(|w| w.lines.len())
                .unwrap_or(0);
            if collapsed
                && hit.line_idx >= self.collapse_preview_lines
                && base > self.collapse_preview_lines
                && hit.msg_idx < self.collapsed.len()
            {
                self.collapsed[hit.msg_idx] = false;
            }
        }
        let mut acc = 0usize;
        for (i, w) in self.chat_cache.iter().enumerate() {
            if i == hit.msg_idx {
                break;
            }
            let base = w.lines.len();
            let collapsed = self.collapsed.get(i).copied().unwrap_or(false);
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
            acc += display + if has_indicator { 1 } else { 0 };
        }
        let base = self
            .chat_cache
            .get(hit.msg_idx)
            .map(|w| w.lines.len())
            .unwrap_or(0);
        let collapsed = self.collapsed.get(hit.msg_idx).copied().unwrap_or(false);
        let preview = self.collapse_preview_lines;
        let threshold = self.collapse_threshold_lines;
        let display = if collapsed && base > preview {
            preview
        } else {
            base
        };
        let _has_indicator = if collapsed && base > preview {
            true
        } else {
            !collapsed && base > threshold
        };
        let global = acc + hit.line_idx.min(display.saturating_sub(1));
        if let Some(area) = self.chat_area {
            let inner_h = area.height.saturating_sub(2) as usize;
            let mut total_effective = 0usize;
            for (i, w) in self.chat_cache.iter().enumerate() {
                let b = w.lines.len();
                let c = self.collapsed.get(i).copied().unwrap_or(false);
                let disp = if c && b > preview { preview } else { b };
                let has_ind = if c && b > preview {
                    true
                } else {
                    !c && b > threshold
                };
                total_effective += disp + if has_ind { 1 } else { 0 };
            }
            let viewport = inner_h.max(1);
            let max_scroll = total_effective.saturating_sub(viewport) as u16;
            let y_offset = global.min(total_effective.saturating_sub(1));
            self.chat_scroll = max_scroll.saturating_sub(y_offset as u16).min(max_scroll);
        }
    }
}

// tests removed as requested
