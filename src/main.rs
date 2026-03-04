mod app;
mod config;
mod grouping;
mod motion;
mod render;
mod terminal;
mod types;

use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    let exit_code = app::run_with_tmux_error_display();

    thread::sleep(Duration::from_secs(1));
    if exit_code == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
