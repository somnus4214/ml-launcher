use crate::config::{ParamField, ParamType, TrainConfig};
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
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::io;
use std::process::Command;

pub struct App {
    config: TrainConfig,
    command_output: String,
    running: bool,
    selected_tab: usize,
    selected_param: usize,
    editing: bool,
    edit_buffer: String,
    input_mode: InputMode,
    save_path: String,
    load_path: String,
    show_save_dialog: bool,
    show_load_dialog: bool,
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
            editing: false,
            edit_buffer: String::new(),
            input_mode: InputMode::Normal,
            save_path: "config.json".to_string(),
            load_path: "config.json".to_string(),
            show_save_dialog: false,
            show_load_dialog: false,
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
                self.selected_tab = (self.selected_tab + 1) % 2;
                self.selected_param = 0;
            }
            KeyCode::Up => {
                if self.selected_param > 0 {
                    self.selected_param -= 1;
                }
            }
            KeyCode::Down => {
                let max = if self.selected_tab == 0 {
                    self.config.params.len().saturating_sub(1)
                } else {
                    6
                };
                if self.selected_param < max {
                    self.selected_param += 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_tab == 1 {
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
            KeyCode::Char('q') => {
                self.running = false;
            }
            _ => {}
        }
    }

    fn handle_editing_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Some(param) = self.config.params.get_mut(self.selected_param) {
                    param.value = self.edit_buffer.clone();
                }
                self.editing = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.editing = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            _ => {}
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
        if self.config.params.len() > 1 {
            self.config.params.remove(self.selected_param);
            if self.selected_param >= self.config.params.len() {
                self.selected_param = self.config.params.len().saturating_sub(1);
            }
        }
    }

    fn start_editing(&mut self) {
        if self.selected_tab == 0 {
            self.editing = true;
            self.input_mode = InputMode::Editing;
            if let Some(param) = self.config.params.get(self.selected_param) {
                self.edit_buffer = param.value.clone();
            }
        }
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

        // Tabs
        let tabs = ["Parameters", "Actions"];
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

        if self.selected_tab == 0 {
            // Parameter list
            let param_items: Vec<ListItem> = self
                .config
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let style = if i == self.selected_param {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!("{} = {} ({:?})", p.name, p.value, p.param_type))
                        .style(style)
                })
                .collect();

            let param_list = List::new(param_items)
                .block(Block::default().title("Parameters").borders(Borders::ALL))
                .highlight_style(Style::default().fg(Color::Cyan));
            f.render_widget(param_list, main_chunks[0]);

            // Description
            if let Some(param) = self.config.params.get(self.selected_param) {
                let desc = Paragraph::new(format!(
                    "Name: {}\nValue: {}\nType: {:?}\nDescription: {}",
                    param.name, param.value, param.param_type, param.description
                ))
                .block(Block::default().title("Detail").borders(Borders::ALL));
                f.render_widget(desc, main_chunks[1]);
            }
        } else {
            // Action buttons
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

            let action_list = List::new(action_items)
                .block(Block::default().title("Actions").borders(Borders::ALL));
            f.render_widget(action_list, main_chunks[0]);

            // Output
            let output = Paragraph::new(self.command_output.as_str())
                .block(Block::default().title("Output").borders(Borders::ALL));
            f.render_widget(output, main_chunks[1]);
        }

        // Command output (for Parameters tab)
        if self.selected_tab == 0 {
            let output = Paragraph::new(self.command_output.as_str())
                .block(Block::default().title("Output").borders(Borders::ALL));
            f.render_widget(output, chunks[2]);
        }

        // Help
        let help = Paragraph::new(
            "Tab: Switch | ↑↓: Navigate | Enter: Run | g: Generate | s: Save | l: Load | a: Add | d: Delete | e: Edit | q: Quit",
        )
        .style(Style::default().fg(Color::DarkGray)).wrap(Wrap { trim: true });
        f.render_widget(help, chunks[3]);

        // Editing popup
        if self.editing {
            let popup_area = centered_rect(60, 20, rect);
            f.render_widget(Clear, popup_area);
            let popup = Paragraph::new(self.edit_buffer.as_str()).block(
                Block::default()
                    .title("Edit Parameter Value (Enter=save, Esc=cancel)")
                    .borders(Borders::ALL),
            );
            f.render_widget(popup, popup_area);
        }
        if self.show_load_dialog {
            let popup_area = centered_rect(60, 20, rect);
            f.render_widget(Clear, popup_area);
            let popup = Paragraph::new(self.load_path.as_str()).block(
                Block::default()
                    .title("Load from file (Enter=load, Esc =cancel)")
                    .borders(Borders::ALL),
            );
            f.render_widget(popup, popup_area);
        }
        // Save dialog popup
        if self.show_save_dialog {
            let popup_area = centered_rect(60, 20, rect);
            f.render_widget(Clear, popup_area);
            let popup = Paragraph::new(self.save_path.as_str()).block(
                Block::default()
                    .title("Save to file (Enter=save, Esc=cancel)")
                    .borders(Borders::ALL),
            );
            f.render_widget(popup, popup_area);
        }
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
