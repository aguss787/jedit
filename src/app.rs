mod action;
mod component;
mod config;
mod job;
mod math;

use std::{
    fs::File,
    io::{Write, stdout},
    process::Command,
    time::Duration,
};

use action::{
    Action, Actions, ConfirmAction, EditJobAction, JobAction, NavigationAction, WorkSpaceAction,
};
use component::workspace::{WorkSpace, WorkSpaceState};
use config::Config;
use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use job::Job;
use ratatui::{DefaultTerminal, Frame};

use crate::{container::node::Node, error::LoadError};

struct GlobalState {
    exit: bool,
}

pub struct CliApp {
    state: GlobalState,
    worktree_state: WorkSpaceState,
    worktree: WorkSpace,
    output_file_name: String,
    jobs: Vec<Job>,
}

impl CliApp {
    pub fn new(input_file_name: String, output_file_name: String) -> std::io::Result<Self> {
        let initial_load_job = Job::new(move || {
            let file = File::open(&input_file_name)?;
            let file_root = Node::load(file).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
            })?;

            Ok(WorkSpaceAction::Load(file_root).into())
        });

        let mut cli_app = Self {
            worktree: WorkSpace::new(Node::null(), Config::load()),
            worktree_state: WorkSpaceState::default(),
            state: GlobalState { exit: false },
            output_file_name,
            jobs: vec![initial_load_job],
        };
        cli_app.worktree.decrease_edit_cntr();
        Ok(cli_app)
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        let mut terminal = Terminal::new();

        self.worktree.handle_action(
            &mut self.worktree_state,
            &mut Actions::new(),
            NavigationAction::TogglePreview.into(),
        )?;

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
                    &mut actions,
                    workspace_action,
                )?,
                Action::ExecuteJob(job) => {
                    if let Some(job) = self.execute_job(terminal, job)? {
                        self.jobs.push(job);
                    }
                }
            }
        }

        self.worktree.set_loading(!self.jobs.is_empty());
        Ok(())
    }

    fn execute_job(&self, terminal: &mut Terminal, job: JobAction) -> std::io::Result<Option<Job>> {
        let job = match job {
            JobAction::Edit(EditJobAction::Init) => {
                let Some(node) = self.worktree.selected_node(&self.worktree_state) else {
                    return Ok(None);
                };
                let node = NodeJob(node);
                Job::new(move || {
                    let mut file = File::create(EDITOR_BUFFER)?;
                    let _ = &node;
                    let node = unsafe { node.0.as_ref().expect("invalid pointer to node") };
                    let content = node
                        .to_string_pretty()
                        .expect("invalid internal representation");
                    file.write_all(content.as_bytes())?;
                    Ok(JobAction::Edit(EditJobAction::Open).into())
                })
            }
            JobAction::Edit(EditJobAction::Open) => {
                terminal.run_editor(EDITOR_BUFFER)?;
                Job::new(|| {
                    let file = File::open(EDITOR_BUFFER)?;

                    match Node::load(file) {
                        Err(LoadError::IO(error)) => Err(error),
                        Err(LoadError::SerdeJson(error)) => Ok(WorkSpaceAction::EditError(
                            ConfirmAction::Request(error.to_string()),
                        )
                        .into()),
                        Err(LoadError::DeserializationError(error)) => Ok(
                            WorkSpaceAction::EditError(ConfirmAction::Request(error.to_string()))
                                .into(),
                        ),
                        Ok(node) => Ok(WorkSpaceAction::Load(node).into()),
                    }
                })
            }
            JobAction::Save => {
                let mut output_file = File::create(&self.output_file_name)?;
                let content: *const Node = self.worktree.file_root();
                let content = NodeJob(content);
                Job::new(move || {
                    let _ = &content;
                    let content =
                        unsafe { content.0.as_ref().expect("invalid pointer to content") };
                    output_file.write_all(
                        content
                            .to_string_pretty()
                            .expect("invalid internal representation")
                            .as_bytes(),
                    )?;
                    Ok(WorkSpaceAction::SaveDone.into())
                })
            }
        };

        Ok(Some(job))
    }
}

struct NodeJob(*const Node);
unsafe impl Send for NodeJob {}
unsafe impl Sync for NodeJob {}

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
