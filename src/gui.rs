use gtk::{
    gio, glib, prelude::*, Application, ApplicationWindow, Button, FlowBox, Image, ScrolledWindow,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const CONFIG_FILE: &str = "~/.config/hyprwall/config.ini";
const BATCH_SIZE: usize = 15;

struct ImageLoader {
    queue: VecDeque<PathBuf>,
    current_folder: Option<PathBuf>,
}

impl ImageLoader {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            current_folder: None,
        }
    }

    fn load_folder(&mut self, folder: &Path) {
        self.queue.clear();
        self.current_folder = Some(folder.to_path_buf());
        if let Ok(entries) = fs::read_dir(folder) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        if let Some(extension) = path.extension() {
                            if ["png", "jpg", "jpeg"].contains(&extension.to_str().unwrap_or("")) {
                                self.queue.push_back(path);
                            }
                        }
                    }
                }
            }
        }
    }

    fn next_batch(&mut self) -> Vec<PathBuf> {
        self.queue.drain(..BATCH_SIZE.min(self.queue.len())).collect()
    }

    fn has_more(&self) -> bool {
        !self.queue.is_empty()
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
    let image_loader = Rc::new(RefCell::new(ImageLoader::new()));

    let choose_folder_button = Button::with_label("Change wallpaper folder");
    let flowbox_clone = Rc::clone(&flowbox_ref);
    let image_loader_clone = Rc::clone(&image_loader);
    let window_weak = window.downgrade();
    choose_folder_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            choose_folder(&window, &flowbox_clone, &image_loader_clone);
        }
    });

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
    main_box.append(&choose_folder_button);
    main_box.append(&scrolled_window);

    window.set_child(Some(&main_box));

    let flowbox_clone = Rc::clone(&flowbox_ref);
    let image_loader_clone = Rc::clone(&image_loader);
    window.connect_show(move |_| {
        if let Some(last_path) = load_last_path() {
            let flowbox_clone2 = Rc::clone(&flowbox_clone);
            let image_loader_clone2 = Rc::clone(&image_loader_clone);
            glib::idle_add_local(move || {
                load_images(&last_path, &flowbox_clone2, &image_loader_clone2);
                glib::ControlFlow::Continue
            });
        }
    });

    let flowbox_clone = Rc::clone(&flowbox_ref);
    let image_loader_clone = Rc::clone(&image_loader);
    scrolled_window.connect_edge_reached(move |_, pos| {
        if pos == gtk::PositionType::Bottom {
            load_more_images(&flowbox_clone, &image_loader_clone);
        }
    });

    window.present();
}

fn choose_folder(
    window: &ApplicationWindow,
    flowbox: &Rc<RefCell<FlowBox>>,
    image_loader: &Rc<RefCell<ImageLoader>>,
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
    let image_loader_clone = Rc::clone(image_loader);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            if let Some(folder) = dialog.file().and_then(|f| f.path()) {
                load_images(&folder, &flowbox_clone, &image_loader_clone);
                save_last_path(&folder);
            }
        }
        dialog.close();
    });

    dialog.show();
}

fn load_images(folder: &Path, flowbox: &Rc<RefCell<FlowBox>>, image_loader: &Rc<RefCell<ImageLoader>>) {
    {
        let flowbox = flowbox.borrow_mut();
        while let Some(child) = flowbox.first_child() {
            flowbox.remove(&child);
        }
    }

    {
        let mut image_loader = image_loader.borrow_mut();
        image_loader.load_folder(folder);
    }

    load_more_images(flowbox, image_loader);
}

fn load_more_images(flowbox: &Rc<RefCell<FlowBox>>, image_loader: &Rc<RefCell<ImageLoader>>) {
    let batch;
    let has_more;
    {
        let mut image_loader = image_loader.borrow_mut();
        batch = image_loader.next_batch();
        has_more = image_loader.has_more();
    }

    {
        let flowbox = flowbox.borrow_mut();
        for path in batch {
            let image = Image::from_file(&path);
            image.set_pixel_size(250);

            let button = Button::builder().child(&image).build();

            let path_clone = path.to_str().unwrap_or("").to_string();
            button.connect_clicked(move |_| {
                crate::set_wallpaper(&path_clone);
            });

            flowbox.insert(&button, -1);
        }
    }

    if has_more {
        let flowbox_clone = Rc::clone(flowbox);
        let image_loader_clone = Rc::clone(image_loader);
        glib::idle_add_local(move || {
            load_more_images(&flowbox_clone, &image_loader_clone);
            glib::ControlFlow::Continue
        });
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
