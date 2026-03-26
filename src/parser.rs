use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Param {
    pub name: String,
    pub value: String,
    pub param_type: String,
    pub description: String,
    pub is_flag: bool,
}
pub fn extract_params(code: &str) -> Vec<Param> {
    let mut params = Vec::new();

    let args_blocks = extract_argument_blocks(code);

    let re_name = Regex::new(r#"['"](--[a-zA-Z0-9_-]+)['"]"#).unwrap();
    let re_type = Regex::new(r#"type=([a-zA-Z_]+)"#).unwrap();
    let re_action = Regex::new(r#"action=['"]([^'"]+)['"]"#).unwrap();
    let re_default = Regex::new(r#"default\s*=\s*(\[[^\]]*\]|'[^']*'|"[^"]*"|[^,\)]*)"#).unwrap();
    let re_help = Regex::new(r#"help\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap();
    let re_const_true = Regex::new(r#"const=True"#).unwrap();

    for args_str in args_blocks {
        let mut name = String::new();
        if let Some(name_cap) = re_name.captures(&args_str) {
            name = name_cap[1].trim_start_matches("--").to_string();
        }

        if name.is_empty() {
            continue;
        }

        let description = re_help
            .captures(&args_str)
            .and_then(|cap| cap.get(1).or(cap.get(2)))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        let mut value = re_default
            .captures(&args_str)
            .map(|cap| cap[1].trim().to_string())
            .unwrap_or_default();

        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            value = value[1..value.len() - 1].to_string();
        }

        let mut param_type = "String".to_string();
        let mut is_flag = false;

        let action_str = re_action
            .captures(&args_str)
            .map(|cap| cap[1].to_string())
            .unwrap_or_default();

        let is_store_true = action_str == "store_true" || re_const_true.is_match(&args_str);

        if is_store_true {
            param_type = "Boolean".to_string();
            is_flag = true;
            if value.is_empty() || value == "False" || value == "None" {
                value = "false".to_string();
            }
        } else if let Some(type_cap) = re_type.captures(&args_str) {
            let t = &type_cap[1];
            param_type = match t {
                "int" => "Integer".to_string(),
                "float" => "Float".to_string(),
                "bool" => "Boolean".to_string(),
                "str" => "String".to_string(),
                _ => "String".to_string(),
            };
        }

        if param_type == "String" {
            let lower_desc = description.to_lowercase();
            if lower_desc.contains("path")
                || lower_desc.contains("dir")
                || lower_desc.contains("directory")
                || name.contains("dir")
                || name.contains("weights")
                || name.contains("project")
            {
                param_type = "Path".to_string();
            }
        }

        params.push(Param {
            name,
            value,
            param_type,
            description,
            is_flag,
        });
    }

    params
}

pub fn extract_argument_blocks(code: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut start_idx = 0;
    let target = "add_argument(";

    while let Some(idx) = code[start_idx..].find(target) {
        let absolute_start = start_idx + idx + target.len();
        let mut parens = 1;
        let mut in_string = false;
        let mut escape = false;
        let mut string_char = ' ';
        let mut absolute_end = absolute_start;

        for (i, c) in code[absolute_start..].char_indices() {
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if in_string {
                if c == string_char {
                    in_string = false;
                }
            } else {
                if c == '"' || c == '\'' {
                    in_string = true;
                    string_char = c;
                } else if c == '(' {
                    parens += 1;
                } else if c == ')' {
                    parens -= 1;
                    if parens == 0 {
                        absolute_end = absolute_start + i;
                        break;
                    }
                }
            }
        }

        blocks.push(code[absolute_start..absolute_end].to_string());
        start_idx = absolute_end;
    }

    blocks
}
