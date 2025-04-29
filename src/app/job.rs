use std::thread::JoinHandle;

use super::action::Action;

#[derive(Debug)]
pub struct Job(JoinHandle<Result<Action, std::io::Error>>);

#[cfg(test)]
impl PartialEq for Job {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Job {
    pub fn new<F: FnOnce() -> Result<Action, std::io::Error> + Sync + Send + 'static>(
        f: F,
    ) -> Self {
        Self(std::thread::spawn(f))
    }

    pub fn is_done(&self) -> bool {
        self.0.is_finished()
    }

    pub fn action(self) -> Result<Action, std::io::Error> {
        self.0.join().map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("{err:?}"))
        })?
    }
}
