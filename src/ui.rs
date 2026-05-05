use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, BorderType, Cell, Row, Table, Padding},
    Frame,
};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use chrono::{DateTime, Local};

use crate::app::{App, BackgroundTask, TabState};
use crate::file_browser::file_browser::{BrowserMode, DialogType, FileItem, FormField};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(f.size());

    draw_tabs(f, app, chunks[0]);

    {
        let active_tab_idx = app.active_tab;
        let syntax_set = &app.syntax_set;
        let theme_set = &app.theme_set;
        match &mut app.tabs[active_tab_idx].state {
            TabState::FileBrowser(state) => draw_file_browser(f, state, syntax_set, theme_set, chunks[1]),
            TabState::Editor(state) => draw_editor(f, state, syntax_set, theme_set, chunks[1]),
        }
    }

    draw_status(f, app, chunks[2]);

    if let Some(task) = &app.active_task {
        draw_progress_bar(f, task, app.anim_frame, f.size());
    }

    if let Some(err) = &app.error_popup {
        draw_error_popup(f, err, f.size());
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
            Style::default().bg(Color::Rgb(25, 25, 30)).fg(Color::Rgb(100, 100, 110))
        };

        if is_active {
            match tab.state {
                TabState::FileBrowser(_) => style = style.bg(Color::Rgb(0, 120, 215)),
                TabState::Editor(_) => style = style.bg(Color::Rgb(255, 140, 0)),
            }
            // Subtle "pulse" effect for active tab
            if (app.anim_frame / 10) % 2 == 0 {
                style = style.add_modifier(Modifier::REVERSED);
            }
        }

        tab_spans.push(Span::styled(format!("  {}  ", title), style));
        tab_spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(tab_spans)).bg(Color::Rgb(15, 15, 20)), area);
}

