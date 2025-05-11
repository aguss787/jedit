use crossterm::event::Event;
use ratatui::{
    layout::Alignment,
    prelude::{Buffer, Rect},
    text::{Line, Text},
    widgets::{Block, Padding, Widget, WidgetRef},
};

use crate::app::{
    action::{Actions, WorkSpaceAction},
    component::popup::BoundedPopUp,
};

use super::ConfirmDialog;

pub struct ErrorConfirmDialog {
    message: Text<'static>,
    title: Option<Line<'static>>,
}

impl ErrorConfirmDialog {
    pub(crate) fn new(message: Text<'static>) -> Self {
        Self {
            message,
            title: None,
        }
    }

    pub(crate) fn title(mut self, title: Line<'static>) -> Self {
        self.title = Some(title);
        self
    }
}

impl ConfirmDialog for ErrorConfirmDialog {
    fn handle_event(&self, actions: &mut Actions, event: Event) {
        if !event.is_key_press() {
            return;
        };

        actions.push(WorkSpaceAction::ErrorConfirmed.into());
    }
}

impl WidgetRef for ErrorConfirmDialog {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let title = self.title.clone().unwrap_or_else(|| "Error!".into());
        let title_width = title.width() as u16;
        let block = Block::bordered()
            .padding(Padding::symmetric(1, 1))
            .title_top(title)
            .title_bottom(Line::from("Press any key"))
            .title_alignment(Alignment::Center);

        BoundedPopUp::new(block, self.message.clone())
            .min_width(title_width.max(20))
            .render(area, buf);
    }
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use crate::app::component::test_render::render_to_string;

    use super::*;

    #[test]
    fn render_default_test() {
        assert_snapshot!(render_to_string(&ErrorConfirmDialog::new(
            "short error!".into()
        )));
    }

    #[test]
    fn render_title_test() {
        assert_snapshot!(render_to_string(
            &ErrorConfirmDialog::new("short error!".into()).title(Line::from("Short title"))
        ));
    }

    #[test]
    fn render_long_title_test() {
        assert_snapshot!(render_to_string(
            &ErrorConfirmDialog::new("short error!".into())
                .title(Line::from("This is a very long title"))
        ));
    }
}
