pub mod boolean_confirm_dialog;
pub mod error_confirm_dialog;
pub mod text_confirm_dialog;

use crate::app::Actions;

use crossterm::event::Event;
use ratatui::widgets::WidgetRef;

pub trait ConfirmDialog: WidgetRef {
    fn handle_event(&self, actions: &mut Actions, event: Event);
}
