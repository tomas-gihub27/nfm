use std::path::{Path, PathBuf};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use std::sync::mpsc::{self, Receiver, Sender};

pub use crate::tabs::{Tab, TabState};
use crate::config::Config;
use crate::utils::Clipboard;
use crate::file_browser::file_browser::TaskType;

pub enum TaskUpdate {
    Progress(f64, String),
    Finished(String),
}

pub struct BackgroundTask {
    pub name: String,
    pub current_item: String,
    pub progress: f64, // 0.0 to 1.0
    pub start_time: Instant,
}

pub struct App {
    pub config: Config,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub should_quit: bool,
    pub clipboard: Clipboard,
    pub status_message: String,
    pub status_time: Instant,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub active_task: Option<BackgroundTask>,
    pub task_receiver: Receiver<TaskUpdate>,
    pub task_sender: Sender<TaskUpdate>,
}

impl App {
    pub fn new(config: Config, start_path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            config,
            tabs: vec![Tab::new_browser(start_path)],
            active_tab: 0,
            should_quit: false,
            clipboard: Clipboard::new(),
            status_message: "Vítejte v NeoFM".to_string(),
            status_time: Instant::now(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            active_task: None,
            task_receiver: rx,
            task_sender: tx,
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = msg;
        self.status_time = Instant::now();
    }

    pub fn run_tick(&mut self) {
        while let Ok(update) = self.task_receiver.try_recv() {
            match update {
                TaskUpdate::Progress(p, msg) => {
                    if let Some(task) = &mut self.active_task {
                        task.progress = p;
                        task.current_item = msg;
                    }
                }
                TaskUpdate::Finished(msg) => {
                    self.active_task = None;
                    self.set_status(msg);
                    if let TabState::FileBrowser(fb) = &mut self.tabs[self.active_tab].state {
                        fb.refresh();
                    }
                }
            }
        }

        if self.status_message.len() > 0 && self.status_time.elapsed() > Duration::from_secs(3) {
            self.status_message.clear();
        }
    }

    pub fn handle_events(&mut self) -> std::io::Result<()> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Already handled in file_browser if focus is there, but this is a fallback or global shortcut
                        }
                        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.close_tab();
                            return Ok(());
                        }
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let path = if let TabState::FileBrowser(fb) = &self.tabs[self.active_tab].state {
                                fb.current_dir.clone()
                            } else {
                                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
                            };
                            self.tabs.push(Tab::new_browser(path));
                            self.active_tab = self.tabs.len() - 1;
                            return Ok(());
                        }
                        KeyCode::Tab => {
                            self.next_tab();
                            return Ok(());
                        }
                        _ => {}
                    }

                    let mut new_tab_req = None;
                    match &mut self.tabs[self.active_tab].state {
                        TabState::FileBrowser(state) => {
                            if let Some(req) = state.handle_key(key, &mut self.clipboard, &self.config) {
                                new_tab_req = Some(req);
                            } else if state.should_quit {
                                self.should_quit = true;
                            }
                        }
                        TabState::Editor(state) => {
                            state.handle_key(key, &mut self.clipboard, &self.config);
                            if state.should_quit {
                                self.close_tab();
                            }
                        }
                    }

                    if let Some(req) = new_tab_req {
                        match req {
                            crate::file_browser::file_browser::TabRequest::OpenEditor(path) => {
                                self.tabs.push(Tab::new_editor(Some(path)));
                                self.active_tab = self.tabs.len() - 1;
                            }
                            crate::file_browser::file_browser::TabRequest::SetStatus(msg) => {
                                self.set_status(msg);
                            }
                            crate::file_browser::file_browser::TabRequest::StartTask { task_type, path, target } => {
                                self.start_background_task(task_type, path, target);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn start_background_task(&mut self, task_type: TaskType, path: PathBuf, target: PathBuf) {
        let sender = self.task_sender.clone();
        let name = match &task_type {
            TaskType::Copy => "Kopírování".to_string(),
            TaskType::Move => "Přesouvání".to_string(),
            TaskType::Zip => "Zabalování".to_string(),
            TaskType::Unzip => "Rozbalování".to_string(),
            TaskType::GitClone(_) => "Git Clone".to_string(),
            TaskType::Wget(_) => "Stahování".to_string(),
        };

        if let TaskType::Copy | TaskType::Move = task_type {
             if target.starts_with(&path) && path.as_os_str().len() > 0 {
                 self.set_status("Chyba: Cíl je uvnitř zdroje".to_string());
                 return;
             }
        }

        self.active_task = Some(BackgroundTask {
            name: name.clone(),
            current_item: if path.as_os_str().is_empty() { "Vzdálený zdroj".to_string() } else { path.to_string_lossy().to_string() },
            progress: 0.0,
            start_time: Instant::now(),
        });

        std::thread::spawn(move || {
            let res = match task_type {
                TaskType::Copy => copy_with_progress(&path, &target, &sender),
                TaskType::Move => {
                    if std::fs::rename(&path, &target).is_ok() {
                        Ok(())
                    } else {
                        copy_with_progress(&path, &target, &sender).and_then(|_| {
                            if path.is_dir() {
                                std::fs::remove_dir_all(&path)
                            } else {
                                std::fs::remove_file(&path)
                            }
                        })
                    }
                },
                TaskType::Zip => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, "Spouštím zip...".to_string()));
                    let parent = path.parent().unwrap_or(Path::new("."));
                    std::process::Command::new("zip")
                        .arg("-r")
                        .arg(&target)
                        .arg(path.file_name().unwrap())
                        .current_dir(parent)
                        .status()
                        .map(|_| ())
                },
                TaskType::Unzip => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, "Spouštím unzip...".to_string()));
                    std::process::Command::new("unzip")
                        .arg(&path)
                        .arg("-d")
                        .arg(&target)
                        .status()
                        .map(|_| ())
                },
                TaskType::GitClone(url) => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, format!("Klonuji {}...", url)));
                    std::process::Command::new("git")
                        .arg("clone")
                        .arg(&url)
                        .current_dir(&target)
                        .status()
                        .map(|_| ())
                },
                TaskType::Wget(url) => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, format!("Stahuji {}...", url)));
                    std::process::Command::new("wget")
                        .arg(&url)
                        .current_dir(&target)
                        .status()
                        .map(|_| ())
                }
            };

            match res {
                Ok(_) => {
                    let _ = sender.send(TaskUpdate::Finished(format!("{} dokončeno", name)));
                }
                Err(e) => {
                    let _ = sender.send(TaskUpdate::Finished(format!("Chyba: {}", e)));
                }
            }
        });
    }

    fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    fn close_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.tabs.remove(self.active_tab);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        } else {
            self.should_quit = true;
        }
    }
}

