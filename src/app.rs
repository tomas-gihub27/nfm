use std::path::{Path, PathBuf};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::fs;
use std::process::Stdio;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use sha2::{Sha256, Digest};
use md5;
use rand::RngCore;

pub use crate::tabs::{Tab, TabState};
use crate::config::Config;
use crate::utils::Clipboard;
use crate::file_browser::file_browser::{TaskType, ArchiveType, EncType};

pub enum TaskUpdate {
    Progress(f64, String),
    Finished(String),
}

pub struct BackgroundTask {
    pub name: String,
    pub current_item: String,
    pub progress: f64, 
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
    pub error_popup: Option<String>,
    pub task_receiver: Receiver<TaskUpdate>,
    pub task_sender: Sender<TaskUpdate>,
    pub anim_frame: usize,
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
            status_message: "Welcome to NeoFM (H for help)".to_string(),
            status_time: Instant::now(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            active_task: None,
            error_popup: None,
            task_receiver: rx,
            task_sender: tx,
            anim_frame: 0,
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = msg;
        self.status_time = Instant::now();
    }

    pub fn run_tick(&mut self) {
        self.anim_frame = self.anim_frame.wrapping_add(1);
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
                    if msg.starts_with("Error:") {
                        self.error_popup = Some(msg);
                    } else {
                        self.set_status(msg);
                    }
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
                    if self.error_popup.is_some() {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char(' ') => { self.error_popup = None; }
                            _ => {}
                        }
                        return Ok(());
                    }

