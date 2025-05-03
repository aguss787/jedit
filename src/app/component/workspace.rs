mod worktree_node;

use std::io::Write;

use crossterm::event::{Event, KeyCode, KeyModifiers};
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
        Action, Actions,
        action::{
            ConfirmAction, JobAction, NavigationAction, PreviewNavigationAction, WorkSpaceAction,
        },
        math::Op,
    },
    container::node::{Index, IndexKind, Node, NodeMeta},
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
    preview_pct: u16,
    loading: Option<Loading>,
}

impl WorkSpace {
    pub fn new(file_root: Node) -> Self {
        let work_tree_root =
            WorkTreeNode::new(String::from("root"), Some(file_root.as_index().meta));
        let list = new_list(&work_tree_root);
        Self {
            file_root,
            work_tree_root,
            edit_cntr: 0,
            list,
            dialogs: Vec::new(),
            preview: None,
            preview_pct: 65,
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

        if event.modifiers == KeyModifiers::CONTROL {
            match event.code {
                KeyCode::Char('u') => {
                    actions.push(NavigationAction::Up(10).into());
                }
                KeyCode::Char('d') => {
                    actions.push(NavigationAction::Down(10).into());
                }
                KeyCode::Char('U') => {
                    actions.push(PreviewNavigationAction::Up(5).into());
                }
                KeyCode::Char('D') => {
                    actions.push(PreviewNavigationAction::Down(5).into());
                }
                KeyCode::Left => {
                    actions.push(NavigationAction::PreviewWindowResize(Op::Add(1)).into());
                }
                KeyCode::Right => {
                    actions.push(NavigationAction::PreviewWindowResize(Op::Sub(1)).into());
                }
                _ => {}
            }
            return;
        }

        match event.code {
            KeyCode::Char('k') | KeyCode::Up => {
                actions.push(NavigationAction::Up(1).into());
            }
            KeyCode::Char('j') | KeyCode::Down => {
                actions.push(NavigationAction::Down(1).into());
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
                actions.push(PreviewNavigationAction::Down(1).into());
            }
            KeyCode::Char('K') => {
                actions.push(PreviewNavigationAction::Up(1).into());
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

    pub(crate) fn handle_action(
        &mut self,
        state: &mut WorkSpaceState,
        actions: &mut Actions,
        action: WorkSpaceAction,
    ) -> std::io::Result<()> {
        match action {
            WorkSpaceAction::Navigation(navigation_action) => {
                self.handle_navigation_action(state, navigation_action);
            }
            WorkSpaceAction::Edit => actions.push(JobAction::Edit.into()),
            WorkSpaceAction::EditError(confirm_action) => {
                if self.handle_edit_error_action(confirm_action) {
                    actions.push(JobAction::Edit.into());
                }
            }
            WorkSpaceAction::Save(confirm_action) => {
                self.dialogs.pop();
                if let Some(action) = self.handle_save_action(confirm_action)? {
                    actions.push(action);
                }
            }
            WorkSpaceAction::SaveDone => self.handle_save_done(),
            WorkSpaceAction::Load(node) => self.replace_selected(state, node),
        }

        Ok(())
    }

    fn handle_navigation_action(
        &mut self,
        state: &mut WorkSpaceState,
        navigation_action: NavigationAction,
    ) {
        let prev_index = state.list_state.selected();
        match navigation_action {
            NavigationAction::Up(n) => {
                let index = state.list_state.selected().unwrap().saturating_sub(n);
                state.list_state.select(Some(index));
            }
            NavigationAction::Down(n) => {
                let index = state
                    .list_state
                    .selected()
                    .unwrap()
                    .saturating_add(n)
                    .min(self.work_tree_root.len().saturating_sub(1));
                state.list_state.select(Some(index));
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
                PreviewNavigationAction::Up(n) => state.preview_state.scroll_up(n),
                PreviewNavigationAction::Down(n) => state.preview_state.scroll_down(n),
                PreviewNavigationAction::Left => state.preview_state.scroll_left(),
                PreviewNavigationAction::Right => state.preview_state.scroll_right(),
            },
            NavigationAction::PreviewWindowResize(delta) => {
                self.preview_pct = delta.exec(self.preview_pct).clamp(20, 80)
            }
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

    pub fn write_selected(
        &self,
        worktree_state: &WorkSpaceState,
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

    pub fn replace_selected(&mut self, worktree_state: &WorkSpaceState, new_node: Node) {
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

    fn toggle_preview(&mut self, state: &WorkSpaceState) {
        if self.preview.is_some() {
            self.preview = None;
            return;
        }

        self.set_preview_to_selected(state);
    }

    fn set_preview_to_selected(&mut self, state: &WorkSpaceState) {
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

    pub fn file_root(&self) -> &Node {
        &self.file_root
    }
}

impl WorkSpace {
    fn handle_save_action(
        &mut self,
        confirm_action: ConfirmAction<()>,
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
                    Ok(Some(JobAction::Save.into()))
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
}

#[derive(Debug)]
pub struct WorkSpaceState {
    list_state: ListState,
    preview_state: PreviewState,
}

impl Default for WorkSpaceState {
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
    type State = WorkSpaceState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if let Some(preview) = &self.preview {
            let layout = Layout::horizontal([
                Constraint::Percentage(100 - self.preview_pct),
                Constraint::Fill(self.preview_pct),
            ]);
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
    fn render_tree(&self, area: Rect, buf: &mut Buffer, state: &mut WorkSpaceState) {
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
            ((KeyCode::Up, KeyModifiers::NONE), NavigationAction::Up(1)),
            (
                (KeyCode::Char('k'), KeyModifiers::NONE),
                NavigationAction::Up(1),
            ),
            (
                (KeyCode::Char('u'), KeyModifiers::CONTROL),
                NavigationAction::Up(10),
            ),
            (
                (KeyCode::Down, KeyModifiers::NONE),
                NavigationAction::Down(1),
            ),
            (
                (KeyCode::Char('j'), KeyModifiers::NONE),
                NavigationAction::Down(1),
            ),
            (
                (KeyCode::Char('d'), KeyModifiers::CONTROL),
                NavigationAction::Down(10),
            ),
            (
                (KeyCode::Enter, KeyModifiers::NONE),
                NavigationAction::Expand,
            ),
            (
                (KeyCode::Char('l'), KeyModifiers::NONE),
                NavigationAction::Expand,
            ),
            (
                (KeyCode::Char(' '), KeyModifiers::NONE),
                NavigationAction::Expand,
            ),
            (
                (KeyCode::Char('h'), KeyModifiers::NONE),
                NavigationAction::Close,
            ),
            (
                (KeyCode::Char('p'), KeyModifiers::NONE),
                NavigationAction::TogglePreview,
            ),
            (
                (KeyCode::Char('K'), KeyModifiers::NONE),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Up(1)),
            ),
            (
                (KeyCode::Char('J'), KeyModifiers::NONE),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Down(1)),
            ),
            (
                (KeyCode::Char('U'), KeyModifiers::CONTROL),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Up(5)),
            ),
            (
                (KeyCode::Char('D'), KeyModifiers::CONTROL),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Down(5)),
            ),
            (
                (KeyCode::Char('H'), KeyModifiers::NONE),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Left),
            ),
            (
                (KeyCode::Char('L'), KeyModifiers::NONE),
                NavigationAction::PreviewNavigation(PreviewNavigationAction::Right),
            ),
            (
                (KeyCode::Left, KeyModifiers::CONTROL),
                NavigationAction::PreviewWindowResize(Op::Add(1)),
            ),
            (
                (KeyCode::Right, KeyModifiers::CONTROL),
                NavigationAction::PreviewWindowResize(Op::Sub(1)),
            ),
        ] {
            assert_key_event_to_action(&worktree, key, vec![action.into()]);
        }
    }

    #[test]
    fn event_handler_fileops_test() {
        let json = String::from("123");
        let worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        for (key, action) in [
            (
                (KeyCode::Char('q'), KeyModifiers::NONE),
                Action::Exit(ConfirmAction::Request(())),
            ),
            (
                (KeyCode::Char('e'), KeyModifiers::NONE),
                WorkSpaceAction::Edit.into(),
            ),
            (
                (KeyCode::Char('w'), KeyModifiers::NONE),
                WorkSpaceAction::Save(ConfirmAction::Request(())).into(),
            ),
        ] {
            assert_key_event_to_action(&worktree, key, vec![action]);
        }
    }

    #[test]
    fn event_handler_ignore_on_confirm_dialog() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Save(ConfirmAction::Request(())),
        );

        for key in [
            (KeyCode::Char('q'), KeyModifiers::NONE),
            (KeyCode::Char('e'), KeyModifiers::NONE),
            (KeyCode::Char('w'), KeyModifiers::NONE),
            (KeyCode::Char('k'), KeyModifiers::NONE),
            (KeyCode::Up, KeyModifiers::NONE),
        ] {
            assert_key_event_to_action(&worktree, key, Vec::new());
        }

        worktree.test_action(
            &mut state,
            WorkSpaceAction::Save(ConfirmAction::Confirm(false)),
        );
        assert_key_event_to_action(
            &worktree,
            (KeyCode::Up, KeyModifiers::NONE),
            vec![NavigationAction::Up(1).into()],
        );
    }

    #[test]
    fn handle_navigation_action() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::Down(1).into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        for _ in 0..3 {
            worktree.test_action(&mut state, NavigationAction::Up(1).into());
        }
        worktree.test_action(&mut state, NavigationAction::Close.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn write_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();
        worktree.test_action(&mut state, NavigationAction::Expand.into());

        worktree.test_action(&mut state, NavigationAction::Down(1).into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Up(1).into());

        let mut buffer = Vec::new();
        worktree.write_selected(&state, &mut buffer).unwrap();
        assert_eq!(String::from_utf8(buffer).unwrap(), "[\n  1,\n  2,\n  3\n]",)
    }

    #[test]
    fn load_selected_test() {
        let json = String::from(r#"{"key": "string", "values": [1, 2, 3]}"#);
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();
        worktree.test_action(&mut state, NavigationAction::Expand.into());

        worktree.test_action(&mut state, NavigationAction::Down(1).into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Up(1).into());

        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load("[{}, 5]".as_bytes()).unwrap()),
        );

        assert_eq!(
            worktree.file_root().to_string_pretty().unwrap(),
            "{\n  \"key\": \"string\",\n  \"values\": [\n    {},\n    5\n  ]\n}"
        );
    }

    #[test]
    fn handle_edit_error_action_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        let action = WorkSpaceAction::EditError(ConfirmAction::Request(String::from(
            "Deserialization error: expected value at line 1 column 2",
        )));
        assert!(worktree.test_action(&mut state, action.clone()).is_empty());
        assert_eq!(worktree.dialogs.len(), 1);
        assert!(
            worktree
                .test_action(
                    &mut state,
                    WorkSpaceAction::EditError(ConfirmAction::Confirm(false))
                )
                .is_empty()
        );
        assert!(worktree.dialogs.is_empty());

        assert!(worktree.test_action(&mut state, action.clone()).is_empty());
        assert_eq!(worktree.dialogs.len(), 1);
        assert_eq!(
            worktree.test_action(
                &mut state,
                WorkSpaceAction::EditError(ConfirmAction::Confirm(true))
            ),
            vec![JobAction::Edit.into()]
        );
        assert!(worktree.dialogs.is_empty());
    }

    #[test]
    fn event_handler_dialog_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(
            &mut state,
            WorkSpaceAction::EditError(ConfirmAction::Request(String::from(
                "Deserialization error: expected value at line 1 column 2",
            ))),
        );
        assert_key_event_to_action(
            &worktree,
            (KeyCode::Char('y'), KeyModifiers::NONE),
            vec![WorkSpaceAction::EditError(ConfirmAction::Confirm(true)).into()],
        );
    }

