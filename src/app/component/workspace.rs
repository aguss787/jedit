mod worktree_node;

use std::io::Write;

use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::{Modifier, Style, palette::tailwind::SLATE},
    text::{Line, Text},
    widgets::{
        Block, HighlightSpacing, List, ListState, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};
use worktree_node::WorkTreeNode;

#[cfg(test)]
use crate::error::LoadError;
#[cfg(test)]
use std::io::Read;

use crate::{
    app::{
        Action, Actions,
        action::{ConfirmAction, NavigationAction, PreviewNavigation},
    },
    container::node::{Index, Node},
};

use super::{
    confirm_dialog::ConfirmDialog,
    loading::Loading,
    preview::{Preview, PreviewState},
    scrollbar::scrollbar,
};

pub struct WorkSpace {
    file_root: Node,
    work_tree_root: WorkTreeNode,
    edit_cntr: i64,

    list: List<'static>,
    dialogs: Vec<ConfirmDialog>,
    preview: Option<Preview>,
    loading: Option<Loading>,
}

impl WorkSpace {
    pub fn new(file_root: Node) -> Self {
        let work_tree_root = WorkTreeNode::new(String::from("root"));
        let list = new_list(&work_tree_root);
        Self {
            file_root,
            work_tree_root,
            edit_cntr: 0,
            list,
            dialogs: Vec::new(),
            preview: None,
            loading: None,
        }
    }

    pub fn decrease_edit_cntr(&mut self) {
        self.edit_cntr -= 1;
    }

