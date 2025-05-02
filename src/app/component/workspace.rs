mod worktree_node;

use std::{fs::File, io::Write};

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

use crate::{
    app::{
        Action, Actions, EDITOR_BUFFER, Terminal,
        action::{ConfirmAction, NavigationAction, PreviewNavigationAction, WorkSpaceAction},
        job::Job,
    },
    container::node::{Index, IndexKind, Node, NodeMeta},
    error::LoadError,
};

use super::{
    confirm_dialog::ConfirmDialog,
    loading::Loading,
    preview::{Preview, PreviewState},
    scrollbar::scrollbar,
};

pub struct WorkSpace {
    output_file_name: String,
    file_root: Node,
    work_tree_root: WorkTreeNode,
    edit_cntr: i64,

    list: List<'static>,
    dialogs: Vec<ConfirmDialog>,
    preview: Option<Preview>,
    loading: Option<Loading>,
}

impl WorkSpace {
    pub fn new(file_root: Node, output_file_name: String) -> Self {
        let work_tree_root =
            WorkTreeNode::new(String::from("root"), Some(file_root.as_index().meta));
        let list = new_list(&work_tree_root);
        Self {
            output_file_name,
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
                actions.push(NavigationAction::Up.into());
            }
            KeyCode::Char('j') | KeyCode::Down => {
                actions.push(NavigationAction::Down.into());
            }
            KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                actions.push(NavigationAction::Expand.into());
            }
            KeyCode::Char('h') => {
                actions.push(NavigationAction::Close.into());
            }
            KeyCode::Char('p') => {
                actions.push(NavigationAction::TogglePreview.into());
            }
            KeyCode::Char('q') => {
                actions.push(Action::Exit(ConfirmAction::Request(())));
            }
            KeyCode::Char('e') => {
                actions.push(WorkSpaceAction::Edit.into());
            }
            KeyCode::Char('w') => {
                actions.push(WorkSpaceAction::Save(ConfirmAction::Request(())).into());
            }
            KeyCode::Char('H') => {
                actions.push(PreviewNavigationAction::Left.into());
            }
            KeyCode::Char('J') => {
                actions.push(PreviewNavigationAction::Down.into());
            }
            KeyCode::Char('K') => {
                actions.push(PreviewNavigationAction::Up.into());
            }
            KeyCode::Char('L') => {
                actions.push(PreviewNavigationAction::Right.into());
            }
            _ => {}
        }
    }

    pub fn set_loading(&mut self, is_loading: bool) {
        if is_loading && self.loading.is_none() {
            self.loading = Some(Loading::default());
        } else if !is_loading {
            self.loading = None;
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

    pub fn handle_action(
        &mut self,
        state: &mut WorkTreeState,
        terminal: &mut Terminal,
        actions: &mut Actions,
        action: WorkSpaceAction,
    ) -> std::io::Result<()> {
        match action {
            WorkSpaceAction::Navigation(navigation_action) => {
                self.handle_navigation_action(state, navigation_action);
            }
            WorkSpaceAction::Edit => {
                let mut file = File::create(EDITOR_BUFFER)?;
                if !self.write_selected(state, &mut file)? {
                    return Ok(());
                };
                drop(file);
                actions.push(self.edit_in_editor(terminal)?);
            }
            WorkSpaceAction::EditError(confirm_action) => {
                if self.handle_edit_error_action(confirm_action) {
                    actions.push(self.edit_in_editor(terminal)?);
                }
            }
            WorkSpaceAction::Save(confirm_action) => {
                self.dialogs.pop();
                let output_file = File::create(&self.output_file_name)?;
                if let Some(action) =
                    self.handle_save_action(confirm_action, move || output_file)?
                {
                    actions.push(action);
                }
            }
            WorkSpaceAction::SaveDone => {
                self.handle_save_done();
            }
        }

        Ok(())
    }

    fn handle_navigation_action(
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
                PreviewNavigationAction::Up => state.preview_state.scroll_up(),
                PreviewNavigationAction::Down => state.preview_state.scroll_down(),
                PreviewNavigationAction::Left => state.preview_state.scroll_left(),
                PreviewNavigationAction::Right => state.preview_state.scroll_right(),
            },
        }

        if self.preview.is_some() && prev_index != state.list_state.selected() {
            self.set_preview_to_selected(state);
        }
    }

    fn expand(&mut self, index: usize) -> bool {
        let selector = self.work_tree_root.selector(index);
        let node_index = self
            .file_root
            .subtree(&selector)
            .expect("broken selector")
            .as_index();
        let is_terminal = matches!(node_index.kind, IndexKind::Terminal);
        self.reindex(index, node_index, true);
        !is_terminal
    }

    fn write_selected(
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
        let Some(index) = state.list_state.selected() else {
            return;
        };
        let meta = self.meta_on_index(index);

        let mut buffer = Vec::new();
        if meta.n_bytes <= 1024 * 1024 {
            let _ = self.write_on_index(&mut buffer, index);
        }
        let preview = String::from_utf8(buffer).unwrap_or_default();
        self.preview = Some(Preview::new((!preview.is_empty()).then_some(preview)))
    }

    fn meta_on_index(&mut self, index: usize) -> NodeMeta {
        if let Some(meta) = self.work_tree_root.meta(index) {
            return meta;
        }

        let selector = self.work_tree_root.selector(index);
        let node_index = self
            .file_root
            .subtree(&selector)
            .expect("broken selector")
            .as_index();
        let meta = node_index.meta;
        self.reindex(index, node_index, false);
        meta
    }
}

