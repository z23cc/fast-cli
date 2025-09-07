use super::App;

impl App {
    // Record input text to history if it's new, and reset history navigation state.
    pub fn record_history_entry(&mut self, text: &str) {
        if let Some(last) = self.history.last() {
            if last == text {
                self.history_index = None;
                return;
            }
        }
        self.history.push(text.to_string());
        self.history_index = None;
    }
}

// tests removed as requested
