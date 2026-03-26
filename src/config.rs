use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamField {
    pub name: String,
    pub value: String,
    pub param_type: ParamType,
    pub description: String,
    pub use_default: bool, // 是否使用默认值（不需要修改）
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    Float,
    Integer,
    String,
    Boolean,
    Path,
}

/// 验证警告
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub param_name: String,
    pub expected_type: ParamType,
    pub invalid_value: String,
    pub message: String,
}

/// 配置合并结果
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub config: TrainConfig,
    pub inherited_params: Vec<String>,      // 从旧配置继承的参数名
    pub new_params: Vec<String>,            // 新提取的参数名
    pub deprecated_params: Vec<ParamField>, // 旧配置中存在但新配置中不存在的参数
}

impl ParamField {
    pub fn new(name: &str, value: &str, param_type: ParamType, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            param_type,
            description: description.to_string(),
            use_default: true,
        }
    }

    pub fn with_use_default(mut self, use_default: bool) -> Self {
        self.use_default = use_default;
        self
    }

    pub fn to_arg(&self) -> Option<String> {
        // 如果使用默认值，不生成命令行参数
        if self.use_default {
            return None;
        }
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

    /// 验证值是否符合类型要求，返回警告（如果有）
    pub fn validate_value(&self) -> Option<ValidationWarning> {
        if self.value.is_empty() {
            return None; // 空值视为有效
        }

        let is_valid = match self.param_type {
            ParamType::Integer => self.value.parse::<i64>().is_ok(),
            ParamType::Float => self.value.parse::<f64>().is_ok(),
            ParamType::Boolean => {
                let lower = self.value.to_lowercase();
                lower == "true" || lower == "false"
            }
            ParamType::String => true,
            ParamType::Path => true,
        };

        if !is_valid {
            Some(ValidationWarning {
                param_name: self.name.clone(),
                expected_type: self.param_type.clone(),
                invalid_value: self.value.clone(),
                message: format!(
                    "值 '{}' 不是有效的 {:?} 类型",
                    self.value, self.param_type
                ),
            })
        } else {
            // 对于 Path 类型，额外检查路径是否存在
            if self.param_type == ParamType::Path {
                let path = std::path::Path::new(&self.value);
                if !path.exists() {
                    return Some(ValidationWarning {
                        param_name: self.name.clone(),
                        expected_type: ParamType::Path,
                        invalid_value: self.value.clone(),
                        message: format!("路径 '{}' 不存在", self.value),
                    });
                }
            }
            None
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
                ParamField::new(
                    "resume",
                    "false",
                    ParamType::Boolean,
                    "Resume from checkpoint",
                ),
            ],
            env_vars: HashMap::new(),
        }
    }
}

impl TrainConfig {
    /// 从 parser 提取的参数创建配置
    pub fn from_parser_params(params: Vec<crate::parser::Param>) -> Self {
        let fields: Vec<ParamField> = params
            .into_iter()
            .map(|p| {
                let param_type = match p.param_type.as_str() {
                    "Integer" => ParamType::Integer,
                    "Float" => ParamType::Float,
                    "Boolean" => ParamType::Boolean,
                    "Path" => ParamType::Path,
                    _ => ParamType::String,
                };
                ParamField::new(&p.name, &p.value, param_type, &p.description)
            })
            .collect();

        Self {
            script_path: "train.py".to_string(),
            python_path: "python".to_string(),
            params: fields,
            env_vars: HashMap::new(),
        }
    }

    /// 合并新旧配置（继承功能）
    /// self: 旧配置（已有的 config.json）
    /// new_config: 新提取的配置
    pub fn merge_with_inheritance(&self, mut new_config: TrainConfig) -> MergeResult {
        let mut inherited_params = Vec::new();
        let mut new_params = Vec::new();
        let mut deprecated_params = Vec::new();

        // 建立新参数名称集合
        let new_param_names: HashSet<_> =
            new_config.params.iter().map(|p| p.name.as_str()).collect();

        // 建立旧参数映射
        let old_params_map: HashMap<&str, &ParamField> =
            self.params.iter().map(|p| (p.name.as_str(), p)).collect();

        // 先找出弃用的参数（在新配置中不存在）
        for old_param in &self.params {
            if !new_param_names.contains(old_param.name.as_str()) {
                deprecated_params.push(old_param.clone());
            }
        }

        // 收集需要继承的值
        let inherit_values: HashMap<String, (String, bool)> = new_config
            .params
            .iter()
            .filter_map(|p| {
                old_params_map.get(p.name.as_str()).map(|old| {
                    (p.name.clone(), (old.value.clone(), old.use_default))
                })
            })
            .collect();

        // 处理新配置中的每个参数
        for new_param in &mut new_config.params {
            if let Some((value, use_default)) = inherit_values.get(&new_param.name) {
                // 参数存在于旧配置中，继承值和 use_default 状态
                new_param.value = value.clone();
                new_param.use_default = *use_default;
                inherited_params.push(new_param.name.clone());
            } else {
                // 新参数，使用默认值
                new_params.push(new_param.name.clone());
            }
        }

        MergeResult {
            config: new_config,
            inherited_params,
            new_params,
            deprecated_params,
        }
    }

    /// 验证所有参数，返回警告列表
    pub fn validate_all(&self) -> Vec<ValidationWarning> {
        self.params
            .iter()
            .filter_map(|p| p.validate_value())
            .collect()
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

    pub fn load(&self, path: &str) -> Result<Self, Box<dyn std::error::Error>> {
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
