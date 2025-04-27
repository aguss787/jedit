use ratatui::widgets::{Scrollbar, ScrollbarOrientation};

pub fn scrollbar(scrollbar_orientation: ScrollbarOrientation) -> Scrollbar<'static> {
    match scrollbar_orientation {
        ScrollbarOrientation::VerticalRight | ScrollbarOrientation::VerticalLeft => {
            Scrollbar::new(scrollbar_orientation)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"))
        }
        ScrollbarOrientation::HorizontalBottom | ScrollbarOrientation::HorizontalTop => {
            Scrollbar::new(scrollbar_orientation)
                .begin_symbol(Some("←"))
                .end_symbol(Some("→"))
        }
    }
}
