mod gui;

use gtk::{prelude::*, Application};
use std::process::Command;

fn main() {
    let app = Application::builder()
        .application_id("nnyyxxxx.hyprwall")
        .build();

    app.connect_activate(gui::build_ui);
    app.run();
}

pub fn set_wallpaper(path: &str) {
    println!("Attempting to set wallpaper: {}", path);
    let monitors = get_monitors();
    println!("Found monitors: {:?}", monitors);

    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    println!("Preloading wallpaper: {}", preload_command);
    let preload_output = Command::new("sh")
        .arg("-c")
        .arg(&preload_command)
        .output()
        .expect("Failed to execute preload command");

    if !preload_output.status.success() {
        eprintln!(
            "Failed to preload wallpaper: {:?}",
            String::from_utf8_lossy(&preload_output.stderr)
        );
        return;
    }

    println!("Wallpaper preloaded successfully");

    for monitor in monitors {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        println!("Executing command: {}", set_command);
        let output = Command::new("sh")
            .arg("-c")
            .arg(&set_command)
            .output()
            .expect("Failed to execute set wallpaper command");

        if !output.status.success() {
            eprintln!(
                "Failed to set wallpaper for {}: {:?}",
                monitor,
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            println!("Successfully set wallpaper for {}", monitor);
        }
    }
}

fn get_monitors() -> Vec<String> {
    let output = Command::new("hyprctl")
        .arg("monitors")
        .output()
        .expect("Failed to execute hyprctl monitors");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut monitors = Vec::new();

    for line in output_str.lines() {
        if line.starts_with("Monitor ") {
            if let Some(monitor_name) = line.split_whitespace().nth(1) {
                monitors.push(monitor_name.to_string());
            }
        }
    }

    monitors
}
