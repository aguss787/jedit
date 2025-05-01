mod action;
mod component;
mod job;

use std::{fs::File, io::stdout, process::Command, time::Duration};

use action::{Action, Actions};
use component::workspace::{WorkSpace, WorkTreeState};
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use job::Job;
use ratatui::{DefaultTerminal, Frame};

use crate::container::node::Node;

struct GlobalState {
    exit: bool,
    output_file_name: String,
}

pub struct CliApp {
    state: GlobalState,
    worktree_state: WorkTreeState,
    worktree: WorkSpace,
    jobs: Vec<Job>,
}

impl CliApp {
    pub fn new(input_file_name: String, output_file_name: String) -> std::io::Result<Self> {
        let initial_load_job = Job::new(move || {
            let file = File::open(&input_file_name)?;
            let file_root = Node::load(file).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?;

            Ok(Action::Load(file_root))
        });

        let mut cli_app = Self {
            worktree: WorkSpace::new(Node::null()),
            worktree_state: WorkTreeState::default(),
            state: GlobalState {
                exit: false,
                output_file_name,
            },
            jobs: vec![initial_load_job],
        };
        cli_app.worktree.decrease_edit_cntr();

        cli_app.worktree.handle_navigation_event(
            &mut cli_app.worktree_state,
            action::NavigationAction::TogglePreview,
        );
        Ok(cli_app)
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let mut terminal = Terminal::new();

        while !self.state.exit {
            terminal.0.draw(|frame| self.draw(frame))?;
            self.handle_event(&mut terminal)?;
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_stateful_widget(&self.worktree, frame.area(), &mut self.worktree_state);
    }

    fn handle_event(&mut self, terminal: &mut Terminal) -> std::io::Result<()> {
        let mut actions = Actions::new();
        if event::poll(FRAME_TIME)? {
            let event = event::read()?;
            if global_exit_handler(&event) {
                self.state.exit = true;
                return Ok(());
            }

            self.worktree.handle_event(&mut actions, event);
        }

        let mut jobs = Vec::new();
        std::mem::swap(&mut jobs, &mut self.jobs);
        jobs.into_iter()
            .filter_map(|job| {
                if job.is_done() {
                    Some(job.action())
                } else {
                    self.jobs.push(job);
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .for_each(|action| actions.push(action));

        while let Some(action) = actions.next() {
            match action {
                Action::Exit(confirm_action) => {
                    self.state.exit = self.worktree.maybe_exit(confirm_action);
                    return Ok(());
                }
                Action::Workspace(workspace_action) => self.worktree.handle_action(
                    &mut self.worktree_state,
                    terminal,
                    &mut actions,
                    workspace_action,
                )?,
                Action::Navigation(navigation_action) => self
                    .worktree
                    .handle_navigation_event(&mut self.worktree_state, navigation_action),
                Action::Save(confirm_action) => {
                    let output_file = File::create(&self.state.output_file_name)?;
                    self.worktree
                        .handle_save_action(confirm_action, move || output_file)?;
                }
                Action::Load(node) => {
                    self.worktree.replace_selected(&self.worktree_state, node);
                }
                Action::RegisterJob(job) => {
                    self.jobs.push(job);
                }
            }
        }

        self.worktree.set_loading(!self.jobs.is_empty());
        Ok(())
    }
}

fn global_exit_handler(event: &Event) -> bool {
    let Some(key_event) = event.as_key_event() else {
        return false;
    };

    if !key_event.is_press() {
        return false;
    }

    key_event.code == KeyCode::F(5)
}

pub struct Terminal(DefaultTerminal);

impl Terminal {
    fn new() -> Self {
        Self(ratatui::init())
    }

    fn run_editor(&mut self, path: &str) -> std::io::Result<()> {
        let editor = std::env::var("EDITOR")
            .ok()
            .unwrap_or_else(|| String::from("vi"));
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Command::new(&editor).arg(path).status()?;
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        self.0.clear()?;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

const FRAME_TIME: Duration = Duration::from_millis(16);
const EDITOR_BUFFER: &str = "/tmp/jedit-buffer.json";
