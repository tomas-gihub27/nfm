pub mod file_browser {
    use std::path::{Path, PathBuf};
    use std::fs;
    use std::time::SystemTime;
    
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use sysinfo::Disks;

    use crate::utils::Clipboard;
    use crate::config::Config;

    #[derive(Clone)]
    pub enum TaskType {
        Copy,
        Move,
        Zip,
        Unzip,
        GitClone(String),
        Wget(String),
    }

    pub enum TabRequest {
        OpenEditor(PathBuf),
        SetStatus(String),
        StartTask { task_type: TaskType, path: PathBuf, target: PathBuf },
    }

    #[derive(Clone)]
    pub struct FileItem {
        pub path: PathBuf,
        pub is_dir: bool,
        pub size: u64,
        pub name: String,
        pub selected: bool,
        pub modified: SystemTime,
    }

    impl FileItem {
        pub fn get_color(&self) -> (u8, u8, u8) {
            if self.is_dir {
                return (0, 150, 255); 
            }
            let ext = self.path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            match ext.as_str() {
                "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "php" | "rb" => (255, 165, 0),
                "md" | "txt" | "log" | "conf" | "toml" | "yaml" | "yml" | "json" => (144, 238, 144),
                "zip" | "tar" | "gz" | "7z" | "rar" | "bz2" | "xz" => (255, 100, 100),
                "jpg" | "jpeg" | "png" | "gif" | "svg" | "bmp" | "webp" => (238, 130, 238),
                "mp3" | "wav" | "flac" | "ogg" | "m4a" => (255, 215, 0),
                "mp4" | "mkv" | "avi" | "mov" | "webm" => (255, 215, 0),
                "exe" | "sh" | "bat" | "msi" | "appimage" | "bin" => (0, 255, 127),
                _ => (200, 200, 200),
            }
        }
    }

    pub enum BrowserMode {
        Normal,
        Drives,
        Menu(usize),
        Dialog(DialogType),
        Metadata(FileItem),
    }

    pub enum DialogType {
        DeleteConfirm,
        Input { title: String, input: String, action: DialogAction },
    }

    #[derive(Clone)]
    pub enum DialogAction {
        NewFile,
        NewFolder,
        Rename(PathBuf),
        GitClone,
        Wget,
        Symlink(PathBuf),
        Filter,
    }

    pub struct FileBrowserState {
        pub current_dir: PathBuf,
        pub items: Vec<FileItem>,
        pub selected_index: usize,
        pub scroll_offset: usize,
        pub mode: BrowserMode,
        pub should_quit: bool,
        pub menu_items: Vec<String>,
        pub drives: Vec<FileItem>,
        pub show_hidden: bool,
        pub filter: String,
    }

    impl FileBrowserState {
        pub fn new(path: PathBuf) -> Self {
            let mut state = Self {
                current_dir: path,
                items: Vec::new(),
                selected_index: 0,
                scroll_offset: 0,
                mode: BrowserMode::Normal,
                should_quit: false,
                menu_items: vec![
                    "1. Zabalit (Zip)".to_string(),
                    "2. Rozbalit (Unzip)".to_string(),
                    "3. Skryté soubory: Vyp/Zap".to_string(),
                    "4. Terminál zde".to_string(),
                    "5. Obnovit".to_string(),
                    "6. Kopírovat cestu".to_string(),
                    "7. Git Clone".to_string(),
                    "8. Wget (Stáhnout)".to_string(),
                    "9. Vytvořit Symlink".to_string(),
                    "10. Vlastnosti (Metadata)".to_string(),
                    "11. Filtrovat seznam".to_string(),
                ],
                drives: Vec::new(),
                show_hidden: false,
                filter: String::new(),
            };
            state.refresh();
            state
        }

        pub fn refresh(&mut self) {
            self.items.clear();
            if let Ok(entries) = fs::read_dir(&self.current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !self.show_hidden && name.starts_with('.') && name != "." && name != ".." { continue; }
                    if !self.filter.is_empty() && !name.to_lowercase().contains(&self.filter.to_lowercase()) { continue; }
                    let metadata = entry.metadata();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    let modified = metadata.as_ref().map(|m| m.modified().unwrap_or(SystemTime::UNIX_EPOCH)).unwrap_or(SystemTime::UNIX_EPOCH);
                    self.items.push(FileItem { path, is_dir: entry.path().is_dir(), size, name, selected: false, modified });
                }
            }
            self.items.sort_by(|a, b| {
                if a.is_dir && !b.is_dir { std::cmp::Ordering::Less }
                else if !a.is_dir && b.is_dir { std::cmp::Ordering::Greater }
                else { a.name.to_lowercase().cmp(&b.name.to_lowercase()) }
            });
            if self.selected_index >= self.items.len() && !self.items.is_empty() { self.selected_index = self.items.len() - 1; }
            else if self.items.is_empty() { self.selected_index = 0; }
        }

        fn load_drives(&mut self) {
            self.drives.clear();
            let disks = Disks::new_with_refreshed_list();
            for disk in disks.list() {
                self.drives.push(FileItem {
                    path: disk.mount_point().to_path_buf(),
                    is_dir: true,
                    size: disk.total_space(),
                    name: disk.mount_point().to_string_lossy().into_owned(),
                    selected: false,
                    modified: SystemTime::now(),
                });
            }
        }

        pub fn handle_key(&mut self, key: KeyEvent, clipboard: &mut Clipboard, config: &Config) -> Option<TabRequest> {
            match &mut self.mode {
                BrowserMode::Normal => self.handle_normal_key(key, clipboard, config),
                BrowserMode::Drives => self.handle_drives_key(key),
                BrowserMode::Menu(_) => self.handle_menu_key(key, clipboard),
                BrowserMode::Dialog(_) => self.handle_dialog_key(key),
                BrowserMode::Metadata(_) => { self.mode = BrowserMode::Normal; None }
            }
        }

        fn handle_normal_key(&mut self, key: KeyEvent, clipboard: &mut Clipboard, config: &Config) -> Option<TabRequest> {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => { if !self.filter.is_empty() { self.filter.clear(); self.refresh(); return None; } self.should_quit = true; }
                KeyCode::Up => if self.selected_index > 0 { self.selected_index -= 1; },
                KeyCode::Down => if self.selected_index + 1 < self.items.len() { self.selected_index += 1; },
                KeyCode::Right => { if let Some(item) = self.items.get(self.selected_index) { if item.is_dir { self.current_dir = item.path.clone(); self.refresh(); self.selected_index = 0; } } }
                KeyCode::Left => { if let Some(parent) = self.current_dir.parent() { self.current_dir = parent.to_path_buf(); self.refresh(); self.selected_index = 0; } else { self.load_drives(); self.mode = BrowserMode::Drives; self.selected_index = 0; } }
                KeyCode::Enter => { if let Some(item) = self.items.get(self.selected_index) { if item.is_dir { self.current_dir = item.path.clone(); self.refresh(); self.selected_index = 0; } else { return Some(TabRequest::OpenEditor(item.path.clone())); } } }
                KeyCode::Char(' ') => { if let Some(item) = self.items.get_mut(self.selected_index) { item.selected = !item.selected; } if self.selected_index + 1 < self.items.len() { self.selected_index += 1; } }
                KeyCode::Char('o') | KeyCode::Char('O') => { self.mode = BrowserMode::Menu(0); }
                KeyCode::Char('n') | KeyCode::Char('N') => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Název nového souboru:".to_string(), input: String::new(), action: DialogAction::NewFile }); }
                KeyCode::Char('m') | KeyCode::Char('M') => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Název nové složky:".to_string(), input: String::new(), action: DialogAction::NewFolder }); }
                KeyCode::Char('r') | KeyCode::Char('R') => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Přejmenovat na:".to_string(), input: item.name.clone(), action: DialogAction::Rename(item.path.clone()) }); } }
                KeyCode::Delete | KeyCode::Char('D') => { if config.file_browser.confirm_delete { self.mode = BrowserMode::Dialog(DialogType::DeleteConfirm); } else { self.delete_selected(); } }
                
                // Copy/Cut/Paste supporting both raw keys and Ctrl modifiers
                KeyCode::Char('c') | KeyCode::Char('C') => {
                     if let Some(item) = self.items.get(self.selected_index) {
                         clipboard.set_file(item.path.clone(), false);
                         return Some(TabRequest::SetStatus("Zkopírováno".to_string()));
                     }
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                     if let Some(item) = self.items.get(self.selected_index) {
                         clipboard.set_file(item.path.clone(), true);
                         return Some(TabRequest::SetStatus("Vyjmuto".to_string()));
                     }
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                     if let Some((path, is_cut)) = clipboard.get_file() {
                         if let Some(name) = path.file_name() {
                             return Some(TabRequest::StartTask { 
                                 task_type: if is_cut { TaskType::Move } else { TaskType::Copy }, 
                                 path: path.clone(), 
                                 target: self.current_dir.join(name) 
                             });
                         }
                     }
                }
                _ => {}
            }
            None
        }

        fn handle_drives_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Up => if self.selected_index > 0 { self.selected_index -= 1; },
                KeyCode::Down => if self.selected_index + 1 < self.drives.len() { self.selected_index += 1; },
                KeyCode::Enter | KeyCode::Right => { if let Some(drive) = self.drives.get(self.selected_index) { self.current_dir = drive.path.clone(); self.mode = BrowserMode::Normal; self.refresh(); self.selected_index = 0; } }
                _ => {}
            }
            None
        }

        fn handle_menu_key(&mut self, key: KeyEvent, clipboard: &mut Clipboard) -> Option<TabRequest> {
            let mut close_menu = false;
            let mut req = None;
            if let BrowserMode::Menu(idx) = &mut self.mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') | KeyCode::Char('O') => close_menu = true,
                    KeyCode::Up => if *idx > 0 { *idx -= 1; },
                    KeyCode::Down => if *idx + 1 < self.menu_items.len() { *idx += 1; },
                    KeyCode::Enter => {
                        match *idx {
                            0 => { if let Some(item) = self.items.get(self.selected_index) { req = Some(TabRequest::StartTask { task_type: TaskType::Zip, path: item.path.clone(), target: self.current_dir.join(format!("{}.zip", item.name)) }); } }
                            1 => { if let Some(item) = self.items.get(self.selected_index) { if item.name.ends_with(".zip") { req = Some(TabRequest::StartTask { task_type: TaskType::Unzip, path: item.path.clone(), target: self.current_dir.clone() }); } } }
                            2 => { self.show_hidden = !self.show_hidden; self.refresh(); req = Some(TabRequest::SetStatus(format!("Skryté: {}", if self.show_hidden { "Ano" } else { "Ne" }))); }
                            3 => { if cfg!(target_os = "windows") { let _ = std::process::Command::new("cmd").current_dir(&self.current_dir).spawn(); } else { let _ = std::process::Command::new("sh").arg("-c").arg("$TERM").current_dir(&self.current_dir).spawn(); } req = Some(TabRequest::SetStatus("Terminál otevřen".to_string())); }
                            4 => { self.refresh(); req = Some(TabRequest::SetStatus("Obnoveno".to_string())); }
                            5 => { if let Some(item) = self.items.get(self.selected_index) { clipboard.set_text(item.path.to_string_lossy().to_string()); req = Some(TabRequest::SetStatus("Cesta zkopírována".to_string())); } }
                            6 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Git Clone URL:".to_string(), input: String::new(), action: DialogAction::GitClone }); return None; }
                            7 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Wget URL:".to_string(), input: String::new(), action: DialogAction::Wget }); return None; }
                            8 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Název symlinku:".to_string(), input: format!("{}_link", item.name), action: DialogAction::Symlink(item.path.clone()) }); return None; } }
                            9 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Metadata(item.clone()); return None; } }
                            10 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Filtr:".to_string(), input: self.filter.clone(), action: DialogAction::Filter }); return None; }
                            _ => {}
                        }
                        close_menu = true;
                    }
                    _ => {}
                }
            }
            if close_menu { self.mode = BrowserMode::Normal; }
            req
        }

        fn handle_dialog_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            let mut next_mode = None;
            let mut req = None;
            match &mut self.mode {
                BrowserMode::Dialog(DialogType::DeleteConfirm) => { match key.code { KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => { self.delete_selected(); next_mode = Some(BrowserMode::Normal); } KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => { next_mode = Some(BrowserMode::Normal); } _ => {} } }
                BrowserMode::Dialog(DialogType::Input { input, action, .. }) => {
                    match key.code {
                        KeyCode::Esc => next_mode = Some(BrowserMode::Normal),
                        KeyCode::Enter => {
                            match action {
                                DialogAction::NewFile => { let _ = fs::File::create(self.current_dir.join(&input)); }
                                DialogAction::NewFolder => { let _ = fs::create_dir_all(self.current_dir.join(&input)); }
                                DialogAction::Rename(old_path) => { let _ = fs::rename(old_path, self.current_dir.join(&input)); }
                                DialogAction::GitClone => { req = Some(TabRequest::StartTask { task_type: TaskType::GitClone(input.clone()), path: PathBuf::new(), target: self.current_dir.clone() }); }
                                DialogAction::Wget => { req = Some(TabRequest::StartTask { task_type: TaskType::Wget(input.clone()), path: PathBuf::new(), target: self.current_dir.clone() }); }
                                DialogAction::Symlink(src) => { 
                                    let target_path = self.current_dir.join(&input);
                                    #[cfg(unix)] { let _ = std::os::unix::fs::symlink(src, target_path); }
                                    #[cfg(windows)] { 
                                        if src.is_dir() { let _ = std::os::windows::fs::symlink_dir(src, target_path); }
                                        else { let _ = std::os::windows::fs::symlink_file(src, target_path); }
                                    } 
                                }
                                DialogAction::Filter => { self.filter = input.clone(); }
                            }
                            self.refresh();
                            next_mode = Some(BrowserMode::Normal);
                        }
                        KeyCode::Backspace => { input.pop(); }
                        KeyCode::Char(c) => { input.push(c); }
                        _ => {}
                    }
                }
                _ => {}
            }
            if let Some(m) = next_mode { self.mode = m; }
            req
        }

        fn delete_selected(&mut self) {
            let mut to_delete = Vec::new();
            for item in &self.items { if item.selected { to_delete.push(item.path.clone()); } }
            if to_delete.is_empty() { if let Some(item) = self.items.get(self.selected_index) { to_delete.push(item.path.clone()); } }
            for path in to_delete { if path.is_dir() { let _ = fs::remove_dir_all(&path); } else { let _ = fs::remove_file(&path); } }
            self.refresh();
        }
    }
}
