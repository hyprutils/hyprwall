use gtk::{
    gio, glib, prelude::*, Application, ApplicationWindow, Button, FlowBox, Image, ScrolledWindow,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const CONFIG_FILE: &str = "~/.config/hyprwall/config.ini";

struct ImageCache {
    cache: HashMap<String, gtk::Image>,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn get(&self, path: &str) -> Option<gtk::Image> {
        self.cache.get(path).cloned()
    }

    fn insert(&mut self, path: String, image: gtk::Image) {
        self.cache.insert(path, image);
    }
}

pub fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Hyprpaper Configuration")
        .default_width(800)
        .default_height(600)
        .build();

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let flowbox = FlowBox::builder()
        .valign(gtk::Align::Start)
        .halign(gtk::Align::Fill)
        .selection_mode(gtk::SelectionMode::None)
        .hexpand(true)
        .vexpand(true)
        .homogeneous(true)
        .row_spacing(10)
        .column_spacing(10)
        .build();

    scrolled_window.set_child(Some(&flowbox));

    let flowbox_ref = Rc::new(RefCell::new(flowbox));
    let cache = Rc::new(RefCell::new(ImageCache::new()));

    let choose_folder_button = Button::with_label("Change wallpaper folder");
    let flowbox_clone = Rc::clone(&flowbox_ref);
    let cache_clone = Rc::clone(&cache);
    let window_weak = window.downgrade();
    choose_folder_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            choose_folder(&window, &flowbox_clone, &cache_clone);
        }
    });

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
    main_box.append(&choose_folder_button);
    main_box.append(&scrolled_window);

    window.set_child(Some(&main_box));

    let flowbox_clone = Rc::clone(&flowbox_ref);
    let cache_clone = Rc::clone(&cache);
    window.connect_show(move |_| {
        if let Some(last_path) = load_last_path() {
            let flowbox_clone2 = Rc::clone(&flowbox_clone);
            let cache_clone2 = Rc::clone(&cache_clone);
            glib::idle_add_local(move || {
                load_images(&last_path, &flowbox_clone2, &cache_clone2);
                glib::ControlFlow::Continue
            });
        }
    });

    window.present();
}

fn choose_folder(
    window: &ApplicationWindow,
    flowbox: &Rc<RefCell<FlowBox>>,
    cache: &Rc<RefCell<ImageCache>>,
) {
    let dialog = gtk::FileChooserDialog::new(
        Some("Change wallpaper folder"),
        Some(window),
        gtk::FileChooserAction::SelectFolder,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Open", gtk::ResponseType::Accept),
        ],
    );

    if let Some(last_path) = load_last_path() {
        let _ = dialog.set_current_folder(Some(&gio::File::for_path(last_path)));
    }

    let flowbox_clone = Rc::clone(flowbox);
    let cache_clone = Rc::clone(cache);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(folder) = dialog.file().and_then(|f| f.path()) {
                load_images(&folder, &flowbox_clone, &cache_clone);
                save_last_path(&folder);
            }
        }
        dialog.close();
    });

    dialog.show();
}

fn load_images(folder: &Path, flowbox: &Rc<RefCell<FlowBox>>, cache: &Rc<RefCell<ImageCache>>) {
    let flowbox = flowbox.borrow_mut();
    while let Some(child) = flowbox.first_child() {
        flowbox.remove(&child);
    }

    let mut cache = cache.borrow_mut();

    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.filter_map(Result::ok) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Some(path) = entry.path().to_str() {
                        if path.ends_with(".png")
                            || path.ends_with(".jpg")
                            || path.ends_with(".jpeg")
                        {
                            let image = if let Some(cached_image) = cache.get(path) {
                                cached_image
                            } else {
                                let new_image = Image::from_file(path);
                                new_image.set_pixel_size(250);
                                cache.insert(path.to_string(), new_image.clone());
                                new_image
                            };

                            let button = Button::builder().child(&image).build();

                            let path_clone = path.to_string();
                            button.connect_clicked(move |_| {
                                crate::set_wallpaper(&path_clone);
                            });

                            flowbox.insert(&button, -1);
                        }
                    }
                }
            }
        }
    }
}

fn load_last_path() -> Option<PathBuf> {
    let config_path = shellexpand::tilde(CONFIG_FILE).into_owned();
    if let Ok(mut file) = fs::File::open(config_path) {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            for line in contents.lines() {
                if line.starts_with("folder = ") {
                    let path = line.trim_start_matches("folder = ");
                    return Some(PathBuf::from(shellexpand::tilde(path).into_owned()));
                }
            }
        }
    }
    None
}

fn save_last_path(path: &Path) {
    let config_path = shellexpand::tilde(CONFIG_FILE).into_owned();
    if let Some(parent) = PathBuf::from(&config_path).parent() {
        fs::create_dir_all(parent).ok();
    }
    let content = format!("[Settings]\nfolder = {}", path.to_str().unwrap_or(""));
    fs::write(config_path, content).ok();
}