struct NodeJob(*const Node);
unsafe impl Send for NodeJob {}
unsafe impl Sync for NodeJob {}

impl WorkSpace {
    fn handle_save_action<F: FnOnce() -> W, W: Write + Sync + Send + 'static>(
        &mut self,
        confirm_action: ConfirmAction<()>,
        writer_getter: F,
    ) -> std::io::Result<Option<Action>> {
        match confirm_action {
            ConfirmAction::Request(()) => {
                self.dialogs.push(ConfirmDialog::new(
                    Text::from(Line::from("Write file?").centered()),
                    Box::new(ConfirmAction::action_confirmer(WorkSpaceAction::Save)),
                ));
                Ok(None)
            }
            ConfirmAction::Confirm(ok) => {
                if ok {
                    let selector = self.work_tree_root.selector(0);
                    let content: *const Node =
                        self.file_root.subtree(&selector).expect("broken selector");
                    let content = NodeJob(content);
                    let mut writer = writer_getter();
                    let action = Action::RegisterJob(Job::new(move || {
                        let _ = &content;
                        let content =
                            unsafe { content.0.as_ref().expect("invalid pointer to content") };
                        writer.write_all(
                            content
                                .to_string_pretty()
                                .expect("invalid internal representation")
                                .as_bytes(),
                        )?;
                        Ok(WorkSpaceAction::SaveDone.into())
                    }));
                    Ok(Some(action))
                } else {
                    self.dialogs.pop();
                    Ok(None)
                }
            }
        }
    }

    fn handle_save_done(&mut self) {
        self.edit_cntr = 0;
    }
}

