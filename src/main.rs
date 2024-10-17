mod gui;

use gtk::{prelude::*, Application};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::{process::Command, process::Stdio, sync::Once};

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

pub fn set_wallpaper(path: &str) -> Result<(), String> {
    ensure_hyprpaper_running()?;

    println!("Attempting to set wallpaper: {}", path);

    INIT.call_once(|| match get_monitors() {
        Ok(monitors) => *MONITORS.lock() = monitors,
        Err(e) => eprintln!("Failed to get monitors: {}", e),
    });

    println!("Found monitors: {:?}", *MONITORS.lock());

    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    println!("Preloading wallpaper: {}", preload_command);
    if !execute_command(&preload_command) {
        return Err("Failed to preload wallpaper".to_string());
    }

    println!("Wallpaper preloaded successfully");

    for monitor in MONITORS.lock().iter() {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        println!("Executing command: {}", set_command);
        if !execute_command(&set_command) {
            return Err(format!("Failed to set wallpaper for {}", monitor));
        }
        println!("Successfully set wallpaper for {}", monitor);
    }

    Ok(())
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

fn get_monitors() -> Result<Vec<String>, String> {
    println!("Retrieving monitor information");
    let output = Command::new("hyprctl")
        .arg("monitors")
        .output()
        .map_err(|e| format!("Failed to execute hyprctl monitors: {}", e))?;

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
    Ok(monitors)
}

fn ensure_hyprpaper_running() -> Result<(), String> {
    if !is_hyprpaper_running() {
        println!("hyprpaper is not running. Attempting to start it...");
        start_hyprpaper()?;
    }
    Ok(())
}

fn is_hyprpaper_running() -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg("hyprpaper")
        .stdout(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn start_hyprpaper() -> Result<(), String> {
    Command::new("hyprpaper")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start hyprpaper: {}", e))?;

    std::thread::sleep(std::time::Duration::from_secs(1));

    if is_hyprpaper_running() {
        Ok(())
    } else {
        Err("Failed to start hyprpaper".to_string())
    }
}
