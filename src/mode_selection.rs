use crate::config::{MergeResult, TrainConfig};
use crate::parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::path::Path;

/// 展开路径中的 `~` 为用户主目录
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{}", home.to_string_lossy(), &path[2..]);
        }
    }
    path.to_string()
}

pub enum LaunchMode {
    OpenConfig(TrainConfig),
    ExtractFromScript {
        config: TrainConfig,
        merge_result: Option<MergeResult>,
    },
}

pub struct ModeSelector {
    // 主菜单状态
    selected: usize,
    // 路径输入
    script_path: String,
    config_path: String,
    editing_script: bool,
    editing_config: bool,
    // 错误信息
    error_msg: String,
    // 退出标志
    should_quit: bool,
    // 继承配置对话框
    show_inherit_dialog: bool,
    inherit_selected: usize,
    inherit_config_path: String,
    editing_inherit_path: bool,
    // 合并结果展示
    show_merge_result: bool,
    pending_merge_result: Option<MergeResult>,
    pending_new_config: Option<TrainConfig>,
}

impl ModeSelector {
    pub fn new() -> Self {
        Self {
            selected: 0,
            script_path: "train.py".to_string(),
            config_path: "config.json".to_string(),
            editing_script: false,
            editing_config: false,
            error_msg: String::new(),
            should_quit: false,
            show_inherit_dialog: false,
            inherit_selected: 0,
            inherit_config_path: "config.json".to_string(),
            editing_inherit_path: false,
            show_merge_result: false,
            pending_merge_result: None,
            pending_new_config: None,
        }
    }

