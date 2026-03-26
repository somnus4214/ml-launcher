mod app;
mod config;

use app::App;

fn main() {
    let mut app = App::new();

    if let Err(e) = app.run() {
        eprintln!("Error: {}", e);
    }
}