fn draw_file_browser(f: &mut Frame, state: &mut crate::file_browser::file_browser::FileBrowserState, syntax_set: &SyntaxSet, theme_set: &ThemeSet, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let list_height = chunks[1].height as usize - 2;
    if state.selected_index < state.scroll_offset { state.scroll_offset = state.selected_index; }
    else if state.selected_index >= state.scroll_offset + list_height { state.scroll_offset = state.selected_index - list_height + 1; }

    // Glassy Breadcrumbs
    let bread_bg = Color::Rgb(30, 30, 40);
    let bread_text = vec![
        Span::styled("  ", Style::default().bg(bread_bg).fg(Color::Rgb(0, 200, 255))),
        Span::styled(format!(" {} ", state.current_dir.to_string_lossy()), Style::default().bg(bread_bg).fg(Color::White).add_modifier(Modifier::BOLD)),
    ];
    f.render_widget(Paragraph::new(Line::from(bread_text)).bg(bread_bg), chunks[0]);

    let items_source = if let BrowserMode::Drives = state.mode { &state.drives } else { &state.items };
    let items: Vec<ListItem> = items_source.iter().enumerate().skip(state.scroll_offset).take(list_height).map(|(i, item)| {
        let is_selected = i == state.selected_index;
        let mut style = Style::default();
        if item.selected { style = style.fg(Color::Rgb(255, 255, 100)); }
        else { let (r, g, b) = item.get_color(); style = style.fg(Color::Rgb(r, g, b)); }
        
        if is_selected { 
            style = style.bg(Color::Rgb(45, 55, 75)).add_modifier(Modifier::BOLD);
            if !item.selected { style = style.fg(Color::White); }
        }

        let icon = if item.is_dir { "󰉋 " } else { "󰈔 " };
        let mark = if item.selected { "󰄲 " } else { "  " };
        let size_str = if item.is_dir { "".to_string() } else { crate::utils::format_size(item.size) };
        
        ListItem::new(Line::from(vec![
            Span::styled(mark, style),
            Span::styled(icon, style),
            Span::styled(format!(" {:<40} ", item.name), style),
            Span::styled(size_str, style.fg(Color::Rgb(100, 100, 110))),
        ]))
    }).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 70)))
        .title(Span::styled(" 󰙅 NEO EXPLORER ", Style::default().fg(Color::Rgb(120, 120, 140)).add_modifier(Modifier::BOLD)));

    if state.show_preview && matches!(state.mode, BrowserMode::Normal) {
        let horizontal = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(45), Constraint::Percentage(55)]).split(chunks[1]);
        f.render_widget(List::new(items).block(block), horizontal[0]);
        if let Some(selected_item) = items_source.get(state.selected_index) {
            draw_preview(f, selected_item, syntax_set, theme_set, horizontal[1]);
        }
    } else {
        f.render_widget(List::new(items).block(block), chunks[1]);
    }

    match &state.mode {
        BrowserMode::Menu(idx) => {
            let menu_area = centered_rect(80, 65, f.size());
            f.render_widget(Clear, menu_area);
            f.render_widget(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Rgb(0, 160, 255)))
                .title(Span::styled(" 󱓞 COMMAND CENTER ", Style::default().fg(Color::White).bold()))
                .title_alignment(Alignment::Center), menu_area);

            let columns = Layout::default().direction(Direction::Horizontal).margin(2).constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)]).split(menu_area);
            
            // Re-render columns with better design
            let c1 = vec![ListItem::new(" 󰈔 FILES ").cyan().bold().bg(Color::Rgb(30, 40, 60)), 
                render_menu_item("Open / Execute", 1, *idx), 
                render_menu_item("New File", 2, *idx),
                render_menu_item("New Folder", 3, *idx),
                render_menu_item("Copy Path", 4, *idx), 
                render_menu_item("Duplicate", 5, *idx),
                render_menu_item("Create Symlink", 6, *idx), 
                render_menu_item("Change Permissions", 7, *idx), 
                render_menu_item("Properties", 8, *idx), 
                render_menu_item("Checksum", 9, *idx)];
            let c2 = vec![ListItem::new(" 󰘦 TOOLS ").yellow().bold().bg(Color::Rgb(50, 45, 20)), 
                render_menu_item("Compress", 11, *idx), 
                render_menu_item("Extract", 12, *idx), 
                render_menu_item("Bulk Rename", 13, *idx), 
                render_menu_item("Git Clone", 14, *idx), 
                render_menu_item("Wget Download", 15, *idx), 
                render_menu_item("Encrypt", 16, *idx), 
                render_menu_item("Decrypt", 17, *idx)];
            let c3 = vec![ListItem::new(" 󰨇 VIEW ").magenta().bold().bg(Color::Rgb(50, 25, 50)), 
                render_menu_item("Hidden Files", 19, *idx), 
                render_menu_item("Filter", 20, *idx), 
                render_menu_item("Search", 21, *idx), 
                render_menu_item("Refresh Panel", 22, *idx), 
                render_menu_item("Terminal", 23, *idx), 
                render_menu_item("Settings", 24, *idx), 
                render_menu_item("Preview", 25, *idx), 
                render_menu_item("Sort", 26, *idx)];
            
            f.render_widget(List::new(c1), columns[0]);
            f.render_widget(List::new(c2), columns[1]);
            f.render_widget(List::new(c3), columns[2]);
        }
        BrowserMode::Dialog(DialogType::Input { title, input, .. }) => {
            let area = centered_rect(50, 15, f.size());
            f.render_widget(Clear, area);
            let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(format!(" 󰋚 {} ", title)).border_style(Style::default().fg(Color::Cyan));
            f.render_widget(Paragraph::new(format!("\n  ❯ {}", input)).block(block), area);
        }
        BrowserMode::Dialog(DialogType::DeleteConfirm) => {
            let area = centered_rect(40, 10, f.size());
            f.render_widget(Clear, area);
            f.render_widget(Paragraph::new("\n  Really delete selected items? (y/n)")
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" 󰆴 CONFIRMATION ").border_style(Style::default().fg(Color::Red))), area);
        }
        BrowserMode::Metadata(item) => draw_metadata(f, item, f.size()),
        BrowserMode::Permissions { item, grid, row, col } => draw_permissions(f, item, grid, *row, *col, f.size()),
        BrowserMode::Selection { title, options, selected, .. } => {
            let area = centered_rect(35, 35, f.size());
            f.render_widget(Clear, area);
            let items: Vec<ListItem> = options.iter().enumerate().map(|(i, s)| {
                let mut style = Style::default();
                if i == *selected { style = style.bg(Color::Rgb(0, 120, 215)).fg(Color::White).bold(); }
                ListItem::new(format!("  󰄾 {}", s)).style(style)
            }).collect();
            f.render_widget(List::new(items).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(format!(" 󰒺 {} ", title)).border_style(Style::default().fg(Color::Cyan))), area);
        }
        BrowserMode::Form { title, fields, active_idx, .. } => draw_form(f, title, fields, *active_idx, f.size()),
        BrowserMode::Help => draw_help(f, f.size()),
        _ => {}
    }
}

