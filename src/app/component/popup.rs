use ratatui::layout::{Constraint, Flex, Layout, Rect};

pub fn popup_area(area: Rect, h: u16, w: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(h)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(w)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
