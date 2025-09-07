use unicode_segmentation::UnicodeSegmentation;

use super::App;

impl App {
    pub fn insert_text(&mut self, s: &str) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let idx = self.input_cursor.min(parts.len());
        let mut new_input = String::new();
        for g in &parts[..idx] {
            new_input.push_str(g);
        }
        new_input.push_str(s);
        for g in &parts[idx..] {
            new_input.push_str(g);
        }
        self.input = new_input;
        let added = s.graphemes(true).count();
        self.input_cursor = (idx + added).min(self.input.graphemes(true).count());
    }

    pub fn delete_left_grapheme(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let mut parts: Vec<&str> = self.input.graphemes(true).collect();
        let idx = self.input_cursor;
        parts.remove(idx - 1);
        self.input = parts.concat();
        self.input_cursor = idx - 1;
    }

    pub fn delete_right_grapheme(&mut self) {
        let mut parts: Vec<&str> = self.input.graphemes(true).collect();
        let idx = self.input_cursor.min(parts.len());
        if idx < parts.len() {
            parts.remove(idx);
            self.input = parts.concat();
        }
    }

    pub fn move_cursor_line_start(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut i = self.input_cursor.min(parts.len());
        while i > 0 {
            if parts[i - 1] == "\n" {
                break;
            }
            i -= 1;
        }
        self.input_cursor = i;
    }

    pub fn move_cursor_line_end(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut i = self.input_cursor.min(parts.len());
        while i < parts.len() {
            if parts[i] == "\n" {
                break;
            }
            i += 1;
        }
        self.input_cursor = i;
    }

    pub fn delete_prev_word(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        if self.input_cursor == 0 {
            return;
        }
        let mut i = self.input_cursor;
        while i > 0 && parts[i - 1].trim().is_empty() {
            i -= 1;
        }
        while i > 0 && !parts[i - 1].trim().is_empty() {
            i -= 1;
        }
        let mut newp = parts.clone();
        newp.drain(i..self.input_cursor);
        self.input = newp.concat();
        self.input_cursor = i;
    }

    pub fn kill_to_line_start(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut start = self.input_cursor.min(parts.len());
        while start > 0 {
            if parts[start - 1] == "\n" {
                break;
            }
            start -= 1;
        }
        let mut newp = parts.clone();
        newp.drain(start..self.input_cursor);
        self.input = newp.concat();
        self.input_cursor = start;
    }

    pub fn kill_to_line_end(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut end = self.input_cursor.min(parts.len());
        while end < parts.len() {
            if parts[end] == "\n" {
                break;
            }
            end += 1;
        }
        let mut newp = parts.clone();
        newp.drain(self.input_cursor..end);
        self.input = newp.concat();
    }

    pub fn move_cursor_word_left(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut i = self.input_cursor.min(parts.len());
        while i > 0 && parts[i - 1].trim().is_empty() {
            i -= 1;
        }
        while i > 0 && !parts[i - 1].trim().is_empty() {
            i -= 1;
        }
        self.input_cursor = i;
    }

    pub fn move_cursor_word_right(&mut self) {
        let parts: Vec<&str> = self.input.graphemes(true).collect();
        let mut i = self.input_cursor.min(parts.len());
        while i < parts.len() && parts[i].trim().is_empty() {
            i += 1;
        }
        while i < parts.len() && !parts[i].trim().is_empty() {
            i += 1;
        }
        self.input_cursor = i;
    }
}
