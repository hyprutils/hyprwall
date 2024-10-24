mod gui;

use clap::Parser;
use gtk::{prelude::*, Application};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use rand::seq::SliceRandom;
use shellexpand::tilde;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;

lazy_static! {
    static ref MONITORS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref CURRENT_BACKEND: Mutex<WallpaperBackend> = Mutex::new(WallpaperBackend::Hyprpaper);
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    #[arg(short = 'r', long, help = "Restore the last selected wallpaper")]
    restore: bool,

    #[arg(short = 'R', long, help = "Set a random wallpaper")]
    random: bool,

    #[arg(short = 'b', long, help = "Set the wallpaper backend", default_value = None)]
    backend: Option<String>,

    #[arg(short = 'f', long, help = "Set the wallpaper folder", default_value = None)]
    folder: Option<PathBuf>,

    #[arg(short = 'w', long, help = "Set a specific wallpaper", default_value = None)]
    wallpaper: Option<PathBuf>,

    #[arg(short = 'g', long, help = "Generate the config file")]
    generate: bool,
}

fn main() {
    let cli = Cli::parse();

    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = rt.enter();

    if !config_exists() {
        generate_config();
    }

    load_wallpaper_backend();

    if cli.generate {
        generate_config();
        return;
    }

    if let Some(backend) = cli.backend {
        set_backend(&backend);
    }

    if let Some(folder) = cli.folder {
        set_folder(&folder);
    }

    if let Some(wallpaper) = cli.wallpaper {
        let wallpaper_path = wallpaper.to_string_lossy().into_owned();
        rt.block_on(async {
            let previous_backend = *CURRENT_BACKEND.lock();
            drop_all_wallpapers(previous_backend).await;
            kill_previous_backend(previous_backend).await;

            match set_wallpaper_internal(&wallpaper_path).await {
                Ok(_) => {
                    println!("Wallpaper set successfully: {}", wallpaper_path);
                    gui::save_last_wallpaper(&wallpaper_path);
                }
                Err(e) => eprintln!("Error setting wallpaper: {}", e),
            }
        });
        return;
    }

    if cli.restore {
        restore_last_wallpaper();
        return;
    }

    if cli.random {
        set_random_wallpaper();
        return;
    }

    let app = Application::builder()
        .application_id("nnyyxxxx.hyprwall")
        .build();

    app.connect_activate(gui::build_ui);
    app.run();
}

fn config_exists() -> bool {
    let config_path = tilde("~/.config/hyprwall/config.ini").into_owned();
    Path::new(&config_path).exists()
}

fn generate_config() {
    let config_path = tilde("~/.config/hyprwall/config.ini").into_owned();
    let config_dir = Path::new(&config_path).parent().unwrap();
    std::fs::create_dir_all(config_dir).expect("Failed to create config directory");

    let default_config = r#"[Settings]
folder = none
backend = none
last_wallpaper = none
"#;

    std::fs::write(&config_path, default_config).expect("Failed to write config file");
    println!("Config file generated at: {}", config_path);
}

fn set_backend(backend: &str) {
    let backend = match backend.to_lowercase().as_str() {
        "hyprpaper" => WallpaperBackend::Hyprpaper,
        "swaybg" => WallpaperBackend::Swaybg,
        "swww" => WallpaperBackend::Swww,
        "wallutils" => WallpaperBackend::Wallutils,
        "feh" => WallpaperBackend::Feh,
        _ => {
            eprintln!("Invalid backend specified. Using default (Hyprpaper).");
            WallpaperBackend::Hyprpaper
        }
    };
    set_wallpaper_backend(backend);
    println!("Wallpaper backend set to: {:?}", backend);
}

fn set_folder(folder: &Path) {
    if folder.is_dir() {
        let config_path = tilde("~/.config/hyprwall/config.ini").into_owned();
        let mut contents = String::new();

        if let Ok(mut file) = fs::File::open(&config_path) {
            file.read_to_string(&mut contents).unwrap_or_default();
        }

        let mut lines: Vec<String> = contents.lines().map(String::from).collect();
        let folder_line = format!("folder = {}", folder.display());

        if let Some(pos) = lines.iter().position(|line| line.starts_with("folder = ")) {
            lines[pos] = folder_line;
        } else {
            lines.push(folder_line);
        }

        let new_contents = lines.join("\n");

        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&config_path)
        {
            let _ = writeln!(file, "{}", new_contents);
        }

        println!("Wallpaper folder set to: {}", folder.display());
    } else {
        eprintln!("Specified folder does not exist or is not a directory.");
    }
}

