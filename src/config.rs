use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamField {
    pub name: String,
    pub value: String,
    pub param_type: ParamType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    Float,
    Integer,
    String,
    Boolean,
    Path,
}

impl ParamField {
    pub fn new(name: &str, value: &str, param_type: ParamType, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            param_type,
            description: description.to_string(),
        }
    }

    pub fn to_arg(&self) -> Option<String> {
        if self.value.is_empty() {
            return None;
        }
        match self.param_type {
            ParamType::Boolean => {
                if self.value == "true" {
                    Some(format!("--{}", self.name))
                } else {
                    None
                }
            }
            _ => Some(format!("--{} {}", self.name, self.value)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainConfig {
    pub script_path: String,
    pub python_path: String,
    pub params: Vec<ParamField>,
    pub env_vars: HashMap<String, String>,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            script_path: "train.py".to_string(),
            python_path: "python".to_string(),
            params: vec![
                ParamField::new("lr", "0.001", ParamType::Float, "Learning rate"),
                ParamField::new("epochs", "100", ParamType::Integer, "Number of epochs"),
                ParamField::new("batch-size", "32", ParamType::Integer, "Batch size"),
                ParamField::new("model", "", ParamType::String, "Model name"),
                ParamField::new("data-dir", "", ParamType::Path, "Data directory"),
                ParamField::new("checkpoint", "", ParamType::Path, "Checkpoint path"),
                ParamField::new("gpu", "0", ParamType::Integer, "GPU device ID"),
                ParamField::new("resume", "false", ParamType::Boolean, "Resume from checkpoint"),
            ],
            env_vars: HashMap::new(),
        }
    }
}

impl TrainConfig {
    pub fn build_command(&self) -> String {
        let mut cmd = format!("{} {}", self.python_path, self.script_path);
        for param in &self.params {
            if let Some(arg) = param.to_arg() {
                cmd.push_str(&format!(" {}", arg));
            }
        }
        cmd
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: TrainConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