    pub fn handle_event(&self, actions: &mut Actions, event: Event) {
        if self.loading.is_some() {
            return;
        }

        if let Some(dialog) = self.dialogs.first() {
            dialog.handle_event(actions, event);
            return;
        }

        let Some(event) = event.as_key_press_event() else {
            return;
        };

        match event.code {
            KeyCode::Char('k') | KeyCode::Up => {
                actions.push(Action::Navigation(NavigationAction::Up));
            }
            KeyCode::Char('j') | KeyCode::Down => {
                actions.push(Action::Navigation(NavigationAction::Down));
            }
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                actions.push(Action::Navigation(NavigationAction::Expand));
            }
            KeyCode::Char('h') => {
                actions.push(Action::Navigation(NavigationAction::Close));
            }
            KeyCode::Char('p') => {
                actions.push(Action::Navigation(NavigationAction::TogglePreview));
            }
            KeyCode::Char('q') => {
                actions.push(Action::Exit(ConfirmAction::Request(())));
            }
            KeyCode::Char('e') => {
                actions.push(Action::Edit);
            }
            KeyCode::Char('w') => {
                actions.push(Action::Save(ConfirmAction::Request(())));
            }
            KeyCode::Char('H') => {
                actions.push(PreviewNavigation::Left.to_action());
            }
            KeyCode::Char('J') => {
                actions.push(PreviewNavigation::Down.to_action());
            }
            KeyCode::Char('K') => {
                actions.push(PreviewNavigation::Up.to_action());
            }
            KeyCode::Char('L') => {
                actions.push(PreviewNavigation::Right.to_action());
            }
            _ => {}
        }
    }

    pub fn handle_navigation_event(
        &mut self,
        state: &mut WorkTreeState,
        navigation_action: NavigationAction,
    ) {
        let prev_index = state.list_state.selected();
        match navigation_action {
            NavigationAction::Up => {
                if state.list_state.selected().is_some_and(|index| index > 0) {
                    state.list_state.select_previous();
                }
            }
            NavigationAction::Down => {
                if state
                    .list_state
                    .selected()
                    .is_some_and(|index| index + 1 < self.work_tree_root.len())
                {
                    state.list_state.select_next();
                }
            }
            NavigationAction::Expand => {
                if let Some(index) = state.list_state.selected() {
                    if self.expand(index) {
                        state.list_state.select_next();
                    }
                }
            }
            NavigationAction::Close => {
                if let Some(index) = state.list_state.selected() {
                    self.work_tree_root.close(index);
                    self.list = new_list(&self.work_tree_root);
                }
            }
            NavigationAction::TogglePreview => {
                self.toggle_preview(state);
            }
            NavigationAction::PreviewNavigation(preview_navigation) => match preview_navigation {
                PreviewNavigation::Up => state.preview_state.scroll_up(),
                PreviewNavigation::Down => state.preview_state.scroll_down(),
                PreviewNavigation::Left => state.preview_state.scroll_left(),
                PreviewNavigation::Right => state.preview_state.scroll_right(),
            },
        }

        if self.preview.is_some() && prev_index != state.list_state.selected() {
            self.set_preview_to_selected(state);
        }
    }

    pub fn set_loading(&mut self, is_loading: bool) {
        if is_loading && self.loading.is_none() {
            self.loading = Some(Loading::new());
        } else if !is_loading {
            self.loading = None;
        }
    }

    pub fn handle_save_action<F: FnOnce() -> W, W: Write>(
        &mut self,
        confirm_action: ConfirmAction<()>,
        writer_getter: F,
    ) -> std::io::Result<()> {
        match confirm_action {
            ConfirmAction::Request(()) => self.dialogs.push(ConfirmDialog::new(
                Text::from(Line::from("Write file?").centered()),
                Box::new(ConfirmAction::action_confirmer(Action::Save)),
            )),
            ConfirmAction::Confirm(ok) => {
                if ok {
                    self.save(writer_getter())?;
                }
                self.dialogs.pop();
            }
        }
        Ok(())
    }

    pub fn handle_edit_error_action(&mut self, confirm_action: ConfirmAction<String>) -> bool {
        match confirm_action {
            ConfirmAction::Request(message) => {
                let mut confirm_dialog = ConfirmDialog::new(
                    Text::from(vec![
                        Line::from(message),
                        Line::from(""),
                        Line::from("Continue to edit?").centered(),
                    ]),
                    Box::new(ConfirmAction::action_confirmer(Action::EditError)),
                );
                confirm_dialog.title(Some(Line::from("JSON Error").left_aligned()));
                self.dialogs.push(confirm_dialog);
                false
            }
            ConfirmAction::Confirm(ok) => {
                self.dialogs.pop();
                ok
            }
        }
    }

    pub fn maybe_exit(&mut self, confirm_action: ConfirmAction<()>) -> bool {
        match confirm_action {
            ConfirmAction::Request(()) => {
                if self.edit_cntr != 0 {
                    self.dialogs.push(ConfirmDialog::new(
                        Text::from(vec![Line::from("Discard unsaved changes?").centered()]),
                        Box::new(ConfirmAction::action_confirmer(Action::Exit)),
                    ));
                }

                self.edit_cntr == 0
            }
            ConfirmAction::Confirm(ok) => {
                self.dialogs.pop();
                ok
            }
        }
    }

    fn expand(&mut self, index: usize) -> bool {
        let selector = self.work_tree_root.selector(index);
        let node_index = self
            .file_root
            .subtree(&selector)
            .expect("broken selector")
            .as_index();
        let is_terminal = matches!(node_index, Index::Terminal);
        self.reindex(index, node_index, true);
        !is_terminal
    }

    pub fn write_selected(
        &self,
        worktree_state: &WorkTreeState,
        writer: impl Write,
    ) -> std::io::Result<bool> {
        let Some(index) = worktree_state.list_state.selected() else {
            return Ok(false);
        };
        self.write_on_index(writer, index)?;

        Ok(true)
    }

    fn save(&mut self, writer: impl Write) -> Result<(), std::io::Error> {
        let res = self.write_on_index(writer, 0);
        if res.is_ok() {
            self.edit_cntr = 0;
        }
        res
    }

    fn write_on_index(&self, mut writer: impl Write, index: usize) -> Result<(), std::io::Error> {
        let selector = self.work_tree_root.selector(index);
        let content = self
            .file_root
            .subtree(&selector)
            .expect("broken selector")
            .to_string_pretty()
            .expect("broken internal representation");
        writer.write_all(content.as_bytes())?;
        Ok(())
    }

    #[cfg(test)]
    pub fn load_selected(
        &mut self,
        worktree_state: &WorkTreeState,
        reader: impl Read,
    ) -> Result<(), LoadError> {
        let Some(index) = worktree_state.list_state.selected() else {
            return Ok(());
        };
        let selector = self.work_tree_root.selector(index);
        let new_node = Node::load(reader)?;

        let node_index = new_node.as_index();
        self.file_root
            .replace(&selector, new_node)
            .expect("broken selector");
        self.reindex(index, node_index, false);
        self.edit_cntr += 1;

        if self.preview.is_some() {
            self.set_preview_to_selected(worktree_state);
        }
        Ok(())
    }

    pub fn replace_selected(&mut self, worktree_state: &WorkTreeState, new_node: Node) {
        let Some(index) = worktree_state.list_state.selected() else {
            return;
        };
        let selector = self.work_tree_root.selector(index);

        let node_index = new_node.as_index();
        self.file_root
            .replace(&selector, new_node)
            .expect("broken selector");
        self.reindex(index, node_index, false);
        self.edit_cntr += 1;

        if self.preview.is_some() {
            self.set_preview_to_selected(worktree_state);
        }
    }

    fn reindex(&mut self, index: usize, node_index: Index, force: bool) {
        self.work_tree_root.reindex(index, node_index, force);
        self.list = new_list(&self.work_tree_root);
    }

    fn toggle_preview(&mut self, state: &WorkTreeState) {
        if self.preview.is_some() {
            self.preview = None;
            return;
        }

        self.set_preview_to_selected(state);
    }

    fn set_preview_to_selected(&mut self, state: &WorkTreeState) {
        let mut buffer = Vec::new();
        let _ = self.write_selected(state, &mut buffer);
        let preview = String::from_utf8(buffer).unwrap_or_default();
        self.preview = Some(Preview::new((!preview.is_empty()).then_some(preview)))
    }
}

