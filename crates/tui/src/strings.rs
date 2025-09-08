// Centralized UI strings and labels. ASCII-friendly by default.

use unicode_width::UnicodeWidthStr;

// Minimal, space‑efficient role prefixes (ASCII)
// User messages: blue '|' prefix (render color applied in UI)
pub const PREFIX_USER: &str = "| ";
// Assistant messages: '>' prefix
pub const PREFIX_ASSISTANT: &str = "> ";

pub const INPUT_HINT: &str = "Type message, Enter to send / Shift+Enter for newline";

// UI block titles (keep surrounding spaces for visual padding)
pub const TITLE_SESSIONS: &str = " Sessions ";
pub const TITLE_CHAT: &str = " Chat ";
pub const TITLE_INPUT: &str = " Input ";
pub const TITLE_HELP: &str = " Help / Shortcuts ";
pub const TITLE_SEARCH: &str = " Search ";
pub const TITLE_RENAME: &str = " Rename Session ";
pub const TITLE_CONFIRM: &str = " Confirm ";
pub const TITLE_CONTEXT: &str = " Context ";

// Confirm messages
pub fn confirm_delete_session_message(name: &str) -> String {
    format!(
        "Delete session \"{}\"? Press Y to confirm, N/Esc to cancel.",
        name
    )
}

// Collapse/expand indicators for long messages
pub fn indicator_expand(remaining: usize) -> String {
    // Example: "Expand (12 more lines)"
    format!("Expand ({} more lines)", remaining)
}

pub fn indicator_collapse(total: usize) -> String {
    // Example: "Collapse (120 total lines)"
    format!("Collapse ({} total lines)", total)
}

// Status bar stick label
pub const STICK_BOTTOM: &str = "Bottom";

pub fn stick_lines(n: u16) -> String {
    // ASCII-friendly label; swap to Unicode variant if desired in future
    format!("+{} lines", n)
}

pub fn build_stick_label(scroll: u16) -> String {
    if scroll == 0 {
        STICK_BOTTOM.to_string()
    } else {
        stick_lines(scroll)
    }
}

// Build the status bar line with width-aware compaction.
// - stick: e.g., "Bottom" or "^12 lines"
// - focus: e.g., "Input" or "Sessions"
// - line_disp/col_disp: caret location (1-based display)
// - history_len: input history length
// - search_info: Some((query, current_index_1_based, total_hits))
// - max_width: available width for the status text
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn build_status_line(
    stick: &str,
    focus: &str,
    line_disp: u16,
    col_disp: u16,
    history_len: usize,
    context_len: usize,
    provider: Option<(&str, &str, &str)>,
    search_info: Option<(String, usize, usize)>,
    max_width: u16,
    usage: Option<(u32, u32)>,
    temp: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
) -> String {
    let mut segments: Vec<String> = Vec::new();
    // Put provider info first for higher visibility on narrow terminals
    if let Some((prov, model, wire)) = provider {
        segments.push(format!("[{}][{}][{}]", prov, model, wire));
    }
    segments.push(format!(
        "[{}][{}] L{} C{}",
        stick, focus, line_disp, col_disp
    ));
    segments.push(format!("Hist:{}", history_len));
    segments.push(format!("Ctx:{}", context_len));
    if let Some(t) = temp {
        segments.push(format!("T:{:.1}", t));
    }
    if let Some(p) = top_p {
        segments.push(format!("P:{:.1}", p));
    }
    if let Some(m) = max_tokens {
        segments.push(format!("Max:{}", m));
    }
    if let Some((p, c)) = usage {
        let t = p.saturating_add(c);
        segments.push(format!("Tok:{}/{}/{}", p, c, t));
    }
    if let Some((q, cur, total)) = search_info {
        segments.push(if total > 0 {
            format!("Search:{} ({}/{})", q, cur, total)
        } else {
            format!("Search:{} (0/0)", q)
        });
    }
    // Hints ordered by importance; will be appended if space allows.
    let hints: [&str; 7] = [
        "Enter: send; Shift+Enter: newline",
        "PgUp/PgDn: scroll; Shift+Pg: fast",
        "Ctrl+Arrow: fine",
        "F2: sessions",
        "History: Up/Down",
        "Ctrl+F: search; F3/Shift+F3: next/prev",
        "?: help",
    ];
    for h in hints {
        segments.push(h.to_string());
    }

    let sep = "  |  ";
    let mut out = String::new();
    let mut used = 0usize;
    for (i, seg) in segments.iter().enumerate() {
        let segw = UnicodeWidthStr::width(seg.as_str());
        let addw = segw
            + if i == 0 {
                0
            } else {
                UnicodeWidthStr::width(sep)
            };
        if used + addw > max_width as usize {
            break;
        }
        if i > 0 {
            out.push_str(sep);
            used += UnicodeWidthStr::width(sep);
        }
        out.push_str(seg);
        used += segw;
    }
    out
}

// ASCII help lines content; UI maps to styled lines.
#[allow(dead_code)]
pub fn help_lines_ascii() -> &'static [&'static str] {
    &[
        "Basic",
        "  Enter: Send    Shift+Enter: Newline    Esc/Ctrl-C: Quit",
        "Input Editing",
        "  Arrow: Move cursor    Backspace/Delete: Delete prev/next char",
        "  Home/End: Line start/end    Ctrl+A/E: Line start/end",
        "  Ctrl+Arrow: Word move    Ctrl+W: Delete prev word",
        "  Ctrl+U/K: Kill to line start/end",
        "Chat Scrolling",
        "  Mouse wheel: Scroll    PgUp/PgDn: Page    Shift+PgUp/PgDn: Fast page    Ctrl+Arrow: Fine scroll    Click indicator: Expand/collapse",
        "  Ctrl+Home/End: Top/bottom    Stick to bottom: Auto when at bottom",
        "Sessions & Others",
        "  F2: Show/hide sessions    Up/Down: Input history    Mouse click sidebar: Switch session",
        "  Sidebar focus: N new / R rename / D or Delete remove",
        "Search",
        "  Ctrl+F: Search    F3: Next match    Shift+F3: Prev match",
        "Help",
        "  ?: Open/close this panel    F1: Open/close this panel",
    ]
}
