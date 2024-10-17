mod gui;

use gtk::{prelude::*, Application};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::{process::Command, process::Stdio, sync::Once};

lazy_static! {
    static ref MONITORS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref CURRENT_BACKEND: Mutex<WallpaperBackend> = Mutex::new(WallpaperBackend::Hyprpaper);
}

static INIT: Once = Once::new();

pub enum WallpaperBackend {
    Hyprpaper,
    Swaybg,
    Swww,
    Wallutils,
    Feh,
}

fn main() {
    let app = Application::builder()
        .application_id("nnyyxxxx.hyprwall")
        .build();

    app.connect_activate(gui::build_ui);
    app.run();
}

pub fn set_wallpaper(path: String) {
    glib::spawn_future_local(async move {
        match set_wallpaper_internal(&path).await {
            Ok(_) => println!("Wallpaper set successfully"),
            Err(e) => eprintln!("Error setting wallpaper: {}", e),
        }
    });
}

async fn set_wallpaper_internal(path: &str) -> Result<(), String> {
    ensure_backend_running()?;

    println!("Attempting to set wallpaper: {}", path);

    INIT.call_once(|| match get_monitors() {
        Ok(monitors) => *MONITORS.lock() = monitors,
        Err(e) => eprintln!("Failed to get monitors: {}", e),
    });

    println!("Found monitors: {:?}", *MONITORS.lock());

    match *CURRENT_BACKEND.lock() {
        WallpaperBackend::Hyprpaper => set_hyprpaper_wallpaper(path),
        WallpaperBackend::Swaybg => set_swaybg_wallpaper(path),
        WallpaperBackend::Swww => set_swww_wallpaper(path),
        WallpaperBackend::Wallutils => set_wallutils_wallpaper(path),
        WallpaperBackend::Feh => set_feh_wallpaper(path),
    }
}

fn set_hyprpaper_wallpaper(path: &str) -> Result<(), String> {
    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    if !execute_command(&preload_command) {
        return Err("Failed to preload wallpaper".to_string());
    }

    for monitor in MONITORS.lock().iter() {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        if !execute_command(&set_command) {
            return Err(format!("Failed to set wallpaper for {}", monitor));
        }
    }

    Ok(())
}

fn set_swaybg_wallpaper(path: &str) -> Result<(), String> {
    let set_command = format!("swaybg -i \"{}\" -m fill", path);
    if !execute_command(&set_command) {
        return Err("Failed to set wallpaper with swaybg".to_string());
    }
    Ok(())
}

fn set_swww_wallpaper(path: &str) -> Result<(), String> {
    let set_command = format!("swww img \"{}\"", path);
    if !execute_command(&set_command) {
        return Err("Failed to set wallpaper with swww".to_string());
    }
    Ok(())
}

fn set_wallutils_wallpaper(path: &str) -> Result<(), String> {
    let set_command = format!("setwallpaper \"{}\"", path);
    if !execute_command(&set_command) {
        return Err("Failed to set wallpaper with wallutils".to_string());
    }
    Ok(())
}

fn set_feh_wallpaper(path: &str) -> Result<(), String> {
    let set_command = format!("feh --bg-fill \"{}\"", path);
    if !execute_command(&set_command) {
        return Err("Failed to set wallpaper with feh".to_string());
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

fn ensure_backend_running() -> Result<(), String> {
    match *CURRENT_BACKEND.lock() {
        WallpaperBackend::Hyprpaper => ensure_hyprpaper_running(),
        WallpaperBackend::Swaybg => ensure_swaybg_running(),
        WallpaperBackend::Swww => ensure_swww_running(),
        WallpaperBackend::Wallutils => Ok(()),
        WallpaperBackend::Feh => Ok(()),
    }
}

fn ensure_hyprpaper_running() -> Result<(), String> {
    if !is_process_running("hyprpaper") {
        println!("hyprpaper is not running. Attempting to start it...");
        start_process("hyprpaper")?;
    }
    Ok(())
}

fn ensure_swaybg_running() -> Result<(), String> {
    if !is_process_running("swaybg") {
        println!("swaybg is not running. Attempting to start it...");
        start_process("swaybg")?;
    }
    Ok(())
}

fn ensure_swww_running() -> Result<(), String> {
    if !is_process_running("swww") {
        println!("swww is not running. Attempting to start it...");
        start_process("swww init")?;
    }
    Ok(())
}

fn is_process_running(process_name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(process_name)
        .stdout(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn start_process(command: &str) -> Result<(), String> {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start {}: {}", command, e))?;

    std::thread::sleep(std::time::Duration::from_secs(1));

    if is_process_running(command.split_whitespace().next().unwrap_or(command)) {
        Ok(())
    } else {
        Err(format!("Failed to start {}", command))
    }
}

pub fn set_wallpaper_backend(backend: WallpaperBackend) {
    *CURRENT_BACKEND.lock() = backend;
}
