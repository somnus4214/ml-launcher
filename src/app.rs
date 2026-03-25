use crate::config::{ParamField, ParamType, TrainConfig};
use eframe::egui;
use std::process::Command;

pub struct MLLauncherApp {
    config: TrainConfig,
    command_output: String,
    running: bool,
    show_save_dialog: bool,
    save_path: String,
}

impl Default for MLLauncherApp {
    fn default() -> Self {
        Self {
            config: TrainConfig::default(),
            command_output: String::new(),
            running: false,
            show_save_dialog: false,
            save_path: "config.json".to_string(),
        }
    }
}

impl eframe::App for MLLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ML Training Launcher");

            ui.horizontal(|ui| {
                ui.label("Script:");
                ui.text_edit_singleline(&mut self.config.script_path);
            });

            ui.horizontal(|ui| {
                ui.label("Python:");
                ui.text_edit_singleline(&mut self.config.python_path);
            });

            ui.separator();
            ui.heading("Parameters");

            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_remove: Option<usize> = None;
                let mut param_types: Vec<(usize, ParamType)> = Vec::new();

                for (i, param) in self.config.params.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(&param.name);
                        match param.param_type {
                            ParamType::Boolean => {
                                let mut checked = param.value == "true";
                                if ui.checkbox(&mut checked, "").clicked() {
                                    param.value = if checked { "true".to_string() } else { "false".to_string() };
                                }
                            }
                            _ => {
                                ui.text_edit_singleline(&mut param.value);
                            }
                        };
                        ui.label(&param.description);
                        ui.small(&format!("({:?})", param.param_type));

                        let selected = format!("{:?}", param.param_type);
                        egui::ComboBox::from_id_salt(i)
                            .selected_text(&selected)
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut param.param_type, ParamType::Float, "Float").clicked() {
                                    param_types.push((i, ParamType::Float));
                                }
                                if ui.selectable_value(&mut param.param_type, ParamType::Integer, "Integer").clicked() {
                                    param_types.push((i, ParamType::Integer));
                                }
                                if ui.selectable_value(&mut param.param_type, ParamType::String, "String").clicked() {
                                    param_types.push((i, ParamType::String));
                                }
                                if ui.selectable_value(&mut param.param_type, ParamType::Boolean, "Boolean").clicked() {
                                    param_types.push((i, ParamType::Boolean));
                                }
                                if ui.selectable_value(&mut param.param_type, ParamType::Path, "Path").clicked() {
                                    param_types.push((i, ParamType::Path));
                                }
                            });

                        if ui.small_button("X").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }

                if let Some(i) = to_remove {
                    self.config.params.remove(i);
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Add Parameter").clicked() {
                    self.config.params.push(ParamField::new(
                        "new_param",
                        "",
                        ParamType::String,
                        "Description",
                    ));
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Generate Command").clicked() {
                    self.command_output = self.config.build_command();
                }

                if ui.button("Run Training").clicked() && !self.running {
                    self.run_training();
                }

                if ui.button("Save Config").clicked() {
                    self.show_save_dialog = true;
                }

                if ui.button("Load Config").clicked() {
                    if let Ok(config) = TrainConfig::load(&self.save_path) {
                        self.config = config;
                        self.command_output = format!("Loaded config from {}", self.save_path);
                    }
                }
            });

            ui.separator();
            ui.label("Generated Command:");
            ui.monospace(&self.command_output);

            if self.running {
                ui.spinner();
                ui.label("Training in progress...");
            }
        });

        if self.show_save_dialog {
            egui::Window::new("Save Configuration").show(ctx, |ui| {
                ui.label("Save to:");
                ui.text_edit_singleline(&mut self.save_path);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if let Err(e) = self.config.save(&self.save_path) {
                            self.command_output = format!("Error: {}", e);
                        } else {
                            self.command_output = format!("Saved to {}", self.save_path);
                        }
                        self.show_save_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_save_dialog = false;
                    }
                });
            });
        }
    }
}

impl MLLauncherApp {
    fn run_training(&mut self) {
        let cmd = self.config.build_command();
        self.command_output = format!("Running: {}\n", cmd);
        self.running = true;

        // Build args vector
        let mut args: Vec<String> = self.config.script_path
            .split_whitespace()
            .map(String::from)
            .collect();

        for param in &self.config.params {
            if let Some(arg) = param.to_arg() {
                for part in arg.split_whitespace() {
                    args.push(String::from(part));
                }
            }
        }

        match Command::new(&self.config.python_path)
            .args([&self.config.script_path])
            .args(&args)
            .output()
        {
            Ok(output) => {
                self.command_output.push_str(&String::from_utf8_lossy(&output.stdout));
                if !output.stderr.is_empty() {
                    self.command_output.push_str(&String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                self.command_output.push_str(&format!("Error: {}", e));
            }
        }
        self.running = false;
    }
}
