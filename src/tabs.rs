use std::path::PathBuf;

pub enum TabState {
    FileBrowser(crate::file_browser::file_browser::FileBrowserState),
    Editor(crate::editor::editor::EditorState),
}

pub struct Tab {
    pub state: TabState,
}

impl Tab {
    pub fn new_browser(path: PathBuf) -> Self {
        Self {
            state: TabState::FileBrowser(crate::file_browser::file_browser::FileBrowserState::new(path)),
        }
    }

    pub fn new_editor(path: Option<PathBuf>) -> Self {
        Self {
            state: TabState::Editor(crate::editor::editor::EditorState::new(path)),
        }
    }

    pub fn get_title(&self) -> String {
        match &self.state {
            TabState::FileBrowser(fb) => {
                let name = fb.current_dir.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "/".to_string());
                format!("📁 {}", name)
            }
            TabState::Editor(ed) => {
                let name = ed.file_path.as_ref()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "New File".to_string());
                format!("📝 {}", name)
            }
        }
    }
}