                    match key.code {
                        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => { self.close_tab(); return Ok(()); }
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let path = if let TabState::FileBrowser(fb) = &self.tabs[self.active_tab].state { fb.current_dir.clone() } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")) };
                            self.tabs.push(Tab::new_browser(path)); self.active_tab = self.tabs.len() - 1; return Ok(());
                        }
                        KeyCode::Tab => { self.next_tab(); return Ok(()); }
                        _ => {}
                    }

                    let mut new_tab_req = None;
                    match &mut self.tabs[self.active_tab].state {
                        TabState::FileBrowser(state) => { if let Some(req) = state.handle_key(key, &mut self.clipboard, &self.config) { new_tab_req = Some(req); } else if state.should_quit { self.should_quit = true; } }
                        TabState::Editor(state) => { state.handle_key(key, &mut self.clipboard, &self.config); if state.should_quit { self.close_tab(); } }
                    }

                    if let Some(req) = new_tab_req {
                        match req {
                            crate::file_browser::file_browser::TabRequest::OpenEditor(path) => { self.tabs.push(Tab::new_editor(Some(path))); self.active_tab = self.tabs.len() - 1; }
                            crate::file_browser::file_browser::TabRequest::SetStatus(msg) => { self.set_status(msg); }
                            crate::file_browser::file_browser::TabRequest::StartTask { task_type, path, target } => { self.start_background_task(task_type, path, target); }
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
            TaskType::Copy => "Copying".to_string(),
            TaskType::Move => "Moving".to_string(),
            TaskType::Archive(ArchiveType::Zip) => "Compressing (Zip)".to_string(),
            TaskType::Archive(ArchiveType::Tar) => "Compressing (Tar)".to_string(),
            TaskType::Archive(ArchiveType::Gzip) => "Compressing (Gzip)".to_string(),
            TaskType::Unzip => "Extracting".to_string(),
            TaskType::Wget(_) => "Downloading".to_string(),
            TaskType::Encrypt { .. } => "Encrypting".to_string(),
            TaskType::Decrypt { .. } => "Decrypting".to_string(),
            TaskType::Search(_) => "Searching".to_string(),
            TaskType::Checksum(_) => "Calculating hashes".to_string(),
            TaskType::Delete(_) => "Deleting".to_string(),
            TaskType::Git(gt) => match gt {
                crate::file_browser::file_browser::GitTask::Status => "Git Status".to_string(),
                crate::file_browser::file_browser::GitTask::AddAll => "Git Add All".to_string(),
                crate::file_browser::file_browser::GitTask::Commit(_) => "Git Commit".to_string(),
                crate::file_browser::file_browser::GitTask::Push { .. } => "Git Push".to_string(),
                crate::file_browser::file_browser::GitTask::Pull { .. } => "Git Pull".to_string(),
                crate::file_browser::file_browser::GitTask::Fetch => "Git Fetch".to_string(),
                crate::file_browser::file_browser::GitTask::Init => "Git Init".to_string(),
                crate::file_browser::file_browser::GitTask::RemoteAdd { .. } => "Git Remote Add".to_string(),
            },
            _ => "Task".to_string(),
        };
        if let TaskType::Copy | TaskType::Move = task_type {
             if target.starts_with(&path) && path.as_os_str().len() > 0 { self.set_status("Error: Target is inside source".to_string()); return; }
        }

        self.active_task = Some(BackgroundTask {
            name: name.clone(),
            current_item: if path.as_os_str().is_empty() { "...".to_string() } else { path.to_string_lossy().to_string() },
            progress: 0.0,
            start_time: Instant::now(),
        });

        std::thread::spawn(move || {
            let res = match task_type {
                TaskType::Copy => copy_with_progress(&path, &target, &sender),
                TaskType::Move => { if fs::rename(&path, &target).is_ok() { Ok(()) } else { copy_with_progress(&path, &target, &sender).and_then(|_| { if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) } }) } },
                TaskType::Archive(atype) => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, "Compressing...".to_string()));
                    let parent = path.parent().unwrap_or(Path::new("."));
                    let cmd = match atype { 
                        ArchiveType::Zip => std::process::Command::new("zip").arg("-r").arg(&target).arg(path.file_name().unwrap()).current_dir(parent).stdout(Stdio::null()).stderr(Stdio::null()).status(),
                        ArchiveType::Tar => std::process::Command::new("tar").arg("-cvf").arg(&target).arg(path.file_name().unwrap()).current_dir(parent).stdout(Stdio::null()).stderr(Stdio::null()).status(),
                        ArchiveType::Gzip => std::process::Command::new("tar").arg("-czvf").arg(&target).arg(path.file_name().unwrap()).current_dir(parent).stdout(Stdio::null()).stderr(Stdio::null()).status(),
                    };
                    cmd.map(|_| ())
                },
                TaskType::Unzip => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, "Extracting...".to_string()));
                    if path.to_string_lossy().ends_with(".zip") { std::process::Command::new("unzip").arg(&path).arg("-d").arg(&target).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|_| ()) }
                    else { std::process::Command::new("tar").arg("-xvf").arg(&path).arg("-C").arg(&target).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|_| ()) }
                },
                TaskType::GitClone(url) => { let _ = sender.send(TaskUpdate::Progress(0.5, "Cloning...".to_string())); std::process::Command::new("git").arg("clone").arg(&url).current_dir(&target).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|_| ()) },
                TaskType::Wget(url) => { let _ = sender.send(TaskUpdate::Progress(0.5, "Downloading...".to_string())); std::process::Command::new("wget").arg(&url).current_dir(&target).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|_| ()) },
                TaskType::Encrypt { etype, key, output } => match etype { EncType::Xor => xor_file(&path, &output, &key), EncType::AesPlaceholder => aes_encrypt(&path, &output, &key) },
                TaskType::Decrypt { etype, key, output } => match etype { EncType::Xor => xor_file(&path, &output, &key), EncType::AesPlaceholder => aes_decrypt(&path, &output, &key) },
                TaskType::Search(pattern) => { let mut results = Vec::new(); search_recursive(&path, &pattern, &mut results); let _ = sender.send(TaskUpdate::Finished(if results.is_empty() { "Nothing found".to_string() } else { format!("Found {} items", results.len()) })); return; }
                TaskType::Checksum(algo) => {
                    let _ = sender.send(TaskUpdate::Progress(0.5, format!("Calculating {}...", algo)));
                    match fs::read(&path) {
                        Ok(data) => {
                            let hash = if algo == "MD5" { format!("{:x}", md5::Md5::digest(&data)) } else { format!("{:x}", sha2::Sha256::digest(&data)) };
                            let _ = sender.send(TaskUpdate::Finished(format!("{} hash: {}", algo, hash))); return;
                        }
                        Err(e) => Err(e)
                    }
                }
                TaskType::Delete(paths) => { let total = paths.len(); for (i, p) in paths.iter().enumerate() { let _ = sender.send(TaskUpdate::Progress(i as f64 / total as f64, p.to_string_lossy().to_string())); if p.is_dir() { let _ = fs::remove_dir_all(p); } else { let _ = fs::remove_file(p); } } Ok(()) }
                TaskType::Git(gt) => {
                    match gt {
                        crate::file_browser::file_browser::GitTask::Status => run_git_command(&path, &["status"], &sender),
                        crate::file_browser::file_browser::GitTask::AddAll => run_git_command(&path, &["add", "."], &sender),
                        crate::file_browser::file_browser::GitTask::Commit(msg) => run_git_command(&path, &["commit", "-m", &msg], &sender),
                        crate::file_browser::file_browser::GitTask::Push { remote, branch } => run_git_command(&path, &["push", &remote, &branch], &sender),
                        crate::file_browser::file_browser::GitTask::Pull { remote, branch } => run_git_command(&path, &["pull", &remote, &branch], &sender),
                        crate::file_browser::file_browser::GitTask::Fetch => run_git_command(&path, &["fetch"], &sender),
                        crate::file_browser::file_browser::GitTask::Init => run_git_command(&path, &["init"], &sender),
                        crate::file_browser::file_browser::GitTask::RemoteAdd { name, url } => run_git_command(&path, &["remote", "add", &name, &url], &sender),
                    }
                }
            };

            match res { Ok(_) => { let _ = sender.send(TaskUpdate::Finished(format!("{} finished", name))); } Err(e) => { let _ = sender.send(TaskUpdate::Finished(format!("Error: {}", e))); } }
        });
    }

    fn next_tab(&mut self) { if !self.tabs.is_empty() { self.active_tab = (self.active_tab + 1) % self.tabs.len(); } }
    fn close_tab(&mut self) { if self.tabs.len() > 1 { self.tabs.remove(self.active_tab); if self.active_tab >= self.tabs.len() { self.active_tab = self.tabs.len() - 1; } } else { self.should_quit = true; } }
}

