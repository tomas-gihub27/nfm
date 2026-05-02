use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, BorderType},
    Frame,
};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;
use chrono::{DateTime, Local};

use crate::app::{App, BackgroundTask, TabState};
use crate::file_browser::file_browser::{BrowserMode, DialogType};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), 
            Constraint::Min(0),    
            Constraint::Length(1), 
        ])
        .split(f.size());

    draw_tabs(f, app, chunks[0]);

    let active_tab_idx = app.active_tab;
    match &mut app.tabs[active_tab_idx].state {
        TabState::FileBrowser(state) => draw_file_browser(f, state, chunks[1]),
        TabState::Editor(_) => draw_editor(f, app, chunks[1]),
    }

    draw_status(f, app, chunks[2]);

    if let Some(task) = &app.active_task {
        draw_progress_bar(f, task, f.size());
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let mut tab_spans = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        let title = tab.get_title();
        let is_active = i == app.active_tab;
        
        let mut style = if is_active {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::Gray)
        };

        if is_active {
            match tab.state {
                TabState::FileBrowser(_) => style = style.bg(Color::Rgb(0, 100, 200)),
                TabState::Editor(_) => style = style.bg(Color::Rgb(200, 100, 0)),
            }
        }

        tab_spans.push(Span::styled(format!(" {} ", title), style));
        tab_spans.push(Span::raw(" "));
    }
    let p = Paragraph::new(Line::from(tab_spans)).block(Block::default().bg(Color::Reset));
    f.render_widget(p, area);
}

fn draw_file_browser(f: &mut Frame, state: &mut crate::file_browser::file_browser::FileBrowserState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let list_height = chunks[1].height as usize - 2;
    if state.selected_index < state.scroll_offset {
        state.scroll_offset = state.selected_index;
    } else if state.selected_index >= state.scroll_offset + list_height {
        state.scroll_offset = state.selected_index - list_height + 1;
    }

    let breadcrumb = match state.mode {
        BrowserMode::Drives => " 💻 Tento počítač ".to_string(),
        _ => format!(" 📂 {} ", state.current_dir.to_string_lossy()),
    };
    let path_p = Paragraph::new(breadcrumb)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(path_p, chunks[0]);

    let items_source = match state.mode {
        BrowserMode::Drives => &state.drives,
        _ => &state.items,
    };

    let items: Vec<ListItem> = items_source.iter().enumerate()
        .skip(state.scroll_offset)
        .take(list_height)
        .map(|(i, item)| {
            let mut style = Style::default();
            if item.selected { 
                style = style.fg(Color::Yellow); 
            } else {
                let (r, g, b) = item.get_color();
                style = style.fg(Color::Rgb(r, g, b));
            }

            if i == state.selected_index { 
                style = style.bg(Color::Rgb(50, 50, 50)).add_modifier(Modifier::BOLD);
                if !item.selected {
                    style = style.fg(Color::White);
                }
            }

            let icon = if item.is_dir { "📁" } else { "📄" };
            let mark = if item.selected { "✔ " } else { "  " };
            let size_str = if item.is_dir { "".to_string() } else { crate::utils::format_size(item.size) };
            
            ListItem::new(format!("{} {} {:<40} {}", mark, icon, item.name, size_str)).style(style)
        }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
            .title(" Explorer "));
    f.render_widget(list, chunks[1]);

    match &state.mode {
        BrowserMode::Menu(idx) => {
            let menu_area = centered_rect(40, 60, f.size());
            f.render_widget(Clear, menu_area);
            let menu_items: Vec<ListItem> = state.menu_items.iter().enumerate().map(|(i, s)| {
                let style = if i == *idx { Style::default().bg(Color::Blue).fg(Color::White) } else { Style::default() };
                ListItem::new(format!("  {}", s)).style(style)
            }).collect();
            f.render_widget(List::new(menu_items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .title(" Akce ")), menu_area);
        }
        BrowserMode::Dialog(DialogType::DeleteConfirm) => {
            let dialog_area = centered_rect(40, 10, f.size());
            f.render_widget(Clear, dialog_area);
            f.render_widget(Paragraph::new("\n  Opravdu smazat vybrané položky? (y/n)")
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Potvrdit smazání ")), dialog_area);
        }
        BrowserMode::Dialog(DialogType::Input { title, input, .. }) => {
            let dialog_area = centered_rect(50, 15, f.size());
            f.render_widget(Clear, dialog_area);
            f.render_widget(Paragraph::new(format!("\n  {}", input))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(format!(" {} ", title))), dialog_area);
        }
        BrowserMode::Metadata(item) => {
            draw_metadata(f, item, f.size());
        }
        _ => {}
    }
}