fn copy_with_progress(src: &PathBuf, dst: &PathBuf, sender: &Sender<TaskUpdate>) -> std::io::Result<()> {
    if src.is_dir() {
        let mut total_files = 0;
        count_files(src, &mut total_files);
        let mut copied_files = 0;
        copy_dir_with_progress(src, dst, sender, total_files, &mut copied_files)
    } else {
        std::fs::copy(src, dst)?;
        let _ = sender.send(TaskUpdate::Progress(1.0, src.to_string_lossy().to_string()));
        Ok(())
    }
}

fn count_files(path: &PathBuf, count: &mut usize) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            *count += 1;
            if entry.path().is_dir() {
                count_files(&entry.path(), count);
            }
        }
    }
}

fn copy_dir_with_progress(src: &PathBuf, dst: &PathBuf, sender: &Sender<TaskUpdate>, total: usize, copied: &mut usize) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let path = entry.path();
            let target = dst.join(entry.file_name());
            if path.is_dir() {
                copy_dir_with_progress(&path, &target, sender, total, copied)?;
            } else {
                std::fs::copy(&path, &target)?;
            }
            *copied += 1;
            let progress = if total > 0 { *copied as f64 / total as f64 } else { 1.0 };
            let _ = sender.send(TaskUpdate::Progress(progress, path.to_string_lossy().to_string()));
        }
    }
    Ok(())
}
