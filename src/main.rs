mod app;
mod config;
mod mode_selection;
mod parser;

use app::App;
use mode_selection::{LaunchMode, ModeSelector};

fn main() {
    // 模式选择
    let mut selector = ModeSelector::new();

    let launch_mode = match selector.run() {
        Ok(Some(mode)) => mode,
        Ok(None) => return, // 用户选择退出
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    // 根据选择创建配置
    let (config, merge_result) = match launch_mode {
        LaunchMode::OpenConfig(config) => (config, None),
        LaunchMode::ExtractFromScript { config, merge_result } => (config, merge_result),
    };

    // 启动主应用
    let mut app = App::with_config(config, merge_result);

    if let Err(e) = app.run() {
        eprintln!("Error: {}", e);
    }
}