fn draw_metadata(f: &mut Frame, item: &crate::file_browser::file_browser::FileItem, area: Rect) {
    let popup_area = centered_rect(50, 40, area);
    f.render_widget(Clear, popup_area);
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Vlastnosti souboru ");
    f.render_widget(block, popup_area);

    let modified: DateTime<Local> = item.modified.into();
    let text = vec![
        Line::from(vec![Span::styled("Název:    ", Style::default().fg(Color::Gray)), Span::raw(&item.name)]),
        Line::from(vec![Span::styled("Cesta:    ", Style::default().fg(Color::Gray)), Span::raw(item.path.to_string_lossy())]),
        Line::from(vec![Span::styled("Velikost: ", Style::default().fg(Color::Gray)), Span::raw(crate::utils::format_size(item.size))]),
        Line::from(vec![Span::styled("Typ:      ", Style::default().fg(Color::Gray)), Span::raw(if item.is_dir { "Složka" } else { "Soubor" })]),
        Line::from(vec![Span::styled("Změněno:  ", Style::default().fg(Color::Gray)), Span::raw(modified.format("%Y-%m-%d %H:%M:%S").to_string())]),
        Line::from(""),
        Line::from(Span::styled("Stiskněte libovolnou klávesu pro zavření", Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))),
    ];

    let p = Paragraph::new(text).block(Block::default().padding(ratatui::widgets::Padding::uniform(1)));
    f.render_widget(p, popup_area);
}

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    let active_tab_idx = app.active_tab;
    let state = match &mut app.tabs[active_tab_idx].state {
        TabState::Editor(s) => s,
        _ => return,
    };

    let height = area.height as usize - 2;
    let width = area.width as usize - 2 - 5;

    if state.cursor_y < state.scroll_y {
        state.scroll_y = state.cursor_y;
    } else if state.cursor_y >= state.scroll_y + height {
        state.scroll_y = state.cursor_y - height + 1;
    }

    if state.cursor_x < state.scroll_x {
        state.scroll_x = state.cursor_x;
    } else if state.cursor_x >= state.scroll_x + width {
        state.scroll_x = state.cursor_x - width + 1;
    }

    let syntax = state.file_path.as_ref()
        .and_then(|p| p.extension())
        .and_then(|ext| app.syntax_set.find_syntax_by_extension(ext.to_str().unwrap_or("")))
        .unwrap_or_else(|| app.syntax_set.find_syntax_plain_text());
    
    let mut h = HighlightLines::new(syntax, &app.theme_set.themes["base16-ocean.dark"]);
    let full_content = state.content.join("\n");
    let mut lines = Vec::new();
    let mut line_num = 0;

    for line in LinesWithEndings::from(&full_content) {
        if line_num >= state.scroll_y && line_num < state.scroll_y + height {
            let ranges = h.highlight_line(line, &app.syntax_set).unwrap_or_default();
            let mut spans = Vec::new();
            spans.push(Span::styled(format!("{:4} ", line_num + 1), Style::default().fg(Color::DarkGray)));
            
            for (style, text) in ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let mut ratatui_style = Style::default().fg(fg);
                if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                    ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                }
                
                let text_val = text.replace("\n", "").replace("\r", "");
                let current_len = spans_len(&spans) - 5;
                let start_x = state.scroll_x.saturating_sub(current_len);
                
                if start_x < text_val.len() {
                    let remaining_width = width.saturating_sub(current_len);
                    let end_x = std::cmp::min(text_val.len(), start_x + remaining_width);
                    if end_x > start_x {
                        spans.push(Span::styled(text_val[start_x..end_x].to_string(), ratatui_style));
                    }
                }
            }
            lines.push(Line::from(spans));
        } else if line_num < state.scroll_y + height {
            let _ = h.highlight_line(line, &app.syntax_set);
        }
        line_num += 1;
        if line_num >= state.scroll_y + height { break; }
    }

    while lines.len() < height {
        lines.push(Line::from(vec![Span::styled("   ~ ", Style::default().fg(Color::DarkGray))]));
    }

    let p = Paragraph::new(lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
            .title(format!(" Editor: {} ", state.file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".to_string()))));
    f.render_widget(p, area);

    let cur_y = area.y + 1 + (state.cursor_y.saturating_sub(state.scroll_y)) as u16;
    let cur_x = area.x + 1 + 5 + (state.cursor_x.saturating_sub(state.scroll_x)) as u16;
    if cur_y >= area.y + 1 && cur_y < area.y + area.height - 1 && cur_x < area.x + area.width - 1 {
        f.set_cursor(cur_x, cur_y);
    }

    if state.confirm_delete_all {
        let dialog_area = centered_rect(40, 15, f.size());
        f.render_widget(Clear, dialog_area);
        f.render_widget(Paragraph::new("\n  Opravdu smazat vše? (y/n)")
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Varování ")), dialog_area);
    }
}

fn spans_len(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.len()).sum()
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = match &app.tabs[app.active_tab].state {
        TabState::FileBrowser(_) => "  BROWSER  o:Menu  Space:Select  c/v:Copy/Paste ",
        TabState::Editor(_) => "  EDITOR  ^S:Save  ^X:Clear  Esc:Exit ",
    };
    
    let left = Span::styled(mode_str, Style::default().bg(Color::Rgb(40, 100, 200)).fg(Color::Black));
    let msg = Span::styled(format!("  {} ", app.status_message), Style::default().fg(Color::White));
    
    let p = Paragraph::new(Line::from(vec![left, msg])).style(Style::default().bg(Color::Rgb(25, 25, 25)));
    f.render_widget(p, area);
}

fn draw_progress_bar(f: &mut Frame, task: &BackgroundTask, area: Rect) {
    let popup_area = centered_rect(60, 20, area);
    f.render_widget(Clear, popup_area);
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", task.name));
    f.render_widget(block, popup_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(popup_area);

    f.render_widget(Paragraph::new(format!("Položka: {}", task.current_item)).style(Style::default().fg(Color::Gray)), inner[0]);

    let width = inner[1].width as usize;
    let progress_width = (task.progress * width as f64) as usize;
    let bar = format!("{:=<filled$}{:>empty$}", "", "", filled = progress_width, empty = width - progress_width);
    
    f.render_widget(Paragraph::new(bar).style(Style::default().fg(Color::Green).bg(Color::Rgb(30, 30, 30))), inner[1]);
    
    let percentage = (task.progress * 100.0) as u32;
    let elapsed = task.start_time.elapsed();
    let eta = if task.progress > 0.01 {
        let total_est = elapsed.as_secs_f64() / task.progress;
        let remaining = total_est - elapsed.as_secs_f64();
        format!("{}s", remaining as u32)
    } else {
        "--s".to_string()
    };
    
    f.render_widget(Paragraph::new(format!("{}% | Zbývá cca: {}", percentage, eta)).alignment(ratatui::layout::Alignment::Right), inner[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
