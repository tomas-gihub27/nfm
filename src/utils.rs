use std::path::PathBuf;

// Minimal clipboard implementation since arboard requires X11 dev headers on linux sometimes
pub struct Clipboard {
    content: String,
    is_cut: bool,
    source_path: Option<PathBuf>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            is_cut: false,
            source_path: None,
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.content = text;
    }

    pub fn get_text(&self) -> String {
        self.content.clone()
    }

    pub fn set_file(&mut self, path: PathBuf, cut: bool) {
        self.source_path = Some(path);
        self.is_cut = cut;
    }

    pub fn get_file(&self) -> Option<(PathBuf, bool)> {
        self.source_path.clone().map(|p| (p, self.is_cut))
    }
    
    pub fn clear_file(&mut self) {
        self.source_path = None;
        self.is_cut = false;
    }
}

pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}
