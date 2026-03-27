use crate::config::{MergeResult, ParamField, ParamType, TrainConfig};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::io;
use std::process::Command;

pub struct App {
    config: TrainConfig,
    command_output: String,
    running: bool,
    selected_tab: usize,
    selected_param: usize,  // Parameters tab 选中的参数
    selected_filter: usize, // Filter tab 选中的参数
    editing: bool,
    edit_buffer: String,
    edit_warning: Option<String>, // 编辑时的验证警告
    input_mode: InputMode,
    save_path: String,
    load_path: String,
    show_save_dialog: bool,
    show_load_dialog: bool,
    deprecated_params: Vec<ParamField>, // 弃用的参数列表（来自合并）
}

#[derive(Default, Clone, PartialEq)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
    SaveDialog,
    LoadDialog,
}

impl App {
    pub fn new() -> Self {
        Self {
            config: TrainConfig::default(),
            command_output: String::new(),
            running: false,
            selected_tab: 0,
            selected_param: 0,
            selected_filter: 0,
            editing: false,
            edit_buffer: String::new(),
            edit_warning: None,
            input_mode: InputMode::Normal,
            save_path: "config.json".to_string(),
            load_path: "config.json".to_string(),
            show_save_dialog: false,
            show_load_dialog: false,
            deprecated_params: Vec::new(),
        }
    }

    pub fn with_config(config: TrainConfig, merge_result: Option<MergeResult>) -> Self {
        let deprecated_params = merge_result
            .as_ref()
            .map(|m| m.deprecated_params.clone())
            .unwrap_or_default();

        Self {
            config,
            command_output: String::new(),
            running: false,
            selected_tab: 0,
            selected_param: 0,
            selected_filter: 0,
            editing: false,
            edit_buffer: String::new(),
            edit_warning: None,
            input_mode: InputMode::Normal,
            save_path: "config.json".to_string(),
            load_path: "config.json".to_string(),
            show_save_dialog: false,
            show_load_dialog: false,
            deprecated_params,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.running = true;
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        // main loop
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            }

            if !self.running {
                break;
            }
        }

        // restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Editing => self.handle_editing_key(key),
            InputMode::SaveDialog => self.handle_save_dialog_key(key),
            InputMode::LoadDialog => self.handle_load_dialog_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.selected_tab = (self.selected_tab + 1) % 3;
                self.selected_param = 0;
                self.selected_filter = 0;
            }
            KeyCode::Up => {
                match self.selected_tab {
                    0 => {
                        // Parameters tab - 只显示 use_default=false 的参数
                        if self.selected_param > 0 {
                            self.selected_param -= 1;
                        }
                    }
                    1 => {
                        // Filter tab - 显示所有参数
                        if self.selected_filter > 0 {
                            self.selected_filter -= 1;
                        }
                    }
                    _ => {
                        // Actions tab
                        if self.selected_param > 0 {
                            self.selected_param -= 1;
                        }
                    }
                }
            }
            KeyCode::Down => {
                match self.selected_tab {
                    0 => {
                        // Parameters tab
                        let active_count =
                            self.config.params.iter().filter(|p| !p.use_default).count();
                        let max = active_count.saturating_sub(1);
                        if self.selected_param < max {
                            self.selected_param += 1;
                        }
                    }
                    1 => {
                        // Filter tab
                        let max = self.config.params.len().saturating_sub(1);
                        if self.selected_filter < max {
                            self.selected_filter += 1;
                        }
                    }
                    _ => {
                        // Actions tab
                        if self.selected_param < 7 {
                            self.selected_param += 1;
                        }
                    }
                }
            }
            KeyCode::Enter => {
                if self.selected_tab == 2 {
                    // Actions tab
                    match self.selected_param {
                        0 => self.generate_command(),
                        1 => self.run_training(),
                        2 => self.show_save_dialog(),
                        3 => self.show_load_dialog(),
                        4 => self.add_param(),
                        5 => self.delete_param(),
                        6 => self.start_editing(),
                        _ => {}
                    }
                }
            }
            KeyCode::Char('g') => self.generate_command(),
            KeyCode::Char('s') => self.show_save_dialog(),
            KeyCode::Char('l') => self.show_load_dialog(),
            KeyCode::Char('a') => self.add_param(),
            KeyCode::Char('d') => self.delete_param(),
            KeyCode::Char('e') => self.start_editing(),
            KeyCode::Char('t') => {
                if self.selected_tab == 1 {
                    // 只在 Filter tab 中有效
                    self.toggle_use_default();
                }
            }
            KeyCode::Char('q') => {
                self.running = false;
            }
            _ => {}
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Some(idx) = self.get_active_param_index() {
                    if let Some(param) = self.config.params.get_mut(idx) {
                        param.value = self.edit_buffer.clone();
                    }
                }
                self.editing = false;
                self.edit_warning = None;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.editing = false;
                self.edit_warning = None;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
                self.validate_edit_buffer();
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
                self.validate_edit_buffer();
            }
            _ => {}
        }
    }

    /// 验证当前编辑缓冲区中的值
    fn validate_edit_buffer(&mut self) {
        if let Some(idx) = self.get_active_param_index() {
            if let Some(param) = self.config.params.get(idx) {
                // 创建临时参数来验证
                let temp_param = ParamField {
                    name: param.name.clone(),
                    value: self.edit_buffer.clone(),
                    param_type: param.param_type.clone(),
                    description: param.description.clone(),
                    use_default: param.use_default,
                };
                self.edit_warning = temp_param.validate_value().map(|w| w.message);
            }
        }
    }

    fn handle_load_dialog_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                // if let Err(e) = self.config.load(&self.load_path) {
                //     self.command_output = format!("Error: {}", e);
                // } else {
                //     self.command_output = format!("Load from {}", self.load_path);
                // }
                match self.config.load(&self.load_path) {
                    Ok(config) => {
                        self.config = config;
                        self.command_output = format!("Load from {}", self.load_path);
                    }
                    Err(e) => {
                        self.command_output = format!("Error: {}", e);
                    }
                }
                self.show_load_dialog = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.show_load_dialog = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.load_path.push(c);
            }
            KeyCode::Backspace => {
                self.load_path.pop();
            }
            _ => {}
        }
    }

    fn handle_save_dialog_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Err(e) = self.config.save(&self.save_path) {
                    self.command_output = format!("Error: {}", e);
                } else {
                    self.command_output = format!("Saved to {}", self.save_path);
                }
                self.show_save_dialog = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.show_save_dialog = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.save_path.push(c);
            }
            KeyCode::Backspace => {
                self.save_path.pop();
            }
            _ => {}
        }
    }

    fn generate_command(&mut self) {
        self.command_output = self.config.build_command();
    }

    fn show_save_dialog(&mut self) {
        self.show_save_dialog = true;
        self.input_mode = InputMode::SaveDialog;
    }

    fn show_load_dialog(&mut self) {
        self.show_load_dialog = true;
        self.input_mode = InputMode::LoadDialog;
    }

    fn add_param(&mut self) {
        self.config.params.push(ParamField::new(
            "new_param",
            "",
            ParamType::String,
            "New parameter",
        ));
    }

    fn delete_param(&mut self) {
        // 在 Parameters tab 中删除 use_default=false 的参数
        if self.selected_tab == 0 {
            let active_indices: Vec<usize> = self
                .config
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.use_default)
                .map(|(i, _)| i)
                .collect();

            if let Some(&idx) = active_indices.get(self.selected_param) {
                if self.config.params.len() > 1 {
                    self.config.params.remove(idx);
                    let active_count = self.config.params.iter().filter(|p| !p.use_default).count();
                    if self.selected_param >= active_count {
                        self.selected_param = active_count.saturating_sub(1);
                    }
                }
            }
        }
    }

    fn start_editing(&mut self) {
        if self.selected_tab == 0 {
            // 找到第 selected_param 个 use_default=false 的参数
            let active_indices: Vec<usize> = self
                .config
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.use_default)
                .map(|(i, _)| i)
                .collect();

            if let Some(&idx) = active_indices.get(self.selected_param) {
                self.editing = true;
                self.input_mode = InputMode::Editing;
                self.edit_warning = None;
                if let Some(param) = self.config.params.get(idx) {
                    self.edit_buffer = param.value.clone();
                    // 初始验证
                    self.validate_edit_buffer();
                }
            }
        }
    }

    fn toggle_use_default(&mut self) {
        // 在 Filter tab 中切换 use_default
        if self.selected_tab == 1 {
            if let Some(param) = self.config.params.get_mut(self.selected_filter) {
                param.use_default = !param.use_default;
            }
        }
    }

    // 获取当前在 Parameters tab 中选中的实际参数索引
    fn get_active_param_index(&self) -> Option<usize> {
        let active_indices: Vec<usize> = self
            .config
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.use_default)
            .map(|(i, _)| i)
            .collect();
        active_indices.get(self.selected_param).copied()
    }

    fn ui(&mut self, f: &mut Frame) {
        let rect = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(5),
                Constraint::Length(3),
            ])
            .split(rect);

        // Tabs - 三个标签页
        let tabs = ["Parameters", "Filter", "Actions"];
        let titles: Vec<Line> = tabs.iter().map(|t| Line::from(*t)).collect();
        let tabs_widget = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("ML Launcher"))
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(self.selected_tab);
        f.render_widget(tabs_widget, chunks[0]);

        // Main content
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        match self.selected_tab {
            0 => self.ui_parameters(f, main_chunks[0], main_chunks[1]),
            1 => self.ui_filter(f, main_chunks[0], main_chunks[1]),
            _ => self.ui_actions(f, main_chunks[0], main_chunks[1]),
        }

        // Command output
        if self.selected_tab != 2 {
            let output = Paragraph::new(self.command_output.as_str())
                .block(Block::default().title("Output").borders(Borders::ALL));
            f.render_widget(output, chunks[2]);
        }

        // Help
        let help = match self.selected_tab {
            0 => "Tab: Switch | ↑↓: Nav | e: Edit | d: Del | q: Quit",
            1 => "Tab: Switch | ↑↓: Nav | t: Toggle Default | q: Quit",
            _ => {
                "Tab: Switch | ↑↓: Nav | Enter: Run | g: Gen | s: Save | l: Load | a: Add | q: Quit"
            }
        };
        let help_widget = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true });
        f.render_widget(help_widget, chunks[3]);

        // Popups
        if self.editing {
            self.render_edit_popup(f, rect);
        }
        if self.show_load_dialog {
            self.render_load_popup(f, rect);
        }
        if self.show_save_dialog {
            self.render_save_popup(f, rect);
        }
    }

    fn ui_parameters(&self, f: &mut Frame, left: Rect, right: Rect) {
        // 只显示 use_default=false 的参数
        let active_params: Vec<(usize, &ParamField)> = self
            .config
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.use_default)
            .collect();

        let param_items: Vec<ListItem> = active_params
            .iter()
            .enumerate()
            .map(|(display_idx, (_, p))| {
                let style = if display_idx == self.selected_param {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{} = {} ({:?})", p.name, p.value, p.param_type)).style(style)
            })
            .collect();

        // 使用 ListState 管理滚动
        let mut state = ListState::default();
        state.select(Some(self.selected_param));

        let param_list = List::new(param_items)
            .block(
                Block::default()
                    .title("Parameters (需修改的参数)")
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">");
        f.render_stateful_widget(param_list, left, &mut state);

        // Description
        if let Some((_, param)) = active_params.get(self.selected_param) {
            let desc = Paragraph::new(format!(
                "Name: {}\nValue: {}\nType: {:?}\nDescription: {}",
                param.name, param.value, param.param_type, param.description
            ))
            .block(Block::default().title("Detail").borders(Borders::ALL));
            f.render_widget(desc, right);
        }
    }

    fn ui_filter(&self, f: &mut Frame, left: Rect, right: Rect) {
        // 显示所有参数，可以切换 use_default
        let param_items: Vec<ListItem> = self
            .config
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let style = if i == self.selected_filter {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let status = if p.use_default {
                    "✓ 默认"
                } else {
                    "✗ 修改"
                };
                ListItem::new(format!("[{}] {} = {}", status, p.name, p.value)).style(style)
            })
            .collect();

        // 使用 ListState 管理滚动
        let mut state = ListState::default();
        state.select(Some(self.selected_filter));

        let param_list = List::new(param_items)
            .block(
                Block::default()
                    .title("Filter - 选择需要修改的参数 [t: 切换]")
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">");
        f.render_stateful_widget(param_list, left, &mut state);

        // Description
        if let Some(param) = self.config.params.get(self.selected_filter) {
            let status_text = if param.use_default {
                "✓ 使用脚本默认值"
            } else {
                "✗ 需要手动设置"
            };
            let desc = Paragraph::new(format!(
                "Name: {}\nValue: {}\nType: {:?}\nDescription: {}\n状态: {}",
                param.name, param.value, param.param_type, param.description, status_text
            ))
            .block(Block::default().title("Detail").borders(Borders::ALL));
            f.render_widget(desc, right);
        }
    }

    fn ui_actions(&self, f: &mut Frame, left: Rect, right: Rect) {
        let action_items: Vec<ListItem> = vec![
            ListItem::new("Generate Command [g]"),
            ListItem::new("Run Training [Enter]"),
            ListItem::new("Save Config [s]"),
            ListItem::new("Load Config [l]"),
            ListItem::new("Add Param [a]"),
            ListItem::new("Del Param [d]"),
            ListItem::new("Edit Param [e]"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            if i == self.selected_param {
                item.style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                item
            }
        })
        .collect();

        let action_list =
            List::new(action_items).block(Block::default().title("Actions").borders(Borders::ALL));
        f.render_widget(action_list, left);

        let output = Paragraph::new(self.command_output.as_str())
            .block(Block::default().title("Output").borders(Borders::ALL));
        f.render_widget(output, right);
    }

    fn render_edit_popup(&self, f: &mut Frame, rect: Rect) {
        // 如果有警告，弹窗需要更大
        let (width, height) = if self.edit_warning.is_some() {
            (70, 35)
        } else {
            (60, 20)
        };
        let popup_area = centered_rect(width, height, rect);
        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if self.edit_warning.is_some() {
                vec![Constraint::Min(3), Constraint::Length(4)]
            } else {
                vec![Constraint::Percentage(100)]
            })
            .split(popup_area);

        // 编辑框
        let edit_text = format!("{}_", self.edit_buffer);
        let popup = Paragraph::new(edit_text).block(
            Block::default()
                .title("Edit Parameter Value (Enter=save, Esc=cancel)")
                .borders(Borders::ALL),
        );
        f.render_widget(popup, chunks[0]);

        // 警告信息
        if let Some(ref warning) = self.edit_warning {
            let warning_widget =
                Paragraph::new(format!("⚠ {}\n  (仍可保存，但可能导致训练错误)", warning))
                    .style(Style::default().fg(Color::Yellow))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Yellow)),
                    );
            f.render_widget(warning_widget, chunks[1]);
        }
    }

    fn render_load_popup(&self, f: &mut Frame, rect: Rect) {
        let popup_area = centered_rect(60, 20, rect);
        f.render_widget(Clear, popup_area);
        let popup = Paragraph::new(self.load_path.as_str()).block(
            Block::default()
                .title("Load from file (Enter=load, Esc=cancel)")
                .borders(Borders::ALL),
        );
        f.render_widget(popup, popup_area);
    }

    fn render_save_popup(&self, f: &mut Frame, rect: Rect) {
        let popup_area = centered_rect(60, 20, rect);
        f.render_widget(Clear, popup_area);
        let popup = Paragraph::new(self.save_path.as_str()).block(
            Block::default()
                .title("Save to file (Enter=save, Esc=cancel)")
                .borders(Borders::ALL),
        );
        f.render_widget(popup, popup_area);
    }

    fn run_training(&mut self) {
        let cmd = self.config.build_command();
        self.command_output = format!("Running: {}\n", cmd);

        // Build args
        let mut args: Vec<String> = vec![];
        for param in &self.config.params {
            if let Some(arg) = param.to_arg() {
                for part in arg.split_whitespace() {
                    args.push(String::from(part));
                }
            }
        }

        match Command::new(&self.config.python_path)
            .arg(&self.config.script_path)
            .args(&args)
            .output()
        {
            Ok(output) => {
                self.command_output
                    .push_str(&String::from_utf8_lossy(&output.stdout));
                if !output.stderr.is_empty() {
                    self.command_output
                        .push_str(&String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                self.command_output.push_str(&format!("Error: {}", e));
            }
        }
    }
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