fn draw_form(f: &mut Frame, title: &str, fields: &[FormField], active_idx: usize, area: Rect) {
    let area = centered_rect(60, 45, area);
    f.render_widget(Clear, area);
    f.render_widget(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(format!(" 󰈙 {} ", title)).border_style(Style::default().fg(Color::Cyan)).padding(Padding::uniform(1)), area);
    let chunks = Layout::default().direction(Direction::Vertical).margin(1).constraints(fields.iter().map(|_| Constraint::Length(3)).collect::<Vec<_>>()).split(area);
    for (i, field) in fields.iter().enumerate() {
        let active = i == active_idx;
        let val = if field.is_password { "•".repeat(field.value.len()) } else { field.value.clone() };
        let b = Block::default().borders(Borders::ALL).border_type(if active { BorderType::Thick } else { BorderType::Plain }).title(field.label.as_str()).border_style(if active { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) });
        f.render_widget(Paragraph::new(format!(" {}", val)).block(b), chunks[i]);
    }
}

fn render_menu_item(label: &str, index: usize, current_idx: usize) -> ListItem {
    let active = index == current_idx;
    let style = if active { Style::default().bg(Color::Rgb(60, 70, 90)).fg(Color::White).bold() } else { Style::default().fg(Color::Rgb(160, 160, 170)) };
    ListItem::new(format!("{} {}", if active { " 󰁔" } else { "   " }, label)).style(style)
}

fn draw_metadata(f: &mut Frame, item: &FileItem, area: Rect) {
    let area = centered_rect(55, 45, area);
    f.render_widget(Clear, area);
    let mod_time: DateTime<Local> = item.modified.into();
    let text = vec![
        Line::from(vec![Span::styled("  NAME:     ", Style::default().cyan()), Span::styled(&item.name, Style::default().bold())]),
        Line::from(vec![Span::styled("  PATH:     ", Style::default().cyan()), Span::raw(item.path.to_string_lossy())]),
        Line::from(vec![Span::styled("  SIZE:     ", Style::default().cyan()), Span::raw(crate::utils::format_size(item.size))]),
        Line::from(vec![Span::styled("  TYPE:      ", Style::default().cyan()), Span::raw(if item.is_dir { "Folder" } else { "File" })]),
        Line::from(vec![Span::styled("  MODIFIED:  ", Style::default().cyan()), Span::raw(mod_time.format("%Y-%m-%d %H:%M:%S").to_string())]),
    ];
    f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" 󰋽 INFO ").border_style(Style::default().fg(Color::Green)).padding(Padding::uniform(1))), area);
}

fn draw_permissions(f: &mut Frame, item: &FileItem, grid: &[[bool; 3]; 3], row: usize, col: usize, area: Rect) {
    let area = centered_rect(60, 50, area);
    f.render_widget(Clear, area);
    let rows = vec!["Owner", "Group", "Others"].into_iter().enumerate().map(|(r, name)| {
        let cells = vec![Cell::from(name)].into_iter().chain((0..3).map(|c| {
            let style = if r == row && c == col { Style::default().bg(Color::Rgb(0, 120, 215)).fg(Color::White).bold() } else { Style::default().fg(if grid[r][c] { Color::Green } else { Color::DarkGray }) };
            Cell::from(if grid[r][c] { " 󰄬 " } else { " 󰄱 " }).style(style)
        })).collect::<Vec<_>>();
        Row::new(cells)
    });
    let table = Table::new(rows, [Constraint::Percentage(40), Constraint::Percentage(20), Constraint::Percentage(20), Constraint::Percentage(20)])
        .header(Row::new(vec!["Category", "Read", "Write", "Exec"]).cyan().bold())
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Double).title(format!(" 󰒓 PERMISSIONS: {} ", item.name)).border_style(Style::default().fg(Color::Yellow)));
    f.render_widget(table, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let area = centered_rect(80, 80, area);
    f.render_widget(Clear, area);
    let text = vec![
        Line::from(vec![Span::styled(" 󱊖 NEOFM LEGENDARY KEYS ", Style::default().bg(Color::Cyan).fg(Color::Black).bold())]),
        Line::from(""),
        Line::from(vec![Span::styled(" GLOBAL: ", Style::default().magenta().bold())]),
        Line::from("   Tab        - Switch tab"),
        Line::from("   Ctrl+T / W - New / Close tab"),
        Line::from("   H / F1     - Help"),
        Line::from(""),
        Line::from(vec![Span::styled(" NAVIGATION: ", Style::default().cyan().bold())]),
        Line::from("   Arrows      - Movement"),
        Line::from("   Enter      - Enter / Edit"),
        Line::from("   Space   - Select file"),
        Line::from("   C / X / V  - Copy / Cut / Paste"),
        Line::from("   O          - Command Center"),
    ];
    f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" 󰘥 HELP ").border_style(Style::default().fg(Color::Blue)).padding(Padding::uniform(1))), area);
}