    pub fn run(&mut self) -> io::Result<Option<LaunchMode>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                let mode = self.handle_key(key);
                if self.should_quit {
                    break Ok(None);
                }
                if let Some(launch_mode) = mode {
                    break Ok(Some(launch_mode));
                }
            }
        };

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<LaunchMode> {
        // 优先处理合并结果展示
        if self.show_merge_result {
            return self.handle_merge_result_key(key);
        }

        // 处理继承配置对话框
        if self.show_inherit_dialog {
            return self.handle_inherit_dialog_key(key);
        }

        // 处理路径编辑
        if self.editing_script {
            self.handle_editing_script_key(key);
            return None;
        }
        if self.editing_config {
            self.handle_editing_config_key(key);
            return None;
        }
        if self.editing_inherit_path {
            self.handle_editing_inherit_path_key(key);
            return None;
        }

        // 处理主菜单
        self.handle_main_menu_key(key)
    }

    fn handle_main_menu_key(&mut self, key: KeyEvent) -> Option<LaunchMode> {
        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected < 2 {
                    self.selected += 1;
                }
            }
            KeyCode::Enter => {
                return match self.selected {
                    0 => self.try_open_config(),
                    1 => self.try_extract_from_script(),
                    2 => {
                        self.should_quit = true;
                        None
                    }
                    _ => None,
                };
            }
            KeyCode::Char('p') => {
                self.editing_script = true;
            }
            KeyCode::Char('c') => {
                self.editing_config = true;
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            _ => {}
        }
        None
    }

    fn handle_inherit_dialog_key(&mut self, key: KeyEvent) -> Option<LaunchMode> {
        if self.editing_inherit_path {
            match key.code {
                KeyCode::Enter => {
                    self.editing_inherit_path = false;
                }
                KeyCode::Esc => {
                    self.editing_inherit_path = false;
                }
                KeyCode::Char(c) => {
                    self.inherit_config_path.push(c);
                }
                KeyCode::Backspace => {
                    self.inherit_config_path.pop();
                }
                _ => {}
            }
            return None;
        }

        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.inherit_selected = if self.inherit_selected == 0 { 1 } else { 0 };
            }
            KeyCode::Enter => {
                if self.inherit_selected == 0 {
                    // 选择继承配置
                    return self.try_merge_config();
                } else {
                    // 不继承，使用新配置
                    let config = self.pending_new_config.take();
                    self.show_inherit_dialog = false;
                    return config.map(|c| LaunchMode::ExtractFromScript {
                        config: c,
                        merge_result: None,
                    });
                }
            }
            KeyCode::Char('e') => {
                self.editing_inherit_path = true;
            }
            KeyCode::Esc => {
                // 取消，返回主菜单
                self.show_inherit_dialog = false;
                self.pending_new_config = None;
            }
            _ => {}
        }
        None
    }

    fn handle_merge_result_key(&mut self, key: KeyEvent) -> Option<LaunchMode> {
        match key.code {
            KeyCode::Enter => {
                let merge_result = self.pending_merge_result.take();
                self.show_merge_result = false;
                return merge_result.map(|m| {
                    let config = m.config.clone();
                    LaunchMode::ExtractFromScript {
                        config,
                        merge_result: Some(m),
                    }
                });
            }
            KeyCode::Esc => {
                self.show_merge_result = false;
                self.pending_merge_result = None;
                self.pending_new_config = None;
            }
            _ => {}
        }
        None
    }

    fn handle_editing_script_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.editing_script = false;
            }
            KeyCode::Char(c) => {
                self.script_path.push(c);
            }
            KeyCode::Backspace => {
                self.script_path.pop();
            }
            _ => {}
        }
    }

    fn handle_editing_config_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.editing_config = false;
            }
            KeyCode::Char(c) => {
                self.config_path.push(c);
            }
            KeyCode::Backspace => {
                self.config_path.pop();
            }
            _ => {}
        }
    }

    fn handle_editing_inherit_path_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.editing_inherit_path = false;
            }
            KeyCode::Char(c) => {
                self.inherit_config_path.push(c);
            }
            KeyCode::Backspace => {
                self.inherit_config_path.pop();
            }
            _ => {}
        }
    }

    fn try_open_config(&mut self) -> Option<LaunchMode> {
        let expanded_path = expand_tilde(&self.config_path);
        let path = Path::new(&expanded_path);
        if !path.exists() {
            self.error_msg = format!("配置文件不存在: {}", self.config_path);
            return None;
        }

        match TrainConfig::default().load(&expanded_path) {
            Ok(config) => Some(LaunchMode::OpenConfig(config)),
            Err(e) => {
                self.error_msg = format!("加载配置失败: {}", e);
                None
            }
        }
    }

    fn try_extract_from_script(&mut self) -> Option<LaunchMode> {
        let expanded_path = expand_tilde(&self.script_path);
        let path = Path::new(&expanded_path);
        if !path.exists() {
            self.error_msg = format!("脚本文件不存在: {}", self.script_path);
            return None;
        }

        let code = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                self.error_msg = format!("读取脚本失败: {}", e);
                return None;
            }
        };

        let params = parser::extract_params(&code);
        if params.is_empty() {
            self.error_msg = "未能从脚本中提取任何参数".to_string();
            return None;
        }

        let config = TrainConfig::from_parser_params(params);

        // 显示继承配置对话框
        self.pending_new_config = Some(config);
        self.show_inherit_dialog = true;
        self.inherit_selected = 0;
        self.inherit_config_path = self.config_path.clone();
        None
    }

    fn try_merge_config(&mut self) -> Option<LaunchMode> {
        let expanded_path = expand_tilde(&self.inherit_config_path);
        let path = Path::new(&expanded_path);
        if !path.exists() {
            self.error_msg = format!("继承配置文件不存在: {}", self.inherit_config_path);
            self.show_inherit_dialog = false;
            return None;
        }

        let old_config = match TrainConfig::default().load(&expanded_path) {
            Ok(c) => c,
            Err(e) => {
                self.error_msg = format!("加载继承配置失败: {}", e);
                self.show_inherit_dialog = false;
                return None;
            }
        };

        let new_config = match self.pending_new_config.take() {
            Some(c) => c,
            None => {
                self.show_inherit_dialog = false;
                return None;
            }
        };

        let merge_result = old_config.merge_with_inheritance(new_config);

        // 显示合并结果
        self.pending_merge_result = Some(merge_result);
        self.show_merge_result = true;
        self.show_inherit_dialog = false;
        None
    }

    fn ui(&self, f: &mut Frame) {
        let rect = f.area();

        if self.show_merge_result {
            self.ui_merge_result(f, rect);
            return;
        }

        if self.show_inherit_dialog {
            self.ui_inherit_dialog(f, rect);
            return;
        }

        self.ui_main_menu(f, rect);
    }

    fn ui_main_menu(&self, f: &mut Frame, rect: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(rect);

        // Title
        let title = Paragraph::new("ML Launcher - 选择启动模式")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Options
        let options: Vec<ListItem> = vec![
            ListItem::new("1. 打开已有配置文件"),
            ListItem::new("2. 从 Python 脚本提取参数"),
            ListItem::new("3. 退出"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            if i == self.selected {
                item.style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                item.style(Style::default())
            }
        })
        .collect();

        let list = List::new(options).block(
            Block::default()
                .title("请选择模式 (↑↓ 选择, Enter 确认)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, chunks[1]);

        // Paths
        let script_text = if self.editing_script {
            format!("> {}_", self.script_path)
        } else {
            format!("{} (按 'p' 编辑)", self.script_path)
        };
        let config_text = if self.editing_config {
            format!("> {}_", self.config_path)
        } else {
            format!("{} (按 'c' 编辑)", self.config_path)
        };

        let paths = Paragraph::new(format!(
            "脚本: {} | 配置: {}",
            script_text, config_text
        ))
        .block(
            Block::default()
                .title("路径设置")
                .borders(Borders::ALL)
                .style(Style::default().fg(if self.editing_script || self.editing_config {
                    Color::Yellow
                } else {
                    Color::Gray
                })),
        );
        f.render_widget(paths, chunks[2]);

        // Error or help
        let help_text = if !self.error_msg.is_empty() {
            format!("错误: {}", self.error_msg)
        } else {
            "按 'q' 退出".to_string()
        };
        let help = Paragraph::new(help_text).style(
            Style::default().fg(if self.error_msg.is_empty() {
                Color::DarkGray
            } else {
                Color::Red
            }),
        );
        f.render_widget(help, chunks[3]);
    }

    fn ui_inherit_dialog(&self, f: &mut Frame, rect: Rect) {
        // 先渲染主菜单作为背景
        self.ui_main_menu(f, rect);

        // 弹出对话框
        let popup_area = centered_rect(70, 40, rect);
        f.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(3),
            ])
            .split(popup_area);

        // 标题
        let title = Paragraph::new("是否继承已有配置的设置？")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // 选项
        let path_text = if self.editing_inherit_path {
            format!("> {}_", self.inherit_config_path)
        } else {
            format!("{} (按 'e' 编辑)", self.inherit_config_path)
        };

        let options: Vec<ListItem> = vec![
            ListItem::new(format!("是，继承配置: {}", path_text)),
            ListItem::new("否，使用全新配置".to_string()),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            if i == self.inherit_selected {
                item.style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                item.style(Style::default())
            }
        })
        .collect();

        let list = List::new(options).block(
            Block::default()
                .title("选择 (↑↓ 切换, Enter 确认, Esc 取消)")
                .borders(Borders::ALL),
        );
        f.render_widget(list, chunks[1]);

        // 提示
        let hint = Paragraph::new("继承功能会将旧配置中的参数值复制到新配置中").style(
            Style::default().fg(Color::DarkGray),
        );
        f.render_widget(hint, chunks[2]);
    }

    fn ui_merge_result(&self, f: &mut Frame, rect: Rect) {
        // 背景
        let block = Block::default()
            .title("配置合并完成")
            .borders(Borders::ALL);
        f.render_widget(block, rect);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Min(1),
            ])
            .margin(1)
            .split(rect);

        // 标题
        let title = Paragraph::new("配置合并结果")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        f.render_widget(title, chunks[0]);

        if let Some(ref result) = self.pending_merge_result {
            // 继承的参数
            let inherited = Paragraph::new(format!(
                "✓ 继承了 {} 个参数的设置\n  {}",
                result.inherited_params.len(),
                result.inherited_params.join(", ")
            ))
            .style(Style::default().fg(Color::Green));
            f.render_widget(inherited, chunks[1]);

            // 新增的参数
            let new_params = Paragraph::new(format!(
                "✓ 新增 {} 个参数 (使用默认值)\n  {}",
                result.new_params.len(),
                result.new_params.join(", ")
            ))
            .style(Style::default().fg(Color::Blue));
            f.render_widget(new_params, chunks[2]);

            // 弃用的参数
            if result.deprecated_params.is_empty() {
                let deprecated = Paragraph::new("✓ 没有弃用的参数")
                    .style(Style::default().fg(Color::Gray));
                f.render_widget(deprecated, chunks[3]);
            } else {
                let deprecated_names: Vec<_> =
                    result.deprecated_params.iter().map(|p| p.name.as_str()).collect();
                let deprecated = Paragraph::new(format!(
                    "⚠ 保留 {} 个旧参数 (在新脚本中不存在)\n  {}",
                    result.deprecated_params.len(),
                    deprecated_names.join(", ")
                ))
                .style(Style::default().fg(Color::Yellow));
                f.render_widget(deprecated, chunks[3]);
            }
        }

        // 提示
        let hint = Paragraph::new("按 Enter 继续编辑，按 Esc 取消")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(hint, chunks[4]);
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
