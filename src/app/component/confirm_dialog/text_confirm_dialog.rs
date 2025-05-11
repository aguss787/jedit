use std::cell::RefCell;

use crossterm::event::{Event, KeyCode};
use ratatui::{
    prelude::{Buffer, Rect},
    text::{Line, Text},
    widgets::{Block, Clear, WidgetRef},
};

use crate::app::{
    action::{Action, Actions},
    component::popup::popup_area,
};

use super::ConfirmDialog;

pub struct TextConfirmDialog {
    // Should this content be a String, and pipe the mutation through actions?
    content: RefCell<String>,
    title: Option<Line<'static>>,
    response_fn: Box<dyn Fn(Option<String>) -> Action>,
}

impl TextConfirmDialog {
    pub fn new(response_fn: Box<dyn Fn(Option<String>) -> Action>) -> Self {
        Self {
            content: String::new().into(),
            title: None,
            response_fn,
        }
    }

    pub fn title(mut self, title: Line<'static>) -> Self {
        self.title = Some(title);
        self
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = content.into();
        self
    }
}

impl ConfirmDialog for TextConfirmDialog {
    fn handle_event(&self, actions: &mut Actions, event: Event) {
        let Some(event) = event.as_key_press_event() else {
            return;
        };

        match event.code {
            KeyCode::Enter => {
                actions.push((self.response_fn)(Some(self.content.borrow().clone())));
            }
            KeyCode::Esc => {
                actions.push((self.response_fn)(None));
            }
            KeyCode::Char(c) => {
                self.content.borrow_mut().push(c);
            }
            KeyCode::Backspace => {
                self.content.borrow_mut().pop();
            }
            _ => {}
        }
    }
}

impl WidgetRef for TextConfirmDialog {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let area = popup_area(area, 3, 54);
        let mut block = Block::bordered();
        if let Some(title) = self.title.clone() {
            block = block.title(title);
        }

        block.render_ref(area, buf);

        let mut content_area = block.inner(area);
        Clear.render_ref(content_area, buf);
        Text::from("> ").render_ref(content_area, buf);
        content_area.x += 2;
        content_area.width -= 2;

        let text_width = content_area.width - 1;
        let content = self
            .content
            .borrow()
            .chars()
            .rev()
            .take(text_width.into())
            .collect::<Vec<_>>();

        Text::from(content.iter().rev().collect::<String>()).render_ref(content_area, buf);

        let n_char = content.len() as u16;
        content_area.x += n_char;
        content_area.width -= n_char;
        Text::from("█").render_ref(content_area, buf);
    }
}

#[cfg(test)]
mod test {
    use crossterm::event::{KeyEvent, KeyModifiers};
    use insta::assert_snapshot;

    use crate::app::{
        action::{ConfirmAction, WorkSpaceAction},
        component::test_render::render_to_string,
    };

    use super::*;

    #[test]
    fn render_empty_test() {
        let dialog = TextConfirmDialog::new(Box::new(ConfirmAction::action_confirmer(
            WorkSpaceAction::Rename,
        )))
        .title(Line::from("Input"));

        assert_snapshot!(render_to_string(&dialog));
    }

    #[test]
    fn render_default_string_test() {
        let dialog = TextConfirmDialog::new(Box::new(ConfirmAction::action_confirmer(
            WorkSpaceAction::Rename,
        )))
        .title(Line::from("Input"))
        .content(String::from("default value"));

        assert_snapshot!(render_to_string(&dialog));
    }

    #[test]
    fn render_edit_test() {
        let dialog = TextConfirmDialog::new(Box::new(ConfirmAction::action_confirmer(
            WorkSpaceAction::Rename,
        )))
        .title(Line::from("Input"))
        .content(String::from("default value"));

        let mut actions = Actions::new();
        dialog.handle_event(
            &mut actions,
            Event::Key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty())),
        );
        dialog.handle_event(
            &mut actions,
            Event::Key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty())),
        );
        dialog.handle_event(
            &mut actions,
            Event::Key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::empty())),
        );

        assert_snapshot!(render_to_string(&dialog));

        dialog.handle_event(
            &mut actions,
            Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
        );

        assert_snapshot!(render_to_string(&dialog));
    }
}
