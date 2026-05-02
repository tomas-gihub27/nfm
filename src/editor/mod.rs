pub mod editor {
    use std::path::PathBuf;
    use std::fs;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::utils::Clipboard;
    use crate::config::Config;

    pub struct EditorState {
        pub file_path: Option<PathBuf>,
        pub content: Vec<String>,
        pub cursor_x: usize,
        pub cursor_y: usize,
        pub scroll_y: usize,
        pub scroll_x: usize,
        pub should_quit: bool,
        pub confirm_delete_all: bool,
    }

    impl EditorState {
        pub fn new(path: Option<PathBuf>) -> Self {
            let mut content = vec![String::new()];
            if let Some(p) = &path {
                if p.exists() {
                    if let Ok(text) = fs::read_to_string(p) {
                        content = text.lines().map(|s| s.to_string()).collect();
                        if content.is_empty() {
                            content.push(String::new());
                        }
                    }
                }
            }
            Self {
                file_path: path,
                content,
                cursor_x: 0,
                cursor_y: 0,
                scroll_y: 0,
                scroll_x: 0,
                should_quit: false,
                confirm_delete_all: false,
            }
        }

        pub fn handle_key(&mut self, key: KeyEvent, _clipboard: &mut Clipboard, _config: &Config) {
            if self.confirm_delete_all {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                        self.content = vec![String::new()];
                        self.cursor_x = 0;
                        self.cursor_y = 0;
                        self.confirm_delete_all = false;
                    }
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                        self.confirm_delete_all = false;
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.save();
                }
                KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.confirm_delete_all = true;
                }
                KeyCode::Up => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.cursor_y = 0;
                        self.cursor_x = 0;
                    } else if self.cursor_y > 0 {
                        self.cursor_y -= 1;
                    }
                    self.adjust_cursor_x();
                }
                KeyCode::Down => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.cursor_y = self.content.len() - 1;
                        self.cursor_x = self.content[self.cursor_y].len();
                    } else if self.cursor_y + 1 < self.content.len() {
                        self.cursor_y += 1;
                    }
                    self.adjust_cursor_x();
                }
                KeyCode::Left => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.cursor_x = 0;
                    } else if self.cursor_x > 0 {
                        self.cursor_x -= 1;
                    } else if self.cursor_y > 0 {
                        self.cursor_y -= 1;
                        self.cursor_x = self.content[self.cursor_y].len();
                    }
                }
                KeyCode::Right => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.cursor_x = self.content[self.cursor_y].len();
                    } else if self.cursor_x < self.content[self.cursor_y].len() {
                        self.cursor_x += 1;
                    } else if self.cursor_y + 1 < self.content.len() {
                        self.cursor_y += 1;
                        self.cursor_x = 0;
                    }
                }
                KeyCode::Home => self.cursor_x = 0,
                KeyCode::End => self.cursor_x = self.content[self.cursor_y].len(),
                KeyCode::PageUp => {
                    self.cursor_y = self.cursor_y.saturating_sub(20);
                    self.adjust_cursor_x();
                }
                KeyCode::PageDown => {
                    self.cursor_y = std::cmp::min(self.content.len() - 1, self.cursor_y + 20);
                    self.adjust_cursor_x();
                }
                KeyCode::Backspace => {
                    if self.cursor_x > 0 {
                        self.content[self.cursor_y].remove(self.cursor_x - 1);
                        self.cursor_x -= 1;
                    } else if self.cursor_y > 0 {
                        let current_line = self.content.remove(self.cursor_y);
                        self.cursor_y -= 1;
                        self.cursor_x = self.content[self.cursor_y].len();
                        self.content[self.cursor_y].push_str(&current_line);
                    }
                }
                KeyCode::Delete => {
                    if self.cursor_x < self.content[self.cursor_y].len() {
                        self.content[self.cursor_y].remove(self.cursor_x);
                    } else if self.cursor_y + 1 < self.content.len() {
                        let next_line = self.content.remove(self.cursor_y + 1);
                        self.content[self.cursor_y].push_str(&next_line);
                    }
                }
                KeyCode::Enter => {
                    let current_line = &mut self.content[self.cursor_y];
                    let new_line = current_line.split_off(self.cursor_x);
                    self.content.insert(self.cursor_y + 1, new_line);
                    self.cursor_y += 1;
                    self.cursor_x = 0;
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
                        self.content[self.cursor_y].insert(self.cursor_x, c);
                        self.cursor_x += 1;
                    }
                }
                _ => {}
            }
        }

        fn adjust_cursor_x(&mut self) {
            let len = self.content[self.cursor_y].len();
            if self.cursor_x > len {
                self.cursor_x = len;
            }
        }

        fn save(&self) {
            if let Some(path) = &self.file_path {
                let text = self.content.join("\n");
                let _ = fs::write(path, text);
            }
        }
    }
}
