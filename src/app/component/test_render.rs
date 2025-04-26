use ratatui::{
    Terminal,
    backend::TestBackend,
    widgets::{StatefulWidget, Widget},
};

pub fn render_to_string<T: Widget>(widget: T) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| frame.render_widget(widget, frame.area()))
        .unwrap();
    terminal.backend().to_string()
}

pub fn stateful_render_to_string<T: StatefulWidget>(widget: T, state: &mut T::State) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| frame.render_stateful_widget(widget, frame.area(), state))
        .unwrap();
    terminal.backend().to_string()
}
