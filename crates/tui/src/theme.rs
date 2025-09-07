use ratatui::style::Color;

pub struct Theme {
    pub border_focus: Color,
    pub border_inactive: Color,
    pub chat_border: Color,
    pub sidebar_selected_fg: Color,
    pub sidebar_selected_bg: Color,
}

pub const THEME: Theme = Theme {
    border_focus: Color::Cyan,
    border_inactive: Color::DarkGray,
    chat_border: Color::DarkGray,
    sidebar_selected_fg: Color::Black,
    sidebar_selected_bg: Color::Cyan,
};
