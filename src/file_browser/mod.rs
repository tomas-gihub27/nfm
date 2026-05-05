pub mod file_browser {
    use std::path::{Path, PathBuf};
    use std::fs;
    use std::time::SystemTime;
    
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use sysinfo::Disks;

    use crate::utils::Clipboard;
    use crate::config::Config;

    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum ArchiveType { Zip, Tar, Gzip }
    
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum EncType { Xor, AesPlaceholder }

    #[derive(Clone, Debug)]
    pub enum GitTask {
        Status,
        AddAll,
        Commit(String),
        Push { remote: String, branch: String },
        Pull { remote: String, branch: String },
        Fetch,
        Init,
        RemoteAdd { name: String, url: String },
    }

    #[derive(Clone, Debug)]
    pub enum TaskType {
        Copy, Move, Archive(ArchiveType), Unzip, GitClone(String), Wget(String),
        Encrypt { etype: EncType, key: String, output: PathBuf },
        Decrypt { etype: EncType, key: String, output: PathBuf },
        Search(String), Checksum(String), Delete(Vec<PathBuf>),
        Git(GitTask),
    }

    pub enum TabRequest {
        OpenEditor(PathBuf),
        SetStatus(String),
        StartTask { task_type: TaskType, path: PathBuf, target: PathBuf },
    }

    #[derive(Clone, Debug)]
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
            if self.is_dir { return (0, 150, 255); }
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

    pub struct FormField {
        pub label: String,
        pub value: String,
        pub is_password: bool,
    }

    pub enum BrowserMode {
        Normal,
        Drives,
        Menu(usize),
        GitMenu(usize),
        Dialog(DialogType),
        Metadata(FileItem),
        Permissions { item: FileItem, grid: [[bool; 3]; 3], row: usize, col: usize },
        Selection { title: String, options: Vec<String>, selected: usize, action: SelectionAction },
        Form { title: String, fields: Vec<FormField>, active_idx: usize, action: FormAction },
        Help,
    }

    pub enum DialogType {
        DeleteConfirm,
        Input { title: String, input: String, action: DialogAction },
    }

    #[derive(Clone)]
    pub enum DialogAction {
        NewFile, NewFolder, Rename(PathBuf), GitClone, Wget, Symlink(PathBuf), Filter, Search, Duplicate(PathBuf),
        GitCommit, GitRemoteName, GitRemoteUrl(String),
    }
    
    #[derive(Clone)]
    pub enum SelectionAction {
        Archive(PathBuf), Encrypt(PathBuf), Decrypt(PathBuf), Checksum(PathBuf), SortMode,
    }

    #[derive(Clone)]
    pub enum FormAction {
        Archive { path: PathBuf, atype: ArchiveType },
        Encrypt { path: PathBuf, etype: EncType },
        Decrypt { path: PathBuf, etype: EncType },
        GitPush, GitPull, GitRemoteAdd,
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
        pub show_preview: bool,
        pub filter: String,
        pub sort_mode: SortMode,
    }

    #[derive(Clone, Copy, PartialEq)]
    pub enum SortMode { NameAsc, NameDesc, SizeAsc, SizeDesc, DateAsc, DateDesc }

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
                    "--- FILES ---".to_string(), 
                    "Open / Execute".to_string(),
                    "New File".to_string(),
                    "New Folder".to_string(),
                    "Copy Path".to_string(), 
                    "Duplicate".to_string(),
                    "Create Symlink".to_string(), 
                    "Change Permissions".to_string(), 
                    "Properties".to_string(), 
                    "Checksum".to_string(), 
                    "--- TOOLS ---".to_string(), 
                    "Compress (Archive)".to_string(), 
                    "Extract (Extract)".to_string(), 
                    "Bulk Rename".to_string(), 
                    "Git Clone".to_string(), 
                    "Wget (Download)".to_string(), 
                    "Encrypt".to_string(), 
                    "Decrypt".to_string(), 
                    "--- VIEW ---".to_string(), 
                    "Hidden Files: Off/On".to_string(), 
                    "Filter List".to_string(), 
                    "Search Files".to_string(), 
                    "Refresh Panel".to_string(), 
                    "Terminal here".to_string(), 
                    "Git Manager".to_string(),
                    "Settings (Config)".to_string(), 
                    "Preview: Off/On".to_string(), 
                    "Sort by...".to_string(), 
                ],
                drives: Vec::new(),
                show_hidden: false,
                show_preview: false,
                filter: String::new(),
                sort_mode: SortMode::NameAsc,
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
                if a.is_dir && !b.is_dir { return std::cmp::Ordering::Less; }
                if !a.is_dir && b.is_dir { return std::cmp::Ordering::Greater; }
                match self.sort_mode {
                    SortMode::NameAsc => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortMode::NameDesc => b.name.to_lowercase().cmp(&a.name.to_lowercase()),
                    SortMode::SizeAsc => a.size.cmp(&b.size),
                    SortMode::SizeDesc => b.size.cmp(&a.size),
                    SortMode::DateAsc => a.modified.cmp(&b.modified),
                    SortMode::DateDesc => b.modified.cmp(&a.modified),
                }
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
                BrowserMode::GitMenu(_) => self.handle_git_menu_key(key),
                BrowserMode::Dialog(_) => self.handle_dialog_key(key),
                BrowserMode::Metadata(_) | BrowserMode::Help => { if key.code != KeyCode::Null { self.mode = BrowserMode::Normal; } None }
                BrowserMode::Permissions { .. } => self.handle_permissions_key(key),
                BrowserMode::Selection { .. } => self.handle_selection_key(key),
                BrowserMode::Form { .. } => self.handle_form_key(key),
            }
        }

        fn handle_git_menu_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            let mut close_menu = false;
            let mut req = None;
            if let BrowserMode::GitMenu(idx) = &mut self.mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('g') => close_menu = true,
                    KeyCode::Up => if *idx > 1 { *idx -= 1; },
                    KeyCode::Down => if *idx < 8 { *idx += 1; },
                    KeyCode::Enter => {
                        match *idx {
                            1 => req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Status), path: self.current_dir.clone(), target: PathBuf::new() }),
                            2 => req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::AddAll), path: self.current_dir.clone(), target: PathBuf::new() }),
                            3 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Commit Message:".to_string(), input: String::new(), action: DialogAction::GitCommit }); return None; }
                            4 => { self.mode = BrowserMode::Form { 
                                title: "GIT PUSH".to_string(), 
                                fields: vec![
                                    FormField { label: "Remote:".to_string(), value: "origin".to_string(), is_password: false },
                                    FormField { label: "Branch:".to_string(), value: "main".to_string(), is_password: false }
                                ], 
                                active_idx: 0, 
                                action: FormAction::GitPush 
                            }; return None; }
                            5 => { self.mode = BrowserMode::Form { 
                                title: "GIT PULL".to_string(), 
                                fields: vec![
                                    FormField { label: "Remote:".to_string(), value: "origin".to_string(), is_password: false },
                                    FormField { label: "Branch:".to_string(), value: "main".to_string(), is_password: false }
                                ], 
                                active_idx: 0, 
                                action: FormAction::GitPull 
                            }; return None; }
                            6 => req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Fetch), path: self.current_dir.clone(), target: PathBuf::new() }),
                            7 => req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Init), path: self.current_dir.clone(), target: PathBuf::new() }),
                            8 => { self.mode = BrowserMode::Form { 
                                title: "ADD REMOTE".to_string(), 
                                fields: vec![
                                    FormField { label: "Name:".to_string(), value: "origin".to_string(), is_password: false },
                                    FormField { label: "URL:".to_string(), value: String::new(), is_password: false }
                                ], 
                                active_idx: 0, 
                                action: FormAction::GitRemoteAdd 
                            }; return None; }
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

        fn handle_normal_key(&mut self, key: KeyEvent, clipboard: &mut Clipboard, config: &Config) -> Option<TabRequest> {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => { if !self.filter.is_empty() { self.filter.clear(); self.refresh(); return None; } self.should_quit = true; }
                KeyCode::Char('h') | KeyCode::F(1) => { self.mode = BrowserMode::Help; }
                KeyCode::Up => if self.selected_index > 0 { self.selected_index -= 1; },
                KeyCode::Down => if self.selected_index + 1 < self.items.len() { self.selected_index += 1; },
                KeyCode::Right => { if let Some(item) = self.items.get(self.selected_index) { if item.is_dir { self.current_dir = item.path.clone(); self.refresh(); self.selected_index = 0; } } }
                KeyCode::Left => { if let Some(parent) = self.current_dir.parent() { self.current_dir = parent.to_path_buf(); self.refresh(); self.selected_index = 0; } else { self.load_drives(); self.mode = BrowserMode::Drives; self.selected_index = 0; } }
                KeyCode::Enter => { if let Some(item) = self.items.get(self.selected_index) { if item.is_dir { self.current_dir = item.path.clone(); self.refresh(); self.selected_index = 0; } else { return Some(TabRequest::OpenEditor(item.path.clone())); } } }
                KeyCode::Char(' ') => { if let Some(item) = self.items.get_mut(self.selected_index) { item.selected = !item.selected; } if self.selected_index + 1 < self.items.len() { self.selected_index += 1; } }
                KeyCode::Char('o') | KeyCode::Char('O') => { self.mode = BrowserMode::Menu(1); }
                KeyCode::Char('n') | KeyCode::Char('N') => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "New file name:".to_string(), input: String::new(), action: DialogAction::NewFile }); }
                KeyCode::Char('m') | KeyCode::Char('M') => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "New folder name:".to_string(), input: String::new(), action: DialogAction::NewFolder }); }
                KeyCode::Char('r') | KeyCode::Char('R') => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Rename to:".to_string(), input: item.name.clone(), action: DialogAction::Rename(item.path.clone()) }); } }
                KeyCode::Delete | KeyCode::Char('D') => { if config.file_browser.confirm_delete { self.mode = BrowserMode::Dialog(DialogType::DeleteConfirm); } else { return self.start_delete(); } }
                KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => { if let Some(item) = self.items.get(self.selected_index) { clipboard.set_file(item.path.clone(), false); return Some(TabRequest::SetStatus("Copied".to_string())); } }
                KeyCode::Char('x') | KeyCode::Char('X') if key.modifiers.contains(KeyModifiers::CONTROL) => { if let Some(item) = self.items.get(self.selected_index) { clipboard.set_file(item.path.clone(), true); return Some(TabRequest::SetStatus("Cut".to_string())); } }
                KeyCode::Char('v') | KeyCode::Char('V') if key.modifiers.contains(KeyModifiers::CONTROL) => { if let Some((path, is_cut)) = clipboard.get_file() { if let Some(name) = path.file_name() { return Some(TabRequest::StartTask { task_type: if is_cut { TaskType::Move } else { TaskType::Copy }, path: path.clone(), target: self.current_dir.join(name) }); } } }
                _ => {}
            }
            None
        }

        fn start_delete(&mut self) -> Option<TabRequest> {
            let mut to_delete = Vec::new();
            for item in &self.items { if item.selected { to_delete.push(item.path.clone()); } }
            if to_delete.is_empty() { if let Some(item) = self.items.get(self.selected_index) { to_delete.push(item.path.clone()); } }
            if to_delete.is_empty() { return None; }
            Some(TabRequest::StartTask { task_type: TaskType::Delete(to_delete), path: PathBuf::new(), target: PathBuf::new() })
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
                    KeyCode::Up => { if *idx > 1 { *idx -= 1; } if self.menu_items[*idx].starts_with("---") && *idx > 1 { *idx -= 1; } },
                    KeyCode::Down => { if *idx + 1 < self.menu_items.len() { *idx += 1; } if self.menu_items[*idx].starts_with("---") && *idx + 1 < self.menu_items.len() { *idx += 1; } },
                    KeyCode::Right => { if *idx < 10 { *idx = 11; } else if *idx < 18 { *idx = 19; } },
                    KeyCode::Left => { if *idx > 18 { *idx = 11; } else if *idx > 10 { *idx = 1; } },
                    KeyCode::Enter => {
                        match *idx {
                            1 => { if let Some(item) = self.items.get(self.selected_index) { let _ = open::that(&item.path); req = Some(TabRequest::SetStatus("Opened externally".to_string())); } }
                            2 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "New file name:".to_string(), input: String::new(), action: DialogAction::NewFile }); return None; }
                            3 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "New folder name:".to_string(), input: String::new(), action: DialogAction::NewFolder }); return None; }
                            4 => { if let Some(item) = self.items.get(self.selected_index) { clipboard.set_text(item.path.to_string_lossy().to_string()); req = Some(TabRequest::SetStatus("Path copied".to_string())); } }
                            5 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Duplicate as:".to_string(), input: format!("{}_copy", item.name), action: DialogAction::Duplicate(item.path.clone()) }); return None; } }
                            6 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Symlink name:".to_string(), input: format!("{}_link", item.name), action: DialogAction::Symlink(item.path.clone()) }); return None; } }
                            7 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = self.init_permissions(item.clone()); return None; } }
                            8 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Metadata(item.clone()); return None; } }
                            9 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Selection { title: "Algorithm".to_string(), options: vec!["MD5".to_string(), "SHA256".to_string()], selected: 0, action: SelectionAction::Checksum(item.path.clone()) }; return None; } }
                            11 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Selection { title: "Archive type".to_string(), options: vec!["ZIP".to_string(), "TAR".to_string(), "GZIP".to_string()], selected: 0, action: SelectionAction::Archive(item.path.clone()) }; return None; } }
                            12 => { if let Some(item) = self.items.get(self.selected_index) { if item.name.ends_with(".zip") || item.name.ends_with(".tar") || item.name.ends_with(".gz") { req = Some(TabRequest::StartTask { task_type: TaskType::Unzip, path: item.path.clone(), target: self.current_dir.clone() }); } } }
                            13 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Rename pattern:".to_string(), input: String::new(), action: DialogAction::NewFile }); return None; }
                            14 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Git Clone URL:".to_string(), input: String::new(), action: DialogAction::GitClone }); return None; }
                            15 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Wget URL:".to_string(), input: String::new(), action: DialogAction::Wget }); return None; }
                            16 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Selection { title: "Encryption type".to_string(), options: vec!["XOR (Fast)".to_string(), "AES (Placeholder)".to_string()], selected: 0, action: SelectionAction::Encrypt(item.path.clone()) }; return None; } }
                            17 => { if let Some(item) = self.items.get(self.selected_index) { self.mode = BrowserMode::Selection { title: "Encryption type".to_string(), options: vec!["XOR".to_string(), "AES".to_string()], selected: 0, action: SelectionAction::Decrypt(item.path.clone()) }; return None; } }
                            19 => { self.show_hidden = !self.show_hidden; self.refresh(); req = Some(TabRequest::SetStatus(format!("Hidden: {}", if self.show_hidden { "Yes" } else { "No" }))); }
                            20 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Filter:".to_string(), input: self.filter.clone(), action: DialogAction::Filter }); return None; }
                            21 => { self.mode = BrowserMode::Dialog(DialogType::Input { title: "Search pattern:".to_string(), input: String::new(), action: DialogAction::Search }); return None; }
                            22 => { self.refresh(); req = Some(TabRequest::SetStatus("Refreshed".to_string())); }
                            23 => { if cfg!(target_os = "windows") { let _ = std::process::Command::new("cmd").current_dir(&self.current_dir).spawn(); } else { let _ = std::process::Command::new("sh").arg("-c").arg("$TERM").current_dir(&self.current_dir).spawn(); } req = Some(TabRequest::SetStatus("Terminal opened".to_string())); }
                            24 => { self.mode = BrowserMode::GitMenu(1); return None; }
                            25 => { let cp = crate::config::get_config_path(); return Some(TabRequest::OpenEditor(cp)); }
                            26 => { self.show_preview = !self.show_preview; req = Some(TabRequest::SetStatus(format!("Preview: {}", if self.show_preview { "On" } else { "Off" }))); }
                            27 => { self.mode = BrowserMode::Selection { title: "Sort by".to_string(), options: vec!["Name (A-Z)".to_string(), "Name (Z-A)".to_string(), "Size (Smallest)".to_string(), "Size (Largest)".to_string(), "Date (Oldest)".to_string(), "Date (Newest)".to_string()], selected: 0, action: SelectionAction::SortMode }; return None; }
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

        fn init_permissions(&self, item: FileItem) -> BrowserMode {
            let mut grid = [[false; 3]; 3];
            #[cfg(unix)] {
                if let Ok(meta) = fs::metadata(&item.path) {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    grid[0][0] = mode & 0o400 != 0; grid[0][1] = mode & 0o200 != 0; grid[0][2] = mode & 0o100 != 0;
                    grid[1][0] = mode & 0o040 != 0; grid[1][1] = mode & 0o020 != 0; grid[1][2] = mode & 0o010 != 0;
                    grid[2][0] = mode & 0o004 != 0; grid[2][1] = mode & 0o002 != 0; grid[2][2] = mode & 0o001 != 0;
                }
            }
            BrowserMode::Permissions { item, grid, row: 0, col: 0 }
        }

        fn handle_permissions_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            if let BrowserMode::Permissions { item, grid, row, col } = &mut self.mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.mode = BrowserMode::Normal,
                    KeyCode::Up => if *row > 0 { *row -= 1; }, KeyCode::Down => if *row < 2 { *row += 1; },
                    KeyCode::Left => if *col > 0 { *col -= 1; }, KeyCode::Right => if *col < 2 { *col += 1; },
                    KeyCode::Char(' ') | KeyCode::Enter => { grid[*row][*col] = !grid[*row][*col]; },
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        #[cfg(unix)] {
                            let mut mode = 0;
                            if grid[0][0] { mode |= 0o400; } if grid[0][1] { mode |= 0o200; } if grid[0][2] { mode |= 0o100; }
                            if grid[1][0] { mode |= 0o040; } if grid[1][1] { mode |= 0o020; } if grid[1][2] { mode |= 0o010; }
                            if grid[2][0] { mode |= 0o004; } if grid[2][1] { mode |= 0o002; } if grid[2][2] { mode |= 0o001; }
                            use std::os::unix::fs::PermissionsExt;
                            let _ = fs::set_permissions(&item.path, fs::Permissions::from_mode(mode));
                        }
                        self.mode = BrowserMode::Normal; return Some(TabRequest::SetStatus("Permissions saved".to_string()));
                    }
                    _ => {}
                }
            }
            None
        }

        fn handle_selection_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            let mut req = None;
            let mut next_mode = None;
            if let BrowserMode::Selection { options, selected, action, .. } = &mut self.mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => next_mode = Some(BrowserMode::Normal),
                    KeyCode::Up => if *selected > 0 { *selected -= 1; }, KeyCode::Down => if *selected + 1 < options.len() { *selected += 1; },
                    KeyCode::Enter => {
                        match action {
                            SelectionAction::Archive(path) => {
                                let atype = match *selected { 0 => ArchiveType::Zip, 1 => ArchiveType::Tar, _ => ArchiveType::Gzip };
                                let ext = match atype { ArchiveType::Zip => "zip", ArchiveType::Tar => "tar", ArchiveType::Gzip => "tar.gz" };
                                let out = path.file_name().unwrap().to_string_lossy().to_string();
                                next_mode = Some(BrowserMode::Form { title: "ARCHIVE FILE".to_string(), fields: vec![FormField { label: "Output name:".to_string(), value: format!("{}.{}", out, ext), is_password: false }], active_idx: 0, action: FormAction::Archive { path: path.clone(), atype } });
                            }
                            SelectionAction::Encrypt(path) => {
                                let etype = if *selected == 0 { EncType::Xor } else { EncType::AesPlaceholder };
                                next_mode = Some(BrowserMode::Form { title: "ENCRYPT FILE".to_string(), fields: vec![FormField { label: "Password/Key:".to_string(), value: String::new(), is_password: true }, FormField { label: "Output name:".to_string(), value: format!("{}.enc", path.file_name().unwrap().to_string_lossy()), is_password: false }], active_idx: 0, action: FormAction::Encrypt { path: path.clone(), etype } });
                            }
                            SelectionAction::Decrypt(path) => {
                                let etype = if *selected == 0 { EncType::Xor } else { EncType::AesPlaceholder };
                                next_mode = Some(BrowserMode::Form { title: "DECRYPT FILE".to_string(), fields: vec![FormField { label: "Password/Key:".to_string(), value: String::new(), is_password: true }, FormField { label: "Output name:".to_string(), value: path.file_name().unwrap().to_string_lossy().to_string().replace(".enc", ""), is_password: false }], active_idx: 0, action: FormAction::Decrypt { path: path.clone(), etype } });
                            }
                            SelectionAction::Checksum(path) => { req = Some(TabRequest::StartTask { task_type: TaskType::Checksum(options[*selected].clone()), path: path.clone(), target: PathBuf::new() }); next_mode = Some(BrowserMode::Normal); }
                            SelectionAction::SortMode => { self.sort_mode = match *selected { 0 => SortMode::NameAsc, 1 => SortMode::NameDesc, 2 => SortMode::SizeAsc, 3 => SortMode::SizeDesc, 4 => SortMode::DateAsc, _ => SortMode::DateDesc }; self.refresh(); next_mode = Some(BrowserMode::Normal); }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(m) = next_mode { self.mode = m; }
            req
        }

        fn handle_form_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            let mut req = None;
            let mut next_mode = None;
            if let BrowserMode::Form { fields, active_idx, action, .. } = &mut self.mode {
                match key.code {
                    KeyCode::Esc => next_mode = Some(BrowserMode::Normal),
                    KeyCode::Up => if *active_idx > 0 { *active_idx -= 1; },
                    KeyCode::Down | KeyCode::Tab => if *active_idx + 1 < fields.len() { *active_idx += 1; },
                    KeyCode::Backspace => { fields[*active_idx].value.pop(); }
                    KeyCode::Char(c) => { fields[*active_idx].value.push(c); }
                    KeyCode::Enter => {
                        if *active_idx + 1 < fields.len() { *active_idx += 1; }
                        else {
                            match action {
                                FormAction::Archive { path, atype } => {
                                    let target = path.parent().unwrap().join(&fields[0].value);
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Archive(*atype), path: path.clone(), target });
                                }
                                FormAction::Encrypt { path, etype } => {
                                    let target = path.parent().unwrap().join(&fields[1].value);
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Encrypt { etype: *etype, key: fields[0].value.clone(), output: target.clone() }, path: path.clone(), target });
                                }
                                FormAction::Decrypt { path, etype } => {
                                    let target = path.parent().unwrap().join(&fields[1].value);
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Decrypt { etype: *etype, key: fields[0].value.clone(), output: target.clone() }, path: path.clone(), target });
                                }
                                FormAction::GitPush => {
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Push { remote: fields[0].value.clone(), branch: fields[1].value.clone() }), path: self.current_dir.clone(), target: PathBuf::new() });
                                }
                                FormAction::GitPull => {
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Pull { remote: fields[0].value.clone(), branch: fields[1].value.clone() }), path: self.current_dir.clone(), target: PathBuf::new() });
                                }
                                FormAction::GitRemoteAdd => {
                                    req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::RemoteAdd { name: fields[0].value.clone(), url: fields[1].value.clone() }), path: self.current_dir.clone(), target: PathBuf::new() });
                                }
                            }
                            next_mode = Some(BrowserMode::Normal);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(m) = next_mode { self.mode = m; }
            req
        }

        fn handle_dialog_key(&mut self, key: KeyEvent) -> Option<TabRequest> {
            let mut next_mode = None;
            let mut req = None;
            match &mut self.mode {
                BrowserMode::Dialog(DialogType::DeleteConfirm) => { match key.code { KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => { req = self.start_delete(); next_mode = Some(BrowserMode::Normal); } KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => { next_mode = Some(BrowserMode::Normal); } _ => {} } }
                BrowserMode::Dialog(DialogType::Input { input, action, .. }) => {
                    match key.code {
                        KeyCode::Esc => next_mode = Some(BrowserMode::Normal),
                        KeyCode::Enter => {
                            match action {
                                DialogAction::NewFile => { let _ = fs::File::create(self.current_dir.join(&input)); }
                                DialogAction::NewFolder => { let _ = fs::create_dir_all(self.current_dir.join(&input)); }
                                DialogAction::Rename(old_path) => { let _ = fs::rename(old_path, self.current_dir.join(&input)); }
                                DialogAction::Duplicate(old_path) => { 
                                    let target_path = self.current_dir.join(&input);
                                    if old_path.is_dir() {
                                        // A simple duplicate for dirs isn't easily done without recursive copy, 
                                        // so we will just create the dir and let user copy contents if needed,
                                        // or use task system. For simplicity, request task system:
                                        req = Some(TabRequest::StartTask { task_type: TaskType::Copy, path: old_path.clone(), target: target_path });
                                    } else {
                                        let _ = fs::copy(old_path, target_path); 
                                    }
                                }
                                DialogAction::GitClone => { req = Some(TabRequest::StartTask { task_type: TaskType::GitClone(input.clone()), path: PathBuf::new(), target: self.current_dir.clone() }); }
                                DialogAction::Wget => { req = Some(TabRequest::StartTask { task_type: TaskType::Wget(input.clone()), path: PathBuf::new(), target: self.current_dir.clone() }); }
                                DialogAction::Symlink(src) => { 
                                    let target_path = self.current_dir.join(&input);
                                    #[cfg(unix)] { let _ = std::os::unix::fs::symlink(src, target_path); }
                                    #[cfg(windows)] { if src.is_dir() { let _ = std::os::windows::fs::symlink_dir(src, target_path); } else { let _ = std::os::windows::fs::symlink_file(src, target_path); } } 
                                }
                                DialogAction::Filter => { self.filter = input.clone(); }
                                DialogAction::Search => { req = Some(TabRequest::StartTask { task_type: TaskType::Search(input.clone()), path: self.current_dir.clone(), target: PathBuf::new() }); }
                                DialogAction::GitCommit => { req = Some(TabRequest::StartTask { task_type: TaskType::Git(GitTask::Commit(input.clone())), path: self.current_dir.clone(), target: PathBuf::new() }); }
                                _ => {}
                            }
                            self.refresh(); next_mode = Some(BrowserMode::Normal);
                        }
                        KeyCode::Backspace => { input.pop(); } KeyCode::Char(c) => { input.push(c); } _ => {}
                    }
                }
                _ => {}
            }
            if let Some(m) = next_mode { self.mode = m; }
            req
        }
    }
}
