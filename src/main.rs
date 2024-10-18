mod gui;

use clap::Parser;
use gtk::{prelude::*, Application};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;

lazy_static! {
    static ref MONITORS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref CURRENT_BACKEND: Mutex<WallpaperBackend> = Mutex::new(WallpaperBackend::Hyprpaper);
}

#[derive(Clone, Copy)]
pub enum WallpaperBackend {
    Hyprpaper,
    Swaybg,
    Swww,
    Wallutils,
    Feh,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, help = "Restore the last selected wallpaper")]
    restore: bool,
}

fn main() {
    let cli = Cli::parse();

    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = rt.enter();

    if cli.restore {
        restore_last_wallpaper();
        return;
    }

    let app = Application::builder()
        .application_id("nnyyxxxx.hyprwall")
        .build();

    app.connect_activate(gui::build_ui);
    app.run();
}

pub fn set_wallpaper(path: String) {
    glib::spawn_future_local(async move {
        match set_wallpaper_internal(&path).await {
            Ok(_) => {
                println!("Wallpaper set successfully");
                gui::save_last_wallpaper(&path);
            }
            Err(e) => {
                eprintln!("Error setting wallpaper: {}", e);
                gui::custom_error_popup("Error setting wallpaper", &e, true);
            }
        }
    });
}

async fn set_wallpaper_internal(path: &str) -> Result<(), String> {
    ensure_backend_running().await?;

    println!("Attempting to set wallpaper: {}", path);

    let backend = *CURRENT_BACKEND.lock();
    match backend {
        WallpaperBackend::Hyprpaper => set_hyprpaper_wallpaper(path).await,
        WallpaperBackend::Swaybg => set_swaybg_wallpaper(path).await,
        WallpaperBackend::Swww => set_swww_wallpaper(path).await,
        WallpaperBackend::Wallutils => set_wallutils_wallpaper(path).await,
        WallpaperBackend::Feh => set_feh_wallpaper(path).await,
    }
}

async fn set_hyprpaper_wallpaper(path: &str) -> Result<(), String> {
    let preload_command = format!("hyprctl hyprpaper preload \"{}\"", path);
    spawn_background_process(&preload_command).await?;

    let monitors = get_monitors().await?;

    if monitors.is_empty() {
        return Err("No monitors detected".to_string());
    }

    *MONITORS.lock() = monitors.clone();

    for monitor in monitors {
        let set_command = format!("hyprctl hyprpaper wallpaper \"{},{}\"", monitor, path);
        spawn_background_process(&set_command).await?;
    }

    Ok(())
}

async fn set_swaybg_wallpaper(path: &str) -> Result<(), String> {
    let command = format!("swaybg -i \"{}\" -m fill", path);
    spawn_background_process(&command).await
}

async fn set_swww_wallpaper(path: &str) -> Result<(), String> {
    let command = format!("swww img \"{}\"", path);
    spawn_background_process(&command).await
}

async fn set_wallutils_wallpaper(path: &str) -> Result<(), String> {
    let command = format!("setwallpaper \"{}\"", path);
    spawn_background_process(&command).await
}

async fn set_feh_wallpaper(path: &str) -> Result<(), String> {
    let command = format!("feh --bg-fill \"{}\"", path);
    spawn_background_process(&command).await
}

async fn spawn_background_process(command: &str) -> Result<(), String> {
    let output = TokioCommand::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| format!("Failed to execute command '{}': {}", command, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Command '{}' failed with exit code {:?}.\nStdout: {}\nStderr: {}",
            command,
            output.status.code(),
            stdout,
            stderr
        ));
    }

    Ok(())
}

async fn get_monitors() -> Result<Vec<String>, String> {
    println!("Retrieving monitor information");
    let output = TokioCommand::new("hyprctl")
        .arg("monitors")
        .output()
        .await
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

async fn ensure_backend_running() -> Result<(), String> {
    let backend = *CURRENT_BACKEND.lock();
    match backend {
        WallpaperBackend::Hyprpaper => ensure_hyprpaper_running().await,
        WallpaperBackend::Swaybg => ensure_swaybg_running().await,
        WallpaperBackend::Swww => ensure_swww_running().await,
        WallpaperBackend::Wallutils => Ok(()),
        WallpaperBackend::Feh => Ok(()),
    }
}

async fn ensure_hyprpaper_running() -> Result<(), String> {
    if !is_process_running("hyprpaper").await {
        println!("hyprpaper is not running. Attempting to start it...");
        start_process("hyprpaper").await?;
    }
    Ok(())
}

async fn ensure_swaybg_running() -> Result<(), String> {
    if !is_process_running("swaybg").await {
        println!("swaybg is not running. Attempting to start it...");
        start_process("swaybg").await?;
    }
    Ok(())
}

async fn ensure_swww_running() -> Result<(), String> {
    if !is_process_running("swww-daemon").await {
        println!("swww is not running. Attempting to start it...");
        start_process("swww-daemon 2>/dev/null").await?;
    }
    Ok(())
}

async fn is_process_running(process_name: &str) -> bool {
    TokioCommand::new("pgrep")
        .arg("-x")
        .arg(process_name)
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn start_process(command: &str) -> Result<(), String> {
    TokioCommand::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()
        .map_err(|e| format!("Failed to start {}: {}", command, e))?;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    if is_process_running(command.split_whitespace().next().unwrap_or(command)).await {
        Ok(())
    } else {
        Err(format!("Failed to start {}", command))
    }
}

pub fn set_wallpaper_backend(backend: WallpaperBackend) {
    let previous_backend = {
        let mut current = CURRENT_BACKEND.lock();
        let prev = *current;
        *current = backend;
        prev
    };
    tokio::spawn(async move {
        drop_all_wallpapers(previous_backend).await;
        kill_previous_backend(previous_backend).await;
    });
}

async fn kill_previous_backend(backend: WallpaperBackend) {
    let process_name = match backend {
        WallpaperBackend::Hyprpaper => "hyprpaper",
        WallpaperBackend::Swaybg => "swaybg",
        WallpaperBackend::Swww => "swww-daemon",
        WallpaperBackend::Wallutils => return,
        WallpaperBackend::Feh => return,
    };

    let _ = TokioCommand::new("killall")
        .arg(process_name)
        .status()
        .await;
}

async fn drop_all_wallpapers(backend: WallpaperBackend) {
    match backend {
        WallpaperBackend::Hyprpaper => {
            let _ = TokioCommand::new("hyprctl")
                .args(["hyprpaper", "unload", "all"])
                .status()
                .await;
        }
        WallpaperBackend::Swww => {
            let _ = TokioCommand::new("swww").args(["clear"]).status().await;
        }
        _ => {}
    }
}

fn restore_last_wallpaper() {
    if let Some(last_wallpaper) = gui::load_last_wallpaper() {
        set_wallpaper(last_wallpaper);
    } else {
        eprintln!("No last wallpaper found to restore");
    }
}
