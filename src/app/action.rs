use std::collections::VecDeque;

use crate::container::node::Node;

use super::job::Job;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub enum PreviewNavigation {
    Up,
    Down,
    Left,
    Right,
}

impl PreviewNavigation {
    // TDDO: change into From impl
    pub fn to_action(self) -> Action {
        Action::Navigation(NavigationAction::PreviewNavigation(self))
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
    PreviewNavigation(PreviewNavigation),
}

impl From<NavigationAction> for Action {
    fn from(value: NavigationAction) -> Self {
        Self::Navigation(value)
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
#[cfg_attr(test, derive(PartialEq))]
pub enum WorkSpaceAction {
    Edit,
    EditError(ConfirmAction<String>),
}

impl From<WorkSpaceAction> for Action {
    fn from(value: WorkSpaceAction) -> Self {
        Self::Workspace(value)
    }
}

#[must_use]
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Action {
    Exit(ConfirmAction<()>),
    Workspace(WorkSpaceAction),
    Navigation(NavigationAction),
    Save(ConfirmAction<()>),
    Load(Node),
    RegisterJob(Job),
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