impl WorkSpace {
    fn handle_edit_error_action(&mut self, confirm_action: ConfirmAction<String>) -> bool {
        match confirm_action {
            ConfirmAction::Request(message) => {
                let mut confirm_dialog = ConfirmDialog::new(
                    Text::from(vec![
                        Line::from(message),
                        Line::from(""),
                        Line::from("Continue to edit?").centered(),
                    ]),
                    Box::new(ConfirmAction::action_confirmer(WorkSpaceAction::EditError)),
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

    fn edit_in_editor(&mut self, terminal: &mut Terminal) -> Result<Action, std::io::Error> {
        terminal.run_editor(EDITOR_BUFFER)?;
        let action = Action::RegisterJob(Job::new(|| {
            let file = File::open(EDITOR_BUFFER)?;

            match Node::load(file) {
                Err(LoadError::IO(error)) => Err(error),
                Err(LoadError::SerdeJson(error)) => Ok(WorkSpaceAction::EditError(
                    ConfirmAction::Request(error.to_string()),
                )
                .into()),
                Err(LoadError::DeserializationError(error)) => Ok(WorkSpaceAction::EditError(
                    ConfirmAction::Request(error.to_string()),
                )
                .into()),
                Ok(node) => Ok(Action::Load(node)),
            }
        }));
        Ok(action)
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

fn new_list(work_tree_node: &WorkTreeNode) -> List<'static> {
    List::new(work_tree_node.as_tree_string())
        .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always)
        .scroll_padding(1)
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use insta::assert_snapshot;

    use crate::app::component::test_render::stateful_render_to_string;

    use super::*;

    #[test]
    fn event_handler_ignore_key_release_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

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
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

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
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Up),
            ),
            (
                KeyCode::Char('J'),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Down),
            ),
            (
                KeyCode::Char('H'),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Left),
            ),
            (
                KeyCode::Char('L'),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Right),
            ),
        ] {
            assert_key_event_to_action(&worktree, key, vec![action.into()]);
        }
    }

    #[test]
    fn event_handler_fileops_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

        for (key, action) in [
            (KeyCode::Char('q'), Action::Exit(ConfirmAction::Request(()))),
            (KeyCode::Char('e'), WorkSpaceAction::Edit.into()),
            (
                KeyCode::Char('w'),
                WorkSpaceAction::Save(ConfirmAction::Request(())).into(),
            ),
        ] {
            assert_key_event_to_action(&worktree, key, vec![action]);
        }
    }

    #[test]
    fn event_handler_ignore_on_confirm_dialog() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        worktree
            .handle_save_action(ConfirmAction::Request(()), Vec::new)
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
            .handle_save_action(ConfirmAction::Confirm(false), Vec::new)
            .unwrap();
        assert_key_event_to_action(&worktree, KeyCode::Up, vec![NavigationAction::Up.into()]);
    }

    #[test]
    fn handle_navigation_action() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::Down);
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        for _ in 0..3 {
            worktree.handle_navigation_action(&mut state, NavigationAction::Up);
        }
        worktree.handle_navigation_action(&mut state, NavigationAction::Close);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn write_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);

        worktree.handle_navigation_action(&mut state, NavigationAction::Down);
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        worktree.handle_navigation_action(&mut state, NavigationAction::Up);

        let mut buffer = Vec::new();
        worktree.write_selected(&state, &mut buffer).unwrap();
        assert_eq!(String::from_utf8(buffer).unwrap(), "[\n  1,\n  2,\n  3\n]",)
    }

    #[test]
    fn load_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);

        worktree.handle_navigation_action(&mut state, NavigationAction::Down);
        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        worktree.handle_navigation_action(&mut state, NavigationAction::Up);

        worktree.replace_selected(&state, Node::load("[{}, 5]".as_bytes()).unwrap());

        let buffer = Buffer::new();
        let Some(Action::RegisterJob(job)) = worktree
            .handle_save_action(ConfirmAction::Confirm(true), || buffer.clone())
            .unwrap()
        else {
            unreachable!()
        };
        let _ = job.action().unwrap();
        assert_eq!(
            String::from_utf8(buffer.to_vec()).unwrap(),
            "{\n  \"key\": \"string\",\n  \"values\": [\n    {},\n    5\n  ]\n}"
        );
    }

    #[test]
    fn handle_edit_error_action_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

        worktree.handle_edit_error_action(ConfirmAction::Request(String::from("error")));
        assert_key_event_to_action(
            &worktree,
            KeyCode::Char('y'),
            vec![WorkSpaceAction::EditError(ConfirmAction::Confirm(true)).into()],
        );
    }

    #[test]
    fn render_edit_error_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));

        let state = WorkTreeState::default();
        worktree.replace_selected(&state, Node::load(String::from("456").as_bytes()).unwrap());
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(!worktree.maybe_exit(ConfirmAction::Confirm(false)));

        worktree.replace_selected(&state, Node::load(String::from("123").as_bytes()).unwrap());
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(worktree.maybe_exit(ConfirmAction::Confirm(true)));

        worktree.replace_selected(&state, Node::load(String::from("123").as_bytes()).unwrap());
        worktree.handle_save_done();
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));
    }

    #[test]
    fn render_exit_confirm_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

        let mut state = WorkTreeState::default();
        worktree.replace_selected(&state, Node::load(String::from("456").as_bytes()).unwrap());
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state,));
    }

    #[test]
    fn render_save_dialog_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

        let mut state = WorkTreeState::default();
        worktree
            .handle_save_action(ConfirmAction::Request(()), Vec::new)
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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_action(&mut state, NavigationAction::TogglePreview);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::Down);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::TogglePreview);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn preview_out_of_bound_test() {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "key": "value",
            "array": [1, 2, ["cat", "dog"]]
        }))
        .unwrap();
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        for action in [
            NavigationAction::TogglePreview,
            NavigationAction::Up,
            NavigationAction::Expand,
            NavigationAction::Down,
            NavigationAction::Down,
            NavigationAction::Up,
        ] {
            worktree.handle_navigation_action(&mut state, action);
        }

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_scroll_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        for action in [NavigationAction::TogglePreview, NavigationAction::Expand] {
            worktree.handle_navigation_action(&mut state, action);
        }

        for action in [
            PreviewNavigationAction::Up,
            PreviewNavigationAction::Down,
            PreviewNavigationAction::Down,
            PreviewNavigationAction::Up,
            PreviewNavigationAction::Right,
            PreviewNavigationAction::Right,
            PreviewNavigationAction::Left,
        ] {
            worktree
                .handle_navigation_action(&mut state, NavigationAction::PreviewNavigation(action));
            assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        }
    }

    #[test]
    fn render_preview_update_on_edit_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_action(&mut state, NavigationAction::TogglePreview);
        worktree.replace_selected(&state, Node::load("123".as_bytes()).unwrap());

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_overlap_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        worktree.handle_navigation_action(&mut state, NavigationAction::TogglePreview);
        worktree.replace_selected(&state, Node::load(json.as_bytes()).unwrap());
        worktree.maybe_exit(ConfirmAction::Request(()));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.maybe_exit(ConfirmAction::Confirm(false));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn meta_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());

        assert_eq!(
            worktree.meta_on_index(0),
            NodeMeta {
                n_lines: 100,
                n_bytes: 3718,
            }
        );
    }

    #[test]
    fn render_loading_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        worktree.toggle_preview(&state);
        worktree.set_loading(true);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.set_loading(false);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_large_preview_test() {
        let json_body = include_str!("example.json");
        let json_bodies: Vec<_> = std::iter::repeat_n(json_body, 1024).collect();
        let json = String::from("[") + &json_bodies.join(",") + "]";
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap(), new_temp_file());
        let mut state = WorkTreeState::default();

        worktree.toggle_preview(&state);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::Expand);
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.handle_navigation_action(&mut state, NavigationAction::Up);
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

    fn new_temp_file() -> String {
        let key: u64 = rand::random();
        format!("/tmp/jedit-test-{key}")
    }

    #[derive(Clone)]
    struct Buffer {
        lock: Arc<Mutex<()>>,
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl Buffer {
        fn new() -> Self {
            Self {
                lock: Arc::new(Mutex::new(())),
                buffer: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn to_vec(&self) -> Vec<u8> {
            self.buffer.lock().unwrap().clone()
        }
    }

    impl Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _lock = self.lock.lock().unwrap();
            let mut buffer = self.buffer.lock().unwrap();
            buffer.extend(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