fn run_git_command(path: &PathBuf, args: &[&str], sender: &Sender<TaskUpdate>) -> std::io::Result<()> {
    let _ = sender.send(TaskUpdate::Progress(0.3, format!("git {}", args.join(" "))));
    let output = std::process::Command::new("git").args(args).current_dir(path).output()?;
    let _ = sender.send(TaskUpdate::Progress(1.0, "Finished".to_string()));
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.len() > 100 {
            let _ = sender.send(TaskUpdate::Finished(format!("Success: {}", &stdout[..100])));
        } else {
            let _ = sender.send(TaskUpdate::Finished(format!("Success: {}", stdout)));
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(std::io::Error::new(std::io::ErrorKind::Other, stderr.to_string()))
    }
}

fn copy_with_progress(src: &PathBuf, dst: &PathBuf, sender: &Sender<TaskUpdate>) -> std::io::Result<()> {
    if src.is_dir() { let mut total = 0; count_files(src, &mut total); let mut current = 0; copy_dir_with_progress(src, dst, sender, total, &mut current) }
    else { fs::copy(src, dst)?; let _ = sender.send(TaskUpdate::Progress(1.0, src.to_string_lossy().to_string())); Ok(()) }
}

fn count_files(path: &PathBuf, count: &mut usize) { if let Ok(entries) = fs::read_dir(path) { for entry in entries.flatten() { *count += 1; if entry.path().is_dir() { count_files(&entry.path(), count); } } } }

fn copy_dir_with_progress(src: &PathBuf, dst: &PathBuf, sender: &Sender<TaskUpdate>, total: usize, current: &mut usize) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let p = entry.path(); let t = dst.join(entry.file_name());
            if p.is_dir() { copy_dir_with_progress(&p, &t, sender, total, current)?; } else { fs::copy(&p, &t)?; }
            *current += 1; let _ = sender.send(TaskUpdate::Progress(if total > 0 { *current as f64 / total as f64 } else { 1.0 }, p.to_string_lossy().to_string()));
        }
    }
    Ok(())
}

fn xor_file(src: &PathBuf, dst: &PathBuf, key: &str) -> std::io::Result<()> {
    let data = fs::read(src)?; let kb = key.as_bytes();
    let xored: Vec<u8> = data.iter().enumerate().map(|(i, b)| b ^ kb[i % kb.len()]).collect();
    fs::write(dst, xored)
}

fn aes_encrypt(src: &PathBuf, dst: &PathBuf, key: &str) -> std::io::Result<()> {
    let data = fs::read(src)?;
    let mut hasher = Sha256::new(); hasher.update(key.as_bytes()); let hashed_key = hasher.finalize();
    let cipher = Aes256Gcm::new(&hashed_key);
    let mut nonce_bytes = [0u8; 12]; rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce_bytes), data.as_slice()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("AES encrypt error: {}", e)))?;
    let mut output = Vec::with_capacity(12 + ciphertext.len()); output.extend_from_slice(&nonce_bytes); output.extend_from_slice(&ciphertext);
    fs::write(dst, output)
}

fn aes_decrypt(src: &PathBuf, dst: &PathBuf, key: &str) -> std::io::Result<()> {
    let data = fs::read(src)?; if data.len() < 12 { return Err(std::io::Error::new(std::io::ErrorKind::Other, "Invalid encrypted file")); }
    let mut hasher = Sha256::new(); hasher.update(key.as_bytes()); let hashed_key = hasher.finalize();
    let cipher = Aes256Gcm::new(&hashed_key);
    let plaintext = cipher.decrypt(Nonce::from_slice(&data[..12]), &data[12..]).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("AES decrypt error (wrong key?): {}", e)))?;
    fs::write(dst, plaintext)
}

fn search_recursive(path: &PathBuf, pattern: &str, results: &mut Vec<PathBuf>) { if let Ok(entries) = fs::read_dir(path) { for entry in entries.flatten() { let p = entry.path(); if p.file_name().unwrap_or_default().to_string_lossy().to_lowercase().contains(&pattern.to_lowercase()) { results.push(p.clone()); } if p.is_dir() { search_recursive(&p, pattern, results); } } } }