#[derive(Debug)]
pub struct WorkTreeState {
    list_state: ListState,
    preview_state: PreviewState,
}

impl Default for WorkTreeState {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            preview_state: PreviewState::default(),
        }
    }
}

impl StatefulWidget for &WorkSpace {
    type State = WorkTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if let Some(preview) = &self.preview {
            let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(2)]);
            let [tree_area, preview_area] = layout.areas(area);

            self.render_tree(tree_area, buf, state);
            preview.render(preview_area, buf, &mut state.preview_state);
        } else {
            self.render_tree(area, buf, state);
        }

        for dialog in &self.dialogs {
            dialog.render(area, buf);
        }

        if let Some(loading) = &self.loading {
            loading.render(area, buf);
        }
    }
}

impl WorkSpace {
    fn render_tree(&self, area: Rect, buf: &mut Buffer, state: &mut WorkTreeState) {
        let block = Block::bordered().title("Tree");
        let inner_area = block.inner(area);

        block.render(area, buf);
        StatefulWidget::render(&self.list, inner_area, buf, &mut state.list_state);

        let scrollbar = scrollbar(ScrollbarOrientation::VerticalRight);
        StatefulWidget::render(
            scrollbar,
            inner_area,
            buf,
            &mut ScrollbarState::new(self.work_tree_root.len())
                .position(state.list_state.selected().unwrap_or_default()),
        );
    }
}

impl WorkSpace {}

fn new_list(work_tree_node: &WorkTreeNode) -> List<'static> {
    List::new(work_tree_node.as_tree_string())
        .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always)
        .scroll_padding(1)
}

#[cfg(test)]
mod test {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use insta::assert_snapshot;

    use crate::app::component::test_render::stateful_render_to_string;

    use super::*;

