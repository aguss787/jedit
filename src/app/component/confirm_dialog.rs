use crate::app::{Action, Actions};

use super::popup::popup_area;
use crossterm::event::{Event, KeyCode};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    text::{Line, Text},
    widgets::{Block, Clear, Padding, Paragraph, Widget, Wrap},
};

pub struct ConfirmDialog {
    message: Text<'static>,
    title: Option<Line<'static>>,
    response_fn: Box<dyn Fn(bool) -> Action>,
}

impl ConfirmDialog {
    pub fn new(message: Text<'static>, response_fn: Box<dyn Fn(bool) -> Action>) -> Self {
        Self {
            message,
            title: None,
            response_fn,
        }
    }

    pub fn title(&mut self, title: Option<Line<'static>>) {
        self.title = title;
    }

    pub fn handle_event(&self, actions: &mut Actions, event: Event) {
        let Some(event) = event.as_key_press_event() else {
            return;
        };

        match event.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                actions.push((self.response_fn)(true));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                actions.push((self.response_fn)(false));
            }
            _ => {}
        }
    }
}

impl Widget for &ConfirmDialog {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let mut block = Block::bordered()
            .padding(Padding::symmetric(1, 1))
            .title_bottom("[Y]es / [N]o")
            .title_alignment(Alignment::Center);

        if let Some(title) = self.title.clone() {
            block = block.title(title);
        }

        let paragraph = Paragraph::new(self.message.clone())
            .wrap(Wrap { trim: true })
            .block(block);

        let width = self
            .message
            .width()
            .try_into()
            .unwrap_or(u16::MAX)
            .max(20)
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

#[cfg(test)]
mod test {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use insta::assert_snapshot;
    use ratatui::text::Line;

    use super::*;
    use crate::app::{action::ConfirmAction, component::test_render::render_to_string};

    #[test]
    fn event_handler_test() {
        let dialog = ConfirmDialog::new(
            Text::default(),
            Box::new(ConfirmAction::action_confirmer(Action::Save)),
        );

        for (code, action) in [
            (
                KeyCode::Char('y'),
                Action::Save(ConfirmAction::Confirm(true)),
            ),
            (
                KeyCode::Char('Y'),
                Action::Save(ConfirmAction::Confirm(true)),
            ),
            (KeyCode::Enter, Action::Save(ConfirmAction::Confirm(true))),
            (
                KeyCode::Char('n'),
                Action::Save(ConfirmAction::Confirm(false)),
            ),
            (
                KeyCode::Char('N'),
                Action::Save(ConfirmAction::Confirm(false)),
            ),
            (KeyCode::Esc, Action::Save(ConfirmAction::Confirm(false))),
        ] {
            let mut actions = Actions::new();
            dialog.handle_event(
                &mut actions,
                Event::Key(KeyEvent {
                    code,
                    modifiers: KeyModifiers::empty(),
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                }),
            );
            assert_eq!(actions.into_vec(), vec![action])
        }
    }

    #[test]
    fn render_test() {
        for prompt in ["Are you sure?", "Save all files in workspace?"] {
            let dialog = ConfirmDialog::new(
                Text::from(vec![Line::from(prompt).centered()]),
                Box::new(ConfirmAction::action_confirmer(Action::Save)),
            );

            assert_snapshot!(render_to_string(&dialog));
        }
    }

    #[test]
    fn render_multiline_test() {
        let mut message = Text::default();
        message.push_line(Line::from("Error:"));
        message.push_line(Line::from("Broken IO pipe"));
        message.push_line(Line::from("continue?"));
        let dialog = ConfirmDialog::new(
            message,
            Box::new(ConfirmAction::action_confirmer(Action::Save)),
        );

        assert_snapshot!(render_to_string(&dialog));
    }

    #[test]
    fn render_title_test() {
        let mut message = Text::default();
        message.push_line(Line::from("Broken IO pipe"));
        message.push_line(Line::from("continue?"));
        let mut dialog = ConfirmDialog::new(
            message,
            Box::new(ConfirmAction::action_confirmer(Action::Save)),
        );
        dialog.title(Some(Line::from("Error")));

        assert_snapshot!(render_to_string(&dialog));
    }

    #[test]
    fn render_long_line_test() {
        let mut message = Text::default();
        message.push_line(Line::from("Error:"));
        message.push_line(Line::from(concat!(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, ",
            "sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
        )));
        message.push_line(Line::from(""));
        message.push_line(Line::from("continue?").centered());
        let dialog = ConfirmDialog::new(
            message,
            Box::new(ConfirmAction::action_confirmer(Action::Save)),
        );

        assert_snapshot!(render_to_string(&dialog));
    }
}
