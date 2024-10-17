mod gui;

use gtk::{prelude::*, Application};
use std::process::Command;
use std::sync::Once;
use lazy_static::lazy_static;
use parking_lot::Mutex;

lazy_static! {
    static ref MONITORS: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

static INIT: Once = Once::new();

fn main() {
    let app = Application::builder()
        .application_id("nnyyxxxx.hyprwall")
        .build();

    app.connect_activate(gui::build_ui);
    app.run();
}

pub fn set_wallpaper(path: &str) {
    INIT.call_once(|| {
        *MONITORS.lock() = get_monitors();
    });

    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    if !execute_command(&preload_command) {
        return;
    }

    for monitor in MONITORS.lock().iter() {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        execute_command(&set_command);
    }
}

fn execute_command(command: &str) -> bool {
    match Command::new("sh").arg("-c").arg(command).status() {
        Ok(status) if status.success() => true,
        _ => {
            eprintln!("Failed to execute command: {}", command);
            false
        }
    }
}

fn get_monitors() -> Vec<String> {
    let output = Command::new("hyprctl")
        .arg("monitors")
        .output()
        .expect("Failed to execute hyprctl monitors");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.starts_with("Monitor ") {
                line.split_whitespace().nth(1).map(String::from)
            } else {
                None
            }
        })
        .collect()
}
