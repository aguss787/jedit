use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Padding, Paragraph, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
    },
};

use super::scrollbar::scrollbar;

#[derive(Debug, Default)]
pub struct PreviewState {
    x_offset: u16,
    y_offset: u16,
}

enum Op {
    Add,
    Sub,
}

const SCROLL_SIZE: u16 = 5;

impl Op {
    fn exec(self, num: u16) -> u16 {
        let num = num / SCROLL_SIZE;
        let num = match self {
            Op::Add => num.saturating_add(1),
            Op::Sub => num.saturating_sub(1),
        };
        num * SCROLL_SIZE
    }
}

impl PreviewState {
    pub fn scroll_up(&mut self) {
        self.y_offset = Op::Sub.exec(self.y_offset);
    }

    pub fn scroll_down(&mut self) {
        self.y_offset = Op::Add.exec(self.y_offset);
    }

    pub fn scroll_left(&mut self) {
        self.x_offset = Op::Sub.exec(self.x_offset);
    }

    pub fn scroll_right(&mut self) {
        self.x_offset = Op::Add.exec(self.x_offset);
    }
}

pub struct Preview {
    content: Option<Content>,
}

impl Preview {
    pub fn new(content: Option<String>) -> Self {
        Self {
            content: content.map(Content::new),
        }
    }
}

impl StatefulWidget for &Preview {
    type State = PreviewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::bordered().title("Preview");
        let Some(content) = &self.content else {
            let content_area = block.inner(area);
            block.render(area, buf);
            let paragraph = Paragraph::new(Line::from("Preview not available").centered());
            let height = paragraph.line_count(content_area.width);
            let vertical =
                Layout::vertical([Constraint::Max(height.try_into().unwrap_or(u16::MAX))])
                    .flex(Flex::Center);
            let [area] = vertical.areas(content_area);
            paragraph.render(area, buf);
            return;
        };

        let scrollbar_area = block.inner(area);
        let block = block.padding(Padding::new(0, 2, 0, 2));
        let mut content_area = block.inner(area);
        block.render(area, buf);

        let line_number_area = content_area;
        let n_digits = content.n_lines.to_string().len().max(3);

        let content_area_shift: u16 = (n_digits + 1).try_into().unwrap_or_default();
        content_area.x += content_area_shift;
        content_area.width -= content_area_shift;

        let y_scroll_size = content
            .n_lines
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_sub(content_area.height);
        state.y_offset = state.y_offset.min(y_scroll_size);

        let x_scroll_size = content
            .width
            .try_into()
            .unwrap_or(u16::MAX)
            .saturating_sub(content_area.width);
        state.x_offset = state.x_offset.min(x_scroll_size);

        (0..content_area.height)
            .map(|i| state.y_offset + i + 1)
            .take_while(|i| {
                u16::try_from(content.n_lines)
                    .ok()
                    .is_none_or(|n_lines| *i <= n_lines)
            })
            .map(|i| Span::from(number_format(i, n_digits)).style(Style::new().cyan()))
            .collect::<Text<'_>>()
            .render(line_number_area, buf);

        let lines = content.text.lines().map(Line::from).collect::<Text>();

        Paragraph::new(lines)
            .scroll((state.y_offset, state.x_offset))
            .render(content_area, buf);

        if y_scroll_size > 0 {
            let mut scrollbar_area = scrollbar_area;
            scrollbar_area.height -= 1;
            let scrollbar = scrollbar(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state =
                ScrollbarState::new((y_scroll_size + 1).into()).position(state.y_offset.into());
            StatefulWidget::render(scrollbar, scrollbar_area, buf, &mut scrollbar_state);
        }

        if x_scroll_size > 0 {
            let mut scrollbar_area = scrollbar_area;
            scrollbar_area.width -= 1;
            let scrollbar = scrollbar(ScrollbarOrientation::HorizontalBottom);
            let mut scrollbar_state =
                ScrollbarState::new((x_scroll_size + 1).into()).position(state.x_offset.into());
            StatefulWidget::render(scrollbar, scrollbar_area, buf, &mut scrollbar_state);
        }
    }
}

fn number_format(index: u16, n_digits: usize) -> String {
    let num = index.to_string();
    (0..n_digits.saturating_sub(num.len()))
        .map(|_| ' ')
        .chain(num.chars())
        .collect()
}

struct Content {
    text: String,
    n_lines: usize,
    width: usize,
}

impl Content {
    fn new(text: String) -> Self {
        let n_lines = text.lines().count();
        let width = text
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default();

        Self {
            text,
            n_lines,
            width,
        }
    }
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use crate::app::component::test_render::stateful_render_to_string;

    use super::*;

    static JSON_DATA: &str = include_str!("example.json");

    #[test]
    fn render_short_test() {
        let preview = Preview::new(Some(
            (1..=16).map(|number| number.to_string() + "\n").collect(),
        ));

        assert_snapshot!(stateful_render_to_string(
            &preview,
            &mut PreviewState::default()
        ));

        let preview = Preview::new(Some(
            (1..=20).map(|number| number.to_string() + "\n").collect(),
        ));

        for y_offset in [0, 2, 4] {
            assert_snapshot!(stateful_render_to_string(
                &preview,
                &mut PreviewState {
                    x_offset: 0,
                    y_offset,
                }
            ));
        }
    }

    #[test]
    fn render_long_line_test() {
        let long_line = (0..74)
            .map(|number| (number % 10).to_string())
            .collect::<String>();
        let longer_line = (0..80)
            .map(|number| (number % 10).to_string())
            .collect::<String>();
        let preview = Preview::new(Some(
            (1..=16)
                .map(|i| {
                    (if i == 10 {
                        longer_line.clone()
                    } else {
                        long_line.clone()
                    }) + "\n"
                })
                .collect(),
        ));

        for x_offset in [0, 2, 4] {
            assert_snapshot!(stateful_render_to_string(
                &preview,
                &mut PreviewState {
                    x_offset,
                    y_offset: 0
                }
            ));
        }

        let long_line = (0..75)
            .map(|number| (number % 10).to_string())
            .collect::<String>();

        let preview = Preview::new(Some((1..=16).map(|_| long_line.clone() + "\n").collect()));
        assert_snapshot!(stateful_render_to_string(
            &preview,
            &mut PreviewState::default()
        ));
    }

    #[test]
    fn render_test() {
        let preview = Preview::new(Some(JSON_DATA.to_string()));
        let mut preview_state = PreviewState::default();

        assert_snapshot!(stateful_render_to_string(&preview, &mut preview_state));

        for i in 0..=8 {
            preview_state.scroll_down();
            preview_state.scroll_down();
            if i % 2 == 0 {
                assert_snapshot!(stateful_render_to_string(&preview, &mut preview_state));
            }
        }

        preview_state.scroll_right();
        assert_snapshot!(stateful_render_to_string(&preview, &mut preview_state));

        preview_state.scroll_up();
        assert_snapshot!(stateful_render_to_string(&preview, &mut preview_state));

        preview_state.scroll_left();
        assert_snapshot!(stateful_render_to_string(&preview, &mut preview_state));
    }

    #[test]
    fn render_empty_test() {
        let preview = Preview::new(None);
        assert_snapshot!(stateful_render_to_string(
            &preview,
            &mut PreviewState::default()
        ));
    }
}
