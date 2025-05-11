use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    text::Text,
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

pub fn popup_area(area: Rect, h: u16, w: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(h)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(w)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub struct BoundedPopUp<'a> {
    block: Block<'a>,
    message: Text<'a>,
    min_width: u16,
}

impl<'a> BoundedPopUp<'a> {
    pub fn new(block: Block<'a>, message: Text<'a>) -> Self {
        Self {
            block,
            message,
            min_width: 20,
        }
    }

    pub fn min_width(mut self, min_width: u16) -> Self {
        self.min_width = min_width;
        self
    }
}

impl<'a> Widget for BoundedPopUp<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let paragraph = Paragraph::new(self.message.clone())
            .wrap(Wrap { trim: true })
            .block(self.block);

        let width = self
            .message
            .width()
            .try_into()
            .unwrap_or(u16::MAX)
            .max(self.min_width)
            .min(area.width - 12);
        let height = paragraph
            .line_count(width)
            .try_into()
            .unwrap_or(area.height - 4);

        let area = popup_area(area, height, width + 4);

        Clear.render(area, buf);
        paragraph.render(area, buf);
    }
}