fn set_random_wallpaper() {
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        match get_random_wallpaper().await {
            Ok(path) => match set_wallpaper_internal(&path).await {
                Ok(_) => {
                    println!("Random wallpaper set successfully: {}", path);
                    gui::save_last_wallpaper(&path);
                }
                Err(e) => eprintln!("Error setting random wallpaper: {}", e),
            },
            Err(e) => eprintln!("Error getting random wallpaper: {}", e),
        }
    });
}

async fn get_random_wallpaper() -> Result<String, String> {
    let config_path = tilde("~/.config/hyprwall/config.ini").into_owned();
    let contents = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let folder = contents
        .lines()
        .find(|line| line.starts_with("folder = "))
        .map(|line| line.trim_start_matches("folder = "))
        .ok_or_else(|| "Wallpaper folder not found in config".to_string())?;

    let folder_path = PathBuf::from(tilde(folder).into_owned());

    let mut entries = tokio::fs::read_dir(&folder_path)
        .await
        .map_err(|e| format!("Failed to read wallpaper directory: {}", e))?;

    let mut wallpapers = Vec::new();

    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if path.is_file()
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("png" | "jpg" | "jpeg")
            )
        {
            wallpapers.push(path);
        }
    }

    wallpapers
        .choose(&mut rand::thread_rng())
        .ok_or_else(|| "No wallpapers found".to_string())
        .map(|p| p.to_string_lossy().into_owned())
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
    let current_backend = *CURRENT_BACKEND.lock();
    
    kill_other_backends(current_backend).await;

    ensure_backend_running().await?;

    println!("Attempting to set wallpaper: {}", path);

    let result = match current_backend {
        WallpaperBackend::Hyprpaper => set_hyprpaper_wallpaper(path).await,
        WallpaperBackend::Swaybg => set_swaybg_wallpaper(path).await,
        WallpaperBackend::Swww => set_swww_wallpaper(path).await,
        WallpaperBackend::Wallutils => set_wallutils_wallpaper(path).await,
        WallpaperBackend::Feh => set_feh_wallpaper(path).await,
    };

    if result.is_ok() {
        gui::save_wallpaper_backend(&current_backend);
    }

    result
}

async fn kill_other_backends(current_backend: WallpaperBackend) {
    let backends = [
        ("hyprpaper", WallpaperBackend::Hyprpaper),
        ("swaybg", WallpaperBackend::Swaybg),
        ("swww-daemon", WallpaperBackend::Swww),
    ];

    for (process_name, backend) in backends.iter() {
        if *backend != current_backend {
            let _ = TokioCommand::new("killall")
                .arg(process_name)
                .status()
                .await;
        }
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
    let command = format!("swaybg -i \"{}\" -m fill &", path);
    TokioCommand::new("sh")
        .arg("-c")
        .arg(&command)
        .spawn()
        .map_err(|e| format!("Failed to start swaybg: {}", e))?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    if is_process_running("swaybg").await {
        Ok(())
    } else {
        Err("swaybg failed to start or crashed immediately".to_string())
    }
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
    gui::save_wallpaper_backend(&backend);
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
        let rt = Runtime::new().expect("Failed to create Tokio runtime");
        match rt.block_on(set_wallpaper_internal(&last_wallpaper)) {
            Ok(_) => {
                println!("Wallpaper restored successfully");
                gui::save_last_wallpaper(&last_wallpaper);
            }
            Err(e) => {
                eprintln!("Error restoring wallpaper: {}", e);
            }
        }
    } else {
        eprintln!("No last wallpaper found to restore");
    }
}

pub fn load_wallpaper_backend() {
    if let Some(backend) = gui::load_wallpaper_backend() {
        *CURRENT_BACKEND.lock() = backend;
    }
}