fn draw_editor(f: &mut Frame, state: &mut crate::editor::editor::EditorState, syntax_set: &SyntaxSet, theme_set: &ThemeSet, area: Rect) {
    let height = area.height as usize - 2; let width = area.width as usize - 7;
    if state.cursor_y < state.scroll_y { state.scroll_y = state.cursor_y; } else if state.cursor_y >= state.scroll_y + height { state.scroll_y = state.cursor_y - height + 1; }
    if state.cursor_x < state.scroll_x { state.scroll_x = state.cursor_x; } else if state.cursor_x >= state.scroll_x + width { state.scroll_x = state.cursor_x - width + 1; }
    let syntax = state.file_path.as_ref().and_then(|p| p.extension()).and_then(|ext| syntax_set.find_syntax_by_extension(ext.to_str().unwrap_or(""))).or_else(|| if !state.content.is_empty() { syntax_set.find_syntax_by_first_line(&state.content[0]) } else { None }).unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, &theme_set.themes["base16-mocha.dark"]);
    let full_content = state.content.join("\n");
    let mut lines = Vec::new(); let mut line_num = 0;
    for line in LinesWithEndings::from(&full_content) {
        if line_num >= state.scroll_y && line_num < state.scroll_y + height {
            let ranges = h.highlight_line(line, syntax_set).unwrap_or_default();
            let mut spans = vec![Span::styled(format!("{:5} ", line_num + 1), Style::default().fg(Color::Rgb(60, 60, 70)))];
            for (style, text) in ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let mut rs = Style::default().fg(fg); if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) { rs = rs.add_modifier(Modifier::BOLD); }
                let text_val = text.replace("\n", "").replace("\r", "");
                let current_len = spans_len(&spans) - 6;
                let start_x = state.scroll_x.saturating_sub(current_len);
                if start_x < text_val.len() {
                    let end_x = std::cmp::min(text_val.len(), start_x + width.saturating_sub(current_len));
                    if end_x > start_x { spans.push(Span::styled(text_val[start_x..end_x].to_string(), rs)); }
                }
            }
            lines.push(Line::from(spans));
        } else if line_num < state.scroll_y + height { let _ = h.highlight_line(line, syntax_set); }
        line_num += 1; if line_num >= state.scroll_y + height { break; }
    }
    while lines.len() < height { lines.push(Line::from(vec![Span::styled("     ~", Style::default().fg(Color::Rgb(40, 40, 50)))])); }
    f.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::Rgb(50, 50, 60))).title(format!(" 󰷈 EDITOR: {} ", state.file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".to_string())))), area);
    let cur_y = area.y + 1 + (state.cursor_y.saturating_sub(state.scroll_y)) as u16;
    let cur_x = area.x + 1 + 6 + (state.cursor_x.saturating_sub(state.scroll_x)) as u16;
    if cur_y >= area.y + 1 && cur_y < area.y + area.height - 1 { f.set_cursor(cur_x, cur_y); }
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let mode_bg = match &app.tabs[app.active_tab].state { TabState::FileBrowser(_) => Color::Rgb(0, 110, 220), TabState::Editor(_) => Color::Rgb(255, 140, 0) };
    let mode_str = match &app.tabs[app.active_tab].state { TabState::FileBrowser(_) => "  󰙅 EXPLORER  ", TabState::Editor(_) => "   󰷈 EDITOR   " };
    let line = Line::from(vec![
        Span::styled(mode_str, Style::default().bg(mode_bg).fg(Color::Black).bold()),
        Span::styled("", Style::default().fg(mode_bg).bg(Color::Rgb(30, 30, 35))),
        Span::styled(format!("  {} ", app.status_message), Style::default().bg(Color::Rgb(30, 30, 35)).fg(Color::White)),
        Span::styled("", Style::default().fg(Color::Rgb(30, 30, 35))),
    ]);
    f.render_widget(Paragraph::new(line).bg(Color::Rgb(15, 15, 20)), area);
}

fn draw_progress_bar(f: &mut Frame, task: &BackgroundTask, anim_frame: usize, area: Rect) {
    let area = centered_rect(65, 25, area);
    f.render_widget(Clear, area);
    let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinners[(anim_frame / 2) % spinners.len()];
    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(format!(" {} {} ", spinner, task.name)).border_style(Style::default().fg(Color::Rgb(0, 255, 150)));
    f.render_widget(block, area);
    let inner = Layout::default().direction(Direction::Vertical).margin(2).constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(area);
    f.render_widget(Paragraph::new(format!("  󰁔 {}", task.current_item)).style(Style::default().fg(Color::Rgb(180, 180, 180))), inner[0]);
    let width = inner[1].width as usize - 4;
    let filled = (task.progress * width as f64) as usize;
    let bar = format!("  [{}{}]", "█".repeat(filled), " ".repeat(width - filled));
    f.render_widget(Paragraph::new(bar).style(Style::default().fg(Color::Rgb(0, 255, 150))), inner[1]);
    let eta = if task.progress > 0.01 { format!("{}s", (task.start_time.elapsed().as_secs_f64() / task.progress - task.start_time.elapsed().as_secs_f64()) as u32) } else { "..." .to_string() };
    f.render_widget(Paragraph::new(format!("{}%  |  Remaining approx: {}  ", (task.progress * 100.0) as u32, eta)).alignment(Alignment::Right).fg(Color::Rgb(150, 150, 150)), inner[2]);
}

fn draw_error_popup(f: &mut Frame, err_msg: &str, area: Rect) {
    let area = centered_rect(55, 25, area);
    f.render_widget(Clear, area);
    f.render_widget(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" 󰅚 ERROR ").border_style(Style::default().fg(Color::Rgb(255, 70, 70))), area);
    let inner = Layout::default().direction(Direction::Vertical).margin(2).constraints([Constraint::Min(1), Constraint::Length(1)]).split(area);
    f.render_widget(Paragraph::new(err_msg).white().wrap(ratatui::widgets::Wrap { trim: true }), inner[0]);
    f.render_widget(Paragraph::new("[ Press ENTER ]").alignment(Alignment::Center).dark_gray().italic(), inner[1]);
}

