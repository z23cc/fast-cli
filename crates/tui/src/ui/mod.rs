use ratatui::{
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use textwrap::wrap;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Role};
use crate::strings::{
    build_status_line, build_stick_label, confirm_delete_session_message, help_lines_ascii,
    indicator_collapse, indicator_expand, INPUT_HINT, PREFIX_ASSISTANT, PREFIX_USER, TITLE_CHAT,
    TITLE_CONFIRM, TITLE_CONTEXT, TITLE_HELP, TITLE_INPUT, TITLE_RENAME, TITLE_SEARCH,
    TITLE_SESSIONS,
};
use crate::theme::THEME;

pub fn draw(f: &mut Frame, app: &mut App) {
    // Layout: optional left sidebar (26), main, optional right context (28)
    let mut constraints: Vec<Constraint> = Vec::new();
    if app.show_sidebar {
        constraints.push(Constraint::Length(26));
    }
    constraints.push(Constraint::Min(10));
    if app.show_context {
        constraints.push(Constraint::Length(28));
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(f.area());
    let mut idx = 0usize;
    if app.show_sidebar {
        app.sidebar_area = Some(chunks[idx]);
        {
            let app_ref: &App = &*app;
            draw_sidebar(f, chunks[idx], app_ref);
        }
        idx += 1;
    } else {
        app.sidebar_area = None;
    }
    let main_area = chunks[idx];
    idx += 1;
    draw_main(f, main_area, app);
    if app.show_context {
        app.context_area = Some(chunks[idx]);
        draw_context(f, chunks[idx], app);
    } else {
        app.context_area = None;
    }

    if let Some(state) = &app.rename {
        draw_rename(f, f.area(), state);
    }
    if let Some(confirm) = &app.confirm {
        draw_confirm(f, f.area(), confirm, app);
    }
    if let Some(state) = &app.search_input {
        draw_search(f, f.area(), state);
    }
    if let Some(state) = &app.palette {
        draw_palette(f, f.area(), state);
    }
    if let Some(state) = &app.model_picker {
        draw_model_picker(f, f.area(), state);
    }
    if app.show_help {
        draw_help(f, f.area());
    }
}

fn draw_sidebar(f: &mut Frame, area: Rect, app: &App) {
    let focused = matches!(app.focus, crate::app::Focus::Sidebar);
    let title = Span::styled(
        TITLE_SESSIONS,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    let border_style = if focused {
        Style::default().fg(THEME.border_focus)
    } else {
        Style::default().fg(THEME.border_inactive)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner_h = area.height.saturating_sub(2) as usize;
    let start = app.sidebar_scroll as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, s) in app.sessions.iter().enumerate().skip(start).take(inner_h) {
        let prefix = if i == app.current_session { "> " } else { "  " };
        let style = if i == app.current_session {
            if focused {
                Style::default()
                    .fg(THEME.sidebar_selected_fg)
                    .bg(THEME.sidebar_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(THEME.border_focus)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{}{}", prefix, s), style)));
    }
    if start >= app.sessions.len() {
        lines.clear();
    }
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);

    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    let total = app.sessions.len();
    let viewport = inner.height as usize;
    if total > viewport {
        let mut sb_state = ScrollbarState::new(total).position(app.sidebar_scroll as usize);
        let sb = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(sb, inner, &mut sb_state);
    }
}

fn draw_main(f: &mut Frame, area: Rect, app: &mut App) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let input_total_lines = measure_total_lines(&app.input, inner_width as u16).max(1) as u16;
    let target_lines = input_total_lines.min(app.input_max_lines);
    let current = app.input_visible_lines.max(1);
    let new_visible = if current < target_lines {
        current + 1
    } else if current > target_lines {
        current - 1
    } else {
        current
    };
    app.input_visible_lines = new_visible;
    let input_height = app.input_visible_lines + 2;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(area);

    app.chat_area = Some(main_chunks[0]);

    draw_chat(f, main_chunks[0], app);
    draw_status(
        f,
        main_chunks[1],
        app,
        app.input_visible_lines,
        inner_width as u16,
    );
    draw_input(
        f,
        main_chunks[2],
        app,
        app.input_visible_lines,
        inner_width as u16,
    );
}

fn draw_context(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = matches!(app.focus, crate::app::Focus::Context);
    let border_style = if focused {
        Style::default().fg(THEME.border_focus)
    } else {
        Style::default().fg(THEME.border_inactive)
    };
    let block = Block::default()
        .title(TITLE_CONTEXT)
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner_h = area.height.saturating_sub(2) as usize;
    let start = app.context_scroll as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, s) in app
        .context_items
        .iter()
        .enumerate()
        .skip(start)
        .take(inner_h)
    {
        let prefix = if i == app.context_current { "> " } else { "  " };
        let style = if i == app.context_current {
            if focused {
                Style::default()
                    .fg(THEME.sidebar_selected_fg)
                    .bg(THEME.sidebar_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(THEME.border_focus)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{}{}", prefix, s), style)));
    }
    if start >= app.context_items.len() {
        lines.clear();
    }
    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);

    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    let total = app.context_items.len();
    let viewport = inner.height as usize;
    if total > viewport {
        let mut sb_state = ScrollbarState::new(total).position(app.context_scroll as usize);
        let sb = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(sb, inner, &mut sb_state);
    }
}

fn draw_chat(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(TITLE_CHAT)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(THEME.chat_border));

    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2);
    app.ensure_chat_wrapped(inner_width);

    let (viewport, _max_scroll, start_offset, _effective_total) =
        app.compute_chat_layout(inner_height);
    app.chat_viewport = viewport as u16;
    let mut y_offset = start_offset;

    let mut vis_lines: Vec<Line> = Vec::new();
    let mut remaining = viewport;

    let current_hit = if app.search_hits.is_empty() {
        None
    } else {
        Some(app.search_hits[app.search_current].clone())
    };
    for (idx, cached) in app.chat_cache.iter().enumerate() {
        let prefix = match cached.role {
            Role::User => PREFIX_USER,
            Role::Assistant => PREFIX_ASSISTANT,
        };
        let header_style = match cached.role {
            Role::User => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Role::Assistant => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        };
        let base = cached.lines.len();
        let collapsed = app.collapsed.get(idx).copied().unwrap_or(false);
        let preview = app.collapse_preview_lines;
        let threshold = app.collapse_threshold_lines;
        let (display_count, indicator): (usize, Option<String>) = if collapsed && base > preview {
            (preview, Some(indicator_expand(base - preview)))
        } else if !collapsed && base > threshold {
            (base, Some(indicator_collapse(base)))
        } else {
            (base, None)
        };
        let effective = display_count + indicator.as_ref().map(|_| 1).unwrap_or(0);
        if y_offset >= effective {
            y_offset -= effective;
            continue;
        }
        let start_i = y_offset.min(display_count);
        for (i, line) in cached.lines.iter().enumerate().skip(start_i) {
            if i >= display_count || remaining == 0 {
                break;
            }

            let mut spans: Vec<Span> = Vec::new();
            let (hl_start, hl_end) = if let Some(h) = &current_hit {
                if h.msg_idx == idx && h.line_idx == i {
                    (Some(h.start), Some(h.end))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let hb = if i == 0 {
                // Use display width for header prefix boundary to support Unicode widths
                UnicodeWidthStr::width(prefix).min(line.len())
            } else {
                0
            };
            let mut cuts = vec![0usize, line.len()];
            if hb > 0 {
                cuts.push(hb);
            }
            if let (Some(s), Some(e)) = (hl_start, hl_end) {
                cuts.push(s.min(line.len()));
                cuts.push(e.min(line.len()));
            }
            cuts.sort_unstable();
            cuts.dedup();
            for w in cuts.windows(2) {
                let a = w[0];
                let b = w[1];
                if a >= b {
                    continue;
                }
                let seg = &line[a..b];
                let style = if let (Some(s), Some(e)) = (hl_start, hl_end) {
                    if a < e && b > s {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else if a < hb {
                        header_style
                    } else {
                        Style::default()
                    }
                } else if a < hb {
                    header_style
                } else {
                    Style::default()
                };
                spans.push(Span::styled(seg.to_string(), style));
            }
            vis_lines.push(Line::from(spans));
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
        if remaining > 0 {
            if let Some(text) = indicator.as_ref() {
                if y_offset >= display_count
                    || (display_count <= cached.lines.len()
                        && start_i + (display_count - start_i) == display_count)
                {
                    vis_lines.push(Line::from(Span::styled(
                        text.clone(),
                        Style::default().fg(Color::DarkGray),
                    )));
                    remaining = remaining.saturating_sub(1);
                }
            }
        }
        if remaining == 0 {
            break;
        }
        y_offset = 0;
    }

    let para = Paragraph::new(vis_lines).block(block);
    f.render_widget(para, area);

    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    let effective_total = app.effective_total_lines();
    if effective_total > inner.height as usize {
        let mut sb_state = ScrollbarState::new(effective_total).position(start_offset);
        let sb = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(sb, inner, &mut sb_state);
    }
}

fn draw_input(f: &mut Frame, area: Rect, app: &App, input_visible_lines: u16, inner_width: u16) {
    let focused = matches!(app.focus, crate::app::Focus::Input);
    let border_style = if focused {
        Style::default().fg(THEME.border_focus)
    } else {
        Style::default().fg(THEME.border_inactive)
    };
    let block = Block::default()
        .title(TITLE_INPUT)
        .borders(Borders::ALL)
        .border_style(border_style);
    let graphemes: Vec<&str> = app.input.graphemes(true).collect();
    let upto = app.input_cursor.min(graphemes.len());
    let cursor_line_idx = measure_prefix_line(&graphemes, upto, inner_width) as u16;
    let offset_y = cursor_line_idx.saturating_sub(input_visible_lines.saturating_sub(1));

    let para = if app.input.is_empty() {
        let hint = Line::from(Span::styled(
            INPUT_HINT,
            Style::default().fg(Color::DarkGray),
        ));
        Paragraph::new(hint)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((0, 0))
    } else {
        Paragraph::new(app.input.clone())
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((offset_y, 0))
    };
    f.render_widget(para, area);

    let x0 = area.x + 1;
    let y0 = area.y + 1;
    let (line_idx, col_width) = measure_prefix_line_col(&graphemes, upto, inner_width);
    if focused {
        let cursor_x = x0 + col_width;
        let cursor_y = y0 + line_idx.saturating_sub(offset_y);
        f.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

fn draw_status(f: &mut Frame, area: Rect, app: &App, _input_visible_lines: u16, inner_width: u16) {
    let stick = build_stick_label(app.chat_scroll);

    let graphemes: Vec<&str> = app.input.graphemes(true).collect();
    let upto = app.input_cursor.min(graphemes.len());
    let prefix: String = graphemes[..upto].concat();
    let wrapped = wrap(&prefix, inner_width.max(1) as usize);
    let (line_idx, col_width) = if wrapped.is_empty() {
        (0u16, 0u16)
    } else {
        let last = wrapped.last().unwrap().as_ref();
        let w = UnicodeWidthStr::width(last) as u16;
        ((wrapped.len() - 1) as u16, w)
    };
    let line_disp = line_idx + 1;
    let col_disp = col_width + 1;

    let focus = match app.focus {
        crate::app::Focus::Input => "Input",
        crate::app::Focus::Sidebar => "Sessions",
        crate::app::Focus::Context => "Context",
    };
    let tips = build_status_line(
        &stick,
        focus,
        line_disp,
        col_disp,
        app.history.len(),
        app.context_items.len(),
        Some(("OpenAI", &app.model_label, &app.wire_label)),
        app.search_query
            .as_ref()
            .map(|q| (q.clone(), app.search_current + 1, app.search_hits.len())),
        area.width.saturating_sub(2),
    );
    let help = Span::styled(tips, Style::default().fg(Color::DarkGray));
    let info = Line::from(vec![help]);
    let para = Paragraph::new(info).block(Block::default().borders(Borders::ALL));
    f.render_widget(para, area);
}

use ratatui::widgets::Clear;

fn draw_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(70, 70, area);
    let block = Block::default()
        .title(Span::styled(
            TITLE_HELP,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);

    let _lines = vec![
        Line::from("Basic"),
        Line::from("  Enter: Send    Shift+Enter: Newline    Esc/Ctrl-C: Quit"),
        Line::from("Input Editing"),
        Line::from("  Left/Right: Cursor move    Backspace/Delete: Delete prev/next char"),
        Line::from("  Home/End: Line start/end    Ctrl+A/E: Line start/end"),
        Line::from("  Ctrl+Left/Right: Word move    Ctrl+W: Delete prev word"),
        Line::from("  Ctrl+U/K: Kill to line start/end"),
        Line::from("Chat Scrolling"),
        Line::from("  Mouse wheel: Scroll    PgUp/PgDn: Page    Shift+PgUp/PgDn: Fast page    Ctrl+Up/Down: Fine scroll    Click indicator: Expand/collapse"),
        Line::from("  Ctrl+Home/End: Top/bottom    Stick to bottom: Auto when at bottom"),
        Line::from("Sessions & Others"),
        Line::from("  F2: Show/hide sessions    Up/Down: Input history    Mouse click sidebar: Switch session"),
        Line::from("  Sidebar focus: N new / R rename / D or Delete remove"),
        Line::from("Search"),
        Line::from("  Ctrl+F: Search    F3: Next match"),
        Line::from("Help"),
        Line::from("  ?: Open/close this panel    F1: Open/close this panel"),
    ];

    let new_lines = help_lines_ascii()
        .iter()
        .map(|s| Line::from(*s))
        .collect::<Vec<Line>>();
    let para = Paragraph::new(new_lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
}

fn draw_palette(f: &mut Frame, area: Rect, state: &crate::app::PaletteState) {
    use unicode_width::UnicodeWidthStr;
    let popup_area = centered_rect(60, 60, area);
    let block = Block::default()
        .title(Span::styled(
            " Command Palette ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(format!(">> {}", state.buffer)));
    let max_list = popup_area.height.saturating_sub(4) as usize;
    for (i, act) in state.filtered.iter().take(max_list).enumerate() {
        let sel = i == state.selected;
        let style = if sel {
            Style::default()
                .fg(THEME.sidebar_selected_fg)
                .bg(THEME.sidebar_selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(act.label().to_string(), style)));
    }
    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
    // place cursor after prompt
    let cursor_x = popup_area.x
        + 3
        + UnicodeWidthStr::width(
            state
                .buffer
                .graphemes(true)
                .take(state.cursor)
                .collect::<String>()
                .as_str(),
        ) as u16;
    let cursor_y = popup_area.y + 1;
    f.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn draw_model_picker(f: &mut Frame, area: Rect, state: &crate::app::ModelPickerState) {
    use unicode_width::UnicodeWidthStr;
    let popup_area = centered_rect(60, 60, area);
    let block = Block::default()
        .title(Span::styled(
            " Select Model ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(format!(">> {}", state.buffer)));
    let max_list = popup_area.height.saturating_sub(4) as usize;
    for (i, m) in state.filtered.iter().take(max_list).enumerate() {
        let sel = i == state.selected;
        let style = if sel {
            Style::default()
                .fg(THEME.sidebar_selected_fg)
                .bg(THEME.sidebar_selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{} {}", if sel { ">" } else { " " }, m),
            style,
        )));
    }

    let para = Paragraph::new(lines).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
}

fn draw_search(f: &mut Frame, area: Rect, state: &crate::app::SearchInput) {
    use unicode_width::UnicodeWidthStr;
    let popup_area = centered_rect(60, 20, area);
    let block = Block::default()
        .title(Span::styled(
            TITLE_SEARCH,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);
    let lines = vec![
        Line::from("Enter keywords, Enter to confirm, Esc to cancel:"),
        Line::from(format!(">> {}", state.buffer)),
    ];
    let para = Paragraph::new(lines).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
    let cursor_x = popup_area.x
        + 3
        + UnicodeWidthStr::width(
            state
                .buffer
                .graphemes(true)
                .take(state.cursor)
                .collect::<String>()
                .as_str(),
        ) as u16;
    let cursor_y = popup_area.y + 2;
    f.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn draw_rename(f: &mut Frame, area: Rect, state: &crate::app::RenameState) {
    use unicode_width::UnicodeWidthStr;
    let popup_area = centered_rect(60, 30, area);
    let block = Block::default()
        .title(Span::styled(
            TITLE_RENAME,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);
    let lines = vec![
        Line::from("Enter new name, Enter to confirm, Esc to cancel:"),
        Line::from(format!(">> {}", state.buffer)),
    ];
    let para = Paragraph::new(lines).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
    let cursor_x = popup_area.x
        + 3
        + UnicodeWidthStr::width(
            state
                .buffer
                .graphemes(true)
                .take(state.cursor)
                .collect::<String>()
                .as_str(),
        ) as u16;
    let cursor_y = popup_area.y + 2;
    f.set_cursor_position(Position::new(cursor_x, cursor_y));
}

fn draw_confirm(f: &mut Frame, area: Rect, confirm: &crate::app::ConfirmState, app: &App) {
    let popup_area = centered_rect(60, 30, area);
    let block = Block::default()
        .title(Span::styled(
            TITLE_CONFIRM,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);
    let mut lines = Vec::new();
    match confirm.action {
        crate::app::ConfirmAction::DeleteSession(idx) => {
            let name = app.sessions.get(idx).cloned().unwrap_or_default();
            lines.push(Line::from(confirm_delete_session_message(&name)));
        }
    }
    let para = Paragraph::new(lines).block(block);
    f.render_widget(Clear, popup_area);
    f.render_widget(para, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

fn measure_total_lines(s: &str, width: u16) -> usize {
    if width == 0 {
        return 1;
    }
    let mut lines = 1usize;
    let mut col = 0usize;
    for g in s.graphemes(true) {
        if g == "\n" {
            lines += 1;
            col = 0;
            continue;
        }
        let w = UnicodeWidthStr::width(g);
        if col + w > width as usize {
            lines += 1;
            col = 0;
        }
        col += w;
    }
    lines
}

fn measure_prefix_line(graphemes: &Vec<&str>, upto: usize, width: u16) -> usize {
    if width == 0 {
        return 0;
    }
    let mut line = 0usize;
    let mut col = 0usize;
    for g in graphemes.iter().take(upto) {
        if *g == "\n" {
            line += 1;
            col = 0;
            continue;
        }
        let w = UnicodeWidthStr::width(*g);
        if col + w > width as usize {
            line += 1;
            col = 0;
        }
        col += w;
    }
    line
}

fn measure_prefix_line_col(graphemes: &Vec<&str>, upto: usize, width: u16) -> (u16, u16) {
    if width == 0 {
        return (0, 0);
    }
    let mut line = 0usize;
    let mut col = 0usize;
    for g in graphemes.iter().take(upto) {
        if *g == "\n" {
            line += 1;
            col = 0;
            continue;
        }
        let w = UnicodeWidthStr::width(*g);
        if col + w > width as usize {
            line += 1;
            col = 0;
        }
        col += w;
    }
    (line as u16, col as u16)
}

/* tests removed as requested
#[cfg(test_disabled)]
mod tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn wrap_basic_ascii() {
        let s = "abcdef";
        let g: Vec<&str> = s.graphemes(true).collect();
        assert_eq!(measure_total_lines(s, 5), 2);
        assert_eq!(measure_prefix_line(&g, 5, 5), 0);
        assert_eq!(measure_prefix_line_col(&g, 5, 5), (0, 5));
        assert_eq!(measure_prefix_line(&g, 6, 5), 1);
        assert_eq!(measure_prefix_line_col(&g, 6, 5), (1, 1));
    }

    #[test]
    fn wrap_with_newline() {
        let s = "ab\ncdef";
        let g: Vec<&str> = s.graphemes(true).collect();
        assert_eq!(measure_total_lines(s, 80), 2);
        assert_eq!(measure_prefix_line_col(&g, 2, 80), (0, 2));
        let upto = 2 + 1 + 2;
        assert_eq!(measure_prefix_line_col(&g, upto, 80), (1, 2));
    }

    #[test]
    fn wrap_fullwidth_chars() {
        let s = "ABC";
        let g: Vec<&str> = s.graphemes(true).collect();
        assert_eq!(measure_total_lines(s, 4), 2);
        assert_eq!(measure_prefix_line_col(&g, 2, 4), (0, 4));
        assert_eq!(measure_prefix_line_col(&g, 3, 4), (1, 2));
    }
}
*/