    #[test]
    fn event_handler_ignore_key_release_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        assert_event_to_action(
            &worktree,
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Release,
                state: KeyEventState::NONE,
            }),
            vec![],
        );
    }

    #[test]
    fn event_handler_navigation_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        for (key, action) in [
            (KeyCode::Up, NavigationAction::Up),
            (KeyCode::Char('k'), NavigationAction::Up),
            (KeyCode::Down, NavigationAction::Down),
            (KeyCode::Char('j'), NavigationAction::Down),
            (KeyCode::Enter, NavigationAction::Expand),
            (KeyCode::Char('l'), NavigationAction::Expand),
            (KeyCode::Char(' '), NavigationAction::Expand),
            (KeyCode::Char('h'), NavigationAction::Close),
            (KeyCode::Char('p'), NavigationAction::TogglePreview),
            (
                KeyCode::Char('K'),
                NavigationAction::PreviewNavigation(PreviewNavigation::Up),
            ),
            (
                KeyCode::Char('J'),
                NavigationAction::PreviewNavigation(PreviewNavigation::Down),
            ),
            (
                KeyCode::Char('H'),
                NavigationAction::PreviewNavigation(PreviewNavigation::Left),
            ),
            (
                KeyCode::Char('L'),
                NavigationAction::PreviewNavigation(PreviewNavigation::Right),
            ),
        ] {
            assert_key_event_to_action(&worktree, key, vec![Action::Navigation(action)]);
        }
    }

    #[test]
    fn event_handler_fileops_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        for (key, action) in [
            (KeyCode::Char('q'), Action::Exit(ConfirmAction::Request(()))),
            (KeyCode::Char('e'), Action::Edit),
            (KeyCode::Char('w'), Action::Save(ConfirmAction::Request(()))),
        ] {
            assert_key_event_to_action(&worktree, key, vec![action]);
        }
    }

    #[test]
    fn event_handler_ignore_on_confirm_dialog() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut buffer = Vec::new();
        worktree
            .handle_save_action(ConfirmAction::Request(()), || &mut buffer)
            .unwrap();

        for key in [
            KeyCode::Char('q'),
            KeyCode::Char('e'),
            KeyCode::Char('w'),
            KeyCode::Char('k'),
            KeyCode::Up,
        ] {
            assert_key_event_to_action(&worktree, key, Vec::new());
        }

        worktree
            .handle_save_action(ConfirmAction::Confirm(false), || &mut buffer)
            .unwrap();
        assert_key_event_to_action(
            &worktree,
            KeyCode::Up,
            vec![Action::Navigation(NavigationAction::Up)],
        );
    }

    #[test]
    fn handle_navigation_action() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_event(&mut state, NavigationAction::Down);
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        for _ in 0..3 {
            worktree.handle_navigation_event(&mut state, NavigationAction::Up);
        }
        worktree.handle_navigation_event(&mut state, NavigationAction::Close);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn write_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);

        worktree.handle_navigation_event(&mut state, NavigationAction::Down);
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        worktree.handle_navigation_event(&mut state, NavigationAction::Up);

        let mut buffer = Vec::new();
        worktree.write_selected(&state, &mut buffer).unwrap();
        assert_eq!(String::from_utf8(buffer).unwrap(), "[\n  1,\n  2,\n  3\n]",)
    }

    #[test]
    fn load_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);

        worktree.handle_navigation_event(&mut state, NavigationAction::Down);
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        worktree.handle_navigation_event(&mut state, NavigationAction::Up);

        worktree
            .load_selected(&state, "[{}, 5]".as_bytes())
            .unwrap();

        let mut buffer = Vec::new();
        worktree
            .handle_save_action(ConfirmAction::Confirm(true), || &mut buffer)
            .unwrap();
        assert_eq!(
            String::from_utf8(buffer).unwrap(),
            "{\n  \"key\": \"string\",\n  \"values\": [\n    {},\n    5\n  ]\n}"
        );
    }

    #[test]
    fn load_selected_invalid_json_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);

        worktree.handle_navigation_event(&mut state, NavigationAction::Down);
        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        worktree.handle_navigation_event(&mut state, NavigationAction::Up);

        assert!(matches!(
            worktree.load_selected(&state, "[{}, 5, asd]".as_bytes()),
            Err(LoadError::SerdeJson(_)),
        ));

        let mut buffer = Vec::new();
        worktree
            .handle_save_action(ConfirmAction::Confirm(true), || &mut buffer)
            .unwrap();
        assert_eq!(
            String::from_utf8(buffer).unwrap(),
            "{\n  \"key\": \"string\",\n  \"values\": [\n    1,\n    2,\n    3\n  ]\n}"
        );
    }

    #[test]
    fn handle_edit_error_action_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        assert!(!worktree.handle_edit_error_action(ConfirmAction::Request(String::from("error"))));
        assert_eq!(worktree.dialogs.len(), 1);
        assert!(!worktree.handle_edit_error_action(ConfirmAction::Confirm(false)));
        assert!(worktree.dialogs.is_empty());

        assert!(!worktree.handle_edit_error_action(ConfirmAction::Request(String::from("error"))));
        assert_eq!(worktree.dialogs.len(), 1);
        assert!(worktree.handle_edit_error_action(ConfirmAction::Confirm(true)));
        assert!(worktree.dialogs.is_empty());
    }

    #[test]
    fn event_handler_dialog_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        worktree.handle_edit_error_action(ConfirmAction::Request(String::from("error")));
        assert_key_event_to_action(
            &worktree,
            KeyCode::Char('y'),
            vec![Action::EditError(ConfirmAction::Confirm(true))],
        );
    }

    #[test]
    fn render_edit_error_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        for response in [true, false] {
            worktree.handle_edit_error_action(ConfirmAction::Request(String::from(
                "Deserialization error: expected value at line 1 column 2",
            )));
            if response {
                assert_snapshot!(stateful_render_to_string(
                    &worktree,
                    &mut WorkTreeState::default()
                ));
            }

            worktree.handle_edit_error_action(ConfirmAction::Confirm(response));
            assert_snapshot!(stateful_render_to_string(
                &worktree,
                &mut WorkTreeState::default()
            ));
        }
    }

    #[test]
    fn render_edit_error_long_message_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        worktree.handle_edit_error_action(ConfirmAction::Request(String::from(
            concat!(
                "Deserialization error: expected value at line 1 column 2. Lorem ipsum dolor sit amet,",
                "consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna",
                "aliqua.",
            )
        )));

        assert_snapshot!(stateful_render_to_string(
            &worktree,
            &mut WorkTreeState::default()
        ));
    }

    #[test]
    fn exit_without_change_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));

        let state = WorkTreeState::default();
        worktree
            .load_selected(&state, String::from("456").as_bytes())
            .unwrap();
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(!worktree.maybe_exit(ConfirmAction::Confirm(false)));

        worktree
            .load_selected(&state, String::from("123").as_bytes())
            .unwrap();
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(worktree.maybe_exit(ConfirmAction::Confirm(true)));

        worktree
            .load_selected(&state, String::from("123").as_bytes())
            .unwrap();
        let mut buffer = Vec::new();
        worktree.save(&mut buffer).unwrap();
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));
    }

    #[test]
    fn render_exit_confirm_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        let mut state = WorkTreeState::default();
        worktree
            .load_selected(&state, String::from("456").as_bytes())
            .unwrap();
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state,));
    }

    #[test]
    fn render_save_dialog_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        let mut state = WorkTreeState::default();
        let mut buffer = Vec::new();
        worktree
            .handle_save_action(ConfirmAction::Request(()), || &mut buffer)
            .unwrap();

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state,));
    }

    #[test]
    fn render_preview_test() {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "key": "value",
            "array": [1, 2, ["cat", "dog"]]
        }))
        .unwrap();
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_event(&mut state, NavigationAction::TogglePreview);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_event(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_event(&mut state, NavigationAction::Down);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_event(&mut state, NavigationAction::TogglePreview);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn preview_out_of_bound_test() {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "key": "value",
            "array": [1, 2, ["cat", "dog"]]
        }))
        .unwrap();
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();

        for action in [
            NavigationAction::TogglePreview,
            NavigationAction::Up,
            NavigationAction::Expand,
            NavigationAction::Down,
            NavigationAction::Down,
            NavigationAction::Up,
        ] {
            worktree.handle_navigation_event(&mut state, action);
        }

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_scroll_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();

        for action in [NavigationAction::TogglePreview, NavigationAction::Expand] {
            worktree.handle_navigation_event(&mut state, action);
        }

        for action in [
            PreviewNavigation::Up,
            PreviewNavigation::Down,
            PreviewNavigation::Down,
            PreviewNavigation::Up,
            PreviewNavigation::Right,
            PreviewNavigation::Right,
            PreviewNavigation::Left,
        ] {
            worktree
                .handle_navigation_event(&mut state, NavigationAction::PreviewNavigation(action));
            assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        }
    }

    #[test]
    fn render_preview_update_on_edit_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_event(&mut state, NavigationAction::TogglePreview);
        worktree.load_selected(&state, "123".as_bytes()).unwrap();

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_overlap_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_event(&mut state, NavigationAction::TogglePreview);
        worktree.load_selected(&state, json.as_bytes()).unwrap();
        worktree.maybe_exit(ConfirmAction::Request(()));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.maybe_exit(ConfirmAction::Confirm(false));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    fn assert_key_event_to_action(
        worktree: &WorkSpace,
        code: KeyCode,
        expected_actions: Vec<Action>,
    ) {
        assert_event_to_action(
            worktree,
            Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::empty(),
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            expected_actions,
        );
    }

    fn assert_event_to_action(worktree: &WorkSpace, event: Event, expected_actions: Vec<Action>) {
        let mut actions = Actions::new();
        worktree.handle_event(&mut actions, event);
        assert_eq!(actions.into_vec(), expected_actions)
    }
}