    #[test]
    fn render_edit_error_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        for response in [true, false] {
            worktree.test_action(
                &mut state,
                WorkSpaceAction::EditError(ConfirmAction::Request(String::from(
                    "Deserialization error: expected value at line 1 column 2",
                ))),
            );
            if response {
                assert_snapshot!(stateful_render_to_string(
                    &worktree,
                    &mut WorkSpaceState::default()
                ));
            }

            worktree.handle_edit_error_action(ConfirmAction::Confirm(response));
            assert_snapshot!(stateful_render_to_string(
                &worktree,
                &mut WorkSpaceState::default()
            ));
        }
    }

    #[test]
    fn render_edit_error_long_message_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, WorkSpaceAction::EditError(ConfirmAction::Request(String::from(
            concat!(
                "Deserialization error: expected value at line 1 column 2. Lorem ipsum dolor sit amet,",
                "consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna",
                "aliqua.",
            )
        ))));

        assert_snapshot!(stateful_render_to_string(
            &worktree,
            &mut WorkSpaceState::default()
        ));
    }

    #[test]
    fn exit_without_change_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));

        let mut state = WorkSpaceState::default();
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load(String::from("456").as_bytes()).unwrap()),
        );
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(!worktree.maybe_exit(ConfirmAction::Confirm(false)));

        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load(String::from("123").as_bytes()).unwrap()),
        );
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));
        assert!(worktree.maybe_exit(ConfirmAction::Confirm(true)));

        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load(String::from("123").as_bytes()).unwrap()),
        );
        worktree.handle_save_done();
        assert!(worktree.maybe_exit(ConfirmAction::Request(())));
    }

    #[test]
    fn render_exit_confirm_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        let mut state = WorkSpaceState::default();
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load(String::from("456").as_bytes()).unwrap()),
        );
        assert!(!worktree.maybe_exit(ConfirmAction::Request(())));

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state,));
    }

    #[test]
    fn render_save_dialog_test() {
        let json = String::from("123");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

        let mut state = WorkSpaceState::default();
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Save(ConfirmAction::Request(())),
        );

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
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::Expand.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::Down(1).into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
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
        let mut state = WorkSpaceState::default();

        for action in [
            NavigationAction::TogglePreview,
            NavigationAction::Up(1),
            NavigationAction::Expand,
            NavigationAction::Down(1),
            NavigationAction::Down(1),
            NavigationAction::Up(1),
        ] {
            worktree.test_action(&mut state, action.into());
        }

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_scroll_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        for action in [NavigationAction::TogglePreview, NavigationAction::Expand] {
            worktree.test_action(&mut state, action.into());
        }

        for action in [
            PreviewNavigationAction::Up(1),
            PreviewNavigationAction::Down(1),
            PreviewNavigationAction::Down(1),
            PreviewNavigationAction::Up(1),
            PreviewNavigationAction::Right,
            PreviewNavigationAction::Right,
            PreviewNavigationAction::Left,
        ] {
            worktree.test_action(&mut state, action.into());
            assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        }
    }

    #[test]
    fn render_preview_overflow_scroll_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        for action in [NavigationAction::TogglePreview, NavigationAction::Expand] {
            worktree.test_action(&mut state, action.into());
        }

        for action in [
            PreviewNavigationAction::Down(3),
            PreviewNavigationAction::Down(100),
            PreviewNavigationAction::Up(3),
            PreviewNavigationAction::Up(100),
        ] {
            worktree.test_action(&mut state, action.into());
            assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        }
    }

    #[test]
    fn render_preview_update_on_edit_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load("123".as_bytes()).unwrap()),
        );

        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_overlap_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        worktree.test_action(
            &mut state,
            WorkSpaceAction::Load(Node::load(json.as_bytes()).unwrap()),
        );
        worktree.maybe_exit(ConfirmAction::Request(()));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.maybe_exit(ConfirmAction::Confirm(false));
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn meta_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());

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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
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
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::Expand.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));

        worktree.test_action(&mut state, NavigationAction::Up(1).into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_navigation_far_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Down(2).into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        worktree.test_action(&mut state, NavigationAction::Down(10).into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(&mut state, NavigationAction::Down(100).into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(&mut state, NavigationAction::Up(100).into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    #[test]
    fn render_preview_resize_test() {
        let json = include_str!("example.json");
        let mut worktree = WorkSpace::new(Node::load(json.as_bytes()).unwrap());
        let mut state = WorkSpaceState::default();

        worktree.test_action(&mut state, NavigationAction::TogglePreview.into());
        worktree.test_action(&mut state, NavigationAction::Expand.into());
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(
            &mut state,
            NavigationAction::PreviewWindowResize(Op::Sub(1)).into(),
        );
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(
            &mut state,
            NavigationAction::PreviewWindowResize(Op::Add(3)).into(),
        );
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(
            &mut state,
            NavigationAction::PreviewWindowResize(Op::Sub(100)).into(),
        );
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
        worktree.test_action(
            &mut state,
            NavigationAction::PreviewWindowResize(Op::Add(100)).into(),
        );
        assert_snapshot!(stateful_render_to_string(&worktree, &mut state));
    }

    fn assert_key_event_to_action(
        worktree: &WorkSpace,
        (code, modifiers): (KeyCode, KeyModifiers),
        expected_actions: Vec<Action>,
    ) {
        assert_event_to_action(
            worktree,
            Event::Key(KeyEvent {
                code,
                modifiers,
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

    impl WorkSpace {
        fn test_action(
            &mut self,
            state: &mut WorkSpaceState,
            action: WorkSpaceAction,
        ) -> Vec<Action> {
            let mut actions = Actions::new();
            self.handle_action(state, &mut actions, action).unwrap();
            actions.into_vec()
        }
    }
}
