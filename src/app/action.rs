use std::collections::VecDeque;

use crate::container::node::Node;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub enum PreviewNavigationAction {
    Up,
    Down,
    Left,
    Right,
}

impl From<PreviewNavigationAction> for Action {
    fn from(value: PreviewNavigationAction) -> Self {
        NavigationAction::PreviewNavigation(value).into()
    }
}

impl From<PreviewNavigationAction> for WorkSpaceAction {
    fn from(value: PreviewNavigationAction) -> Self {
        NavigationAction::PreviewNavigation(value).into()
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone, Copy))]
pub enum NavigationAction {
    Up,
    Down,
    Expand,
    Close,
    TogglePreview,
    PreviewNavigation(PreviewNavigationAction),
}

impl From<NavigationAction> for Action {
    fn from(value: NavigationAction) -> Self {
        WorkSpaceAction::from(value).into()
    }
}

impl From<NavigationAction> for WorkSpaceAction {
    fn from(value: NavigationAction) -> Self {
        WorkSpaceAction::Navigation(value)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone, Copy))]
pub enum ConfirmAction<T> {
    Request(T),
    Confirm(bool),
}

impl<T> ConfirmAction<T> {
    pub fn action_confirmer<R: Into<Action>>(
        f: impl Fn(ConfirmAction<T>) -> R,
    ) -> impl Fn(bool) -> Action {
        move |b| f(ConfirmAction::Confirm(b)).into()
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub enum WorkSpaceAction {
    Navigation(NavigationAction),
    Edit,
    EditError(ConfirmAction<String>),
    Save(ConfirmAction<()>),
    SaveDone,
    Load(Node),
}

impl From<WorkSpaceAction> for Action {
    fn from(value: WorkSpaceAction) -> Self {
        Self::Workspace(value)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum JobAction {
    Edit,
    Save,
}

impl From<JobAction> for Action {
    fn from(value: JobAction) -> Self {
        Self::ExecuteJob(value)
    }
}

#[must_use]
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Action {
    Exit(ConfirmAction<()>),
    Workspace(WorkSpaceAction),
    ExecuteJob(JobAction),
}

pub struct Actions(VecDeque<Action>);

impl Actions {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn push(&mut self, action: Action) {
        self.0.push_back(action);
    }

    pub fn next(&mut self) -> Option<Action> {
        self.0.pop_front()
    }

    #[cfg(test)]
    pub fn into_vec(self) -> Vec<Action> {
        self.0.into_iter().collect()
    }
}

impl Default for Actions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn actions_in_order_test() {
        let mut actions = Actions::new();
        actions.push(Action::Workspace(WorkSpaceAction::Edit));
        actions.push(Action::Exit(ConfirmAction::Request(())));
        assert_eq!(
            actions.next(),
            Some(Action::Workspace(WorkSpaceAction::Edit))
        );
        assert_eq!(
            actions.next(),
            Some(Action::Exit(ConfirmAction::Request(())))
        );
        assert_eq!(actions.next(), None);
    }
}
