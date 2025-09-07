use std::time::{Duration, Instant};

use crossterm::event::{self, Event, MouseButton, MouseEventKind};
use ratatui::{backend::Backend, Terminal};

use crate::{app::App, ui};

pub fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> anyhow::Result<()> {
    let mut last_draw = Instant::now();
    let heartbeat = Duration::from_millis(500);
    loop {
        if app.dirty || last_draw.elapsed() >= heartbeat {
            terminal.draw(|f| ui::draw(f, app))?;
            app.dirty = false;
            last_draw = Instant::now();
        }
        if matches!(app.focus, crate::app::Focus::Input) {
            let _ = terminal.show_cursor();
        } else {
            let _ = terminal.hide_cursor();
        }

        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) => {
                    app.on_key(key);
                }
                Event::Paste(s) => {
                    app.insert_text(&s);
                    app.dirty = true;
                }
                Event::Resize(_, _) => {}
                Event::Mouse(me) => {
                    if app.show_help {
                    } else if let Some(area) = app.chat_area {
                        let x = me.column;
                        let y = me.row;
                        let inside = x >= area.x
                            && x < area.x + area.width
                            && y >= area.y
                            && y < area.y + area.height;
                        if inside {
                            match me.kind {
                                MouseEventKind::ScrollUp => {
                                    app.chat_scroll = app.chat_scroll.saturating_add(3);
                                    app.stick_to_bottom = false;
                                    app.dirty = true;
                                }
                                MouseEventKind::ScrollDown => {
                                    app.chat_scroll = app.chat_scroll.saturating_sub(3);
                                    if app.chat_scroll == 0 {
                                        app.stick_to_bottom = true;
                                    }
                                    app.dirty = true;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    let inner_w = area.width.saturating_sub(2);
                                    let inner_h = area.height.saturating_sub(2);
                                    app.ensure_chat_wrapped(inner_w);
                                    let (_viewport, _max_scroll, start_offset, _total) =
                                        app.compute_chat_layout(inner_h);
                                    let y_offset = start_offset;
                                    let rel_y = (y - (area.y + 1)) as usize;
                                    let global = y_offset.saturating_add(rel_y);

                                    let mut acc = 0usize;
                                    for (i, w) in app.chat_cache.iter().enumerate() {
                                        let base = w.lines.len();
                                        let collapsed =
                                            app.collapsed.get(i).copied().unwrap_or(false);
                                        let preview = app.collapse_preview_lines;
                                        let threshold = app.collapse_threshold_lines;
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
                                        let effective = display + if has_indicator { 1 } else { 0 };
                                        if global >= acc + effective {
                                            acc += effective;
                                            continue;
                                        }
                                        let offset_in_msg = global - acc;
                                        if has_indicator && offset_in_msg == display {
                                            app.toggle_collapse_at(i);
                                            app.dirty = true;
                                        }
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    if !app.show_sidebar {
                    } else if let Some(area) = app.sidebar_area {
                        let x = me.column;
                        let y = me.row;
                        let inside = x >= area.x
                            && x < area.x + area.width
                            && y >= area.y
                            && y < area.y + area.height;
                        if inside {
                            match me.kind {
                                MouseEventKind::ScrollUp => {
                                    let max = app.sidebar_max_scroll();
                                    app.sidebar_scroll =
                                        app.sidebar_scroll.saturating_sub(1).min(max);
                                    let _ = crate::persist::save_state(app);
                                    app.dirty = true;
                                }
                                MouseEventKind::ScrollDown => {
                                    let max = app.sidebar_max_scroll();
                                    app.sidebar_scroll = (app.sidebar_scroll + 1).min(max);
                                    let _ = crate::persist::save_state(app);
                                    app.dirty = true;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    if y > area.y && y < area.y + area.height - 1 {
                                        let start = app.sidebar_scroll as usize;
                                        let idx = start + (y - (area.y + 1)) as usize;
                                        if idx < app.sessions.len() {
                                            app.current_session = idx;
                                            app.ensure_sidebar_visible();
                                            let _ = crate::persist::save_state(app);
                                            app.load_current_session_messages();
                                            app.dirty = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        app.on_tick();

        if app.should_quit {
            let _ = crate::persist::save_state(app);
            break;
        }
    }
    Ok(())
}