fn draw_preview(f: &mut Frame, item: &FileItem, syntax_set: &SyntaxSet, theme_set: &ThemeSet, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::Rgb(0, 120, 215))).title(" 󰈈 PREVIEW ");
    if item.is_dir {
        f.render_widget(Paragraph::new(vec![Line::from(""), Line::from(vec![Span::styled("  📁 FOLDER", Style::default().cyan().bold())]), Line::from(format!("  Name: {}", item.name)), Line::from(format!("  Size: {}", crate::utils::format_size(item.size)))]).block(block), area);
        return;
    }
    if item.size > 1024 * 500 { f.render_widget(Paragraph::new("\n  File is too large for preview.").block(block).gray(), area); return; }
    let content = match std::fs::read_to_string(&item.path) {
        Ok(c) => c.lines().take(50).collect::<Vec<&str>>().join("\n"),
        Err(_) => { f.render_widget(Paragraph::new("\n  (Binary file or missing permissions)").block(block).dark_gray(), area); return; }
    };
    let syntax = item.path.extension().and_then(|ext| syntax_set.find_syntax_by_extension(ext.to_str().unwrap_or(""))).or_else(|| { let lines: Vec<&str> = content.lines().collect(); if !lines.is_empty() { syntax_set.find_syntax_by_first_line(lines[0]) } else { None } }).unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, &theme_set.themes["base16-mocha.dark"]);
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(&content) {
        let ranges = h.highlight_line(line, syntax_set).unwrap_or_default();
        let mut spans = Vec::new();
        for (style, text) in ranges {
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            let mut rs = Style::default().fg(fg); if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) { rs = rs.add_modifier(Modifier::BOLD); }
            spans.push(Span::styled(text.replace("\n", "").replace("\r", ""), rs));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)]).split(r);
    Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)]).split(layout[1])[1]
}

fn spans_len(spans: &[Span]) -> usize { spans.iter().map(|s| s.content.len()).sum() }
