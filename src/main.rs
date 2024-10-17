mod gui;

use gtk::{prelude::*, Application};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::process::Command;
use std::sync::Once;

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
    println!("Attempting to set wallpaper: {}", path);

    INIT.call_once(|| {
        *MONITORS.lock() = get_monitors();
    });

    println!("Found monitors: {:?}", *MONITORS.lock());

    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    println!("Preloading wallpaper: {}", preload_command);
    if !execute_command(&preload_command) {
        eprintln!("Failed to preload wallpaper");
        return;
    }

    println!("Wallpaper preloaded successfully");

    for monitor in MONITORS.lock().iter() {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        println!("Executing command: {}", set_command);
        if execute_command(&set_command) {
            println!("Successfully set wallpaper for {}", monitor);
        } else {
            eprintln!("Failed to set wallpaper for {}", monitor);
        }
    }
}

fn execute_command(command: &str) -> bool {
    match Command::new("sh").arg("-c").arg(command).output() {
        Ok(output) => {
            if output.status.success() {
                true
            } else {
                eprintln!(
                    "Command failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                false
            }
        }
        Err(e) => {
            eprintln!("Failed to execute command: {}. Error: {}", command, e);
            false
        }
    }
}

fn get_monitors() -> Vec<String> {
    println!("Retrieving monitor information");
    let output = Command::new("hyprctl")
        .arg("monitors")
        .output()
        .expect("Failed to execute hyprctl monitors");

    let monitors: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.starts_with("Monitor ") {
                let monitor_name = line.split_whitespace().nth(1).map(String::from);
                println!("Found monitor: {:?}", monitor_name);
                monitor_name
            } else {
                None
            }
        })
        .collect();

    println!("Retrieved monitors: {:?}", monitors);
    monitors
}
