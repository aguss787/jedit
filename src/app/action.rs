use std::collections::VecDeque;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone, Copy))]
pub enum NavigationAction {
    Up,
    Down,
    Expand,
    Close,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone, Copy))]
pub enum ConfirmAction<T> {
    Request(T),
    Confirm(bool),
}

impl<T> ConfirmAction<T> {
    pub fn action_confirmer(f: impl Fn(ConfirmAction<T>) -> Action) -> impl Fn(bool) -> Action {
        move |b| f(ConfirmAction::Confirm(b))
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub enum Action {
    Exit,
    Navigation(NavigationAction),
    Edit,
    EditError(ConfirmAction<String>),
    Save(ConfirmAction<()>),
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
    pub fn to_vec(&self) -> Vec<Action> {
        self.0.iter().cloned().collect()
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
        actions.push(Action::Edit);
        actions.push(Action::Exit);
        assert_eq!(actions.next(), Some(Action::Edit));
        assert_eq!(actions.next(), Some(Action::Exit));
        assert_eq!(actions.next(), None);
    }
}
