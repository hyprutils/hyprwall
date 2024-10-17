use crossbeam_channel::unbounded;
use glib::ControlFlow;
use gtk::gdk::Texture;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::{
    gdk, gio, glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, Button,
    EventControllerMotion, FlowBox, Image, ScrolledWindow,
};
use parking_lot::Mutex;
use rand::seq::SliceRandom;
use rayon::prelude::*;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    fs,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

const CONFIG_FILE: &str = "~/.config/hyprwall/config.ini";
const CACHE_SIZE: usize = 100;

struct ImageCache {
    cache: HashMap<PathBuf, gdk::Texture>,
    order: VecDeque<PathBuf>,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(CACHE_SIZE),
            order: VecDeque::with_capacity(CACHE_SIZE),
        }
    }

    fn get(&mut self, path: &Path) -> Option<gdk::Texture> {
        if let Some(texture) = self.cache.get(path) {
            self.order.retain(|p| p != path);
            self.order.push_front(path.to_path_buf());
            Some(texture.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, path: PathBuf, texture: gdk::Texture) {
        if self.cache.len() >= CACHE_SIZE {
            if let Some(old_path) = self.order.pop_back() {
                self.cache.remove(&old_path);
            }
        }
        self.cache.insert(path.clone(), texture);
        self.order.push_front(path);
    }

    fn get_or_insert(&mut self, path: &Path, max_size: i32) -> Option<Texture> {
        if let Some(texture) = self.get(path) {
            Some(texture)
        } else {
            let pixbuf = Pixbuf::from_file_at_scale(path, max_size, max_size, true).ok()?;
            let texture = Texture::for_pixbuf(&pixbuf);
            self.insert(path.to_path_buf(), texture.clone());
            Some(texture)
        }
    }
}

struct ImageLoader {
    queue: VecDeque<PathBuf>,
    current_folder: Option<PathBuf>,
    cache: Arc<Mutex<ImageCache>>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl ImageLoader {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            current_folder: None,
            cache: Arc::new(Mutex::new(ImageCache::new())),
            cancel_flag: None,
        }
    }

    fn load_folder(&mut self, folder: &Path) {
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::Relaxed);
        }

        self.queue.clear();
        self.current_folder = Some(folder.to_path_buf());
        if let Ok(entries) = fs::read_dir(folder) {
            self.queue.extend(entries.filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.is_file()
                        && matches!(
                            path.extension().and_then(|e| e.to_str()),
                            Some("png" | "jpg" | "jpeg")
                        )
                    {
                        Some(path)
                    } else {
                        None
                    }
                })
            }));
        }
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

    let random_button = Button::with_label("Random");
    let exit_button = Button::with_label("Exit");

    let bottom_box = GtkBox::new(gtk::Orientation::Horizontal, 10);
    bottom_box.set_margin_top(10);
    bottom_box.set_margin_bottom(10);
    bottom_box.set_halign(gtk::Align::Center);
    bottom_box.append(&choose_folder_button);
    bottom_box.append(&random_button);
    bottom_box.append(&exit_button);

    let main_box = GtkBox::new(gtk::Orientation::Vertical, 0);
    main_box.append(&scrolled_window);
    main_box.append(&bottom_box);

    window.set_child(Some(&main_box));

    let flowbox_clone = Rc::clone(&flowbox_ref);
    let image_loader_clone = Rc::clone(&image_loader);
    window.connect_show(move |_| {
        if let Some(last_path) = load_last_path() {
            let flowbox_clone2 = Rc::clone(&flowbox_clone);
            let image_loader_clone2 = Rc::clone(&image_loader_clone);
            glib::idle_add_local(move || {
                load_images(&last_path, &flowbox_clone2, &image_loader_clone2);
                glib::ControlFlow::Break
            });
        }
    });

    let flowbox_clone = Rc::clone(&flowbox_ref);
    let image_loader_clone = Rc::clone(&image_loader);
    random_button.connect_clicked(move |_| {
        set_random_wallpaper(&flowbox_clone, &image_loader_clone);
    });

    let app_clone = app.clone();
    exit_button.connect_clicked(move |_| {
        app_clone.quit();
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

fn load_images(
    folder: &Path,
    flowbox: &Rc<RefCell<FlowBox>>,
    image_loader: &Rc<RefCell<ImageLoader>>,
) {
    let mut image_loader = image_loader.borrow_mut();

    if let Some(flag) = &image_loader.cancel_flag {
        flag.store(true, Ordering::Relaxed);
    }

    image_loader.load_folder(folder);

    let batch = image_loader.queue.drain(..).collect::<Vec<_>>();
    let cache = Arc::clone(&image_loader.cache);

    let flowbox_clone = Rc::clone(flowbox);
    let (sender, receiver) = unbounded::<(Texture, String)>();

    while let Some(child) = flowbox.borrow().first_child() {
        flowbox.borrow().remove(&child);
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = Arc::clone(&cancel_flag);
    let cancel_flag_clone2 = Arc::clone(&cancel_flag);

    std::thread::spawn(move || {
        let num_cores = num_cpus::get();
        batch
            .par_iter()
            .with_max_len(num_cores)
            .for_each_with(sender.clone(), |s, path| {
                if cancel_flag_clone.load(Ordering::Relaxed) {
                    return;
                }
                let texture = {
                    let mut cache = cache.lock();
                    match cache.get_or_insert(path, 250) {
                        Some(texture) => texture,
                        None => {
                            eprintln!("Failed to load texture for {:?}", path);
                            return;
                        }
                    }
                };

                let path_clone = path.to_str().unwrap_or("").to_string();
                if s.send((texture, path_clone)).is_err() {
                    cancel_flag_clone.store(true, Ordering::Relaxed);
                }
            });
    });

    glib::source::idle_add_local(move || {
        if cancel_flag_clone2.load(Ordering::Relaxed) {
            return ControlFlow::Break;
        }

        let flowbox = flowbox_clone.borrow_mut();
        for _ in 0..10 {
            match receiver.try_recv() {
                Ok((texture, path_clone)) => {
                    let image = Image::from_paintable(Some(&texture));
                    image.set_pixel_size(250);

                    let button = Button::builder().child(&image).build();
                    button.set_has_frame(false);

                    let motion_controller = EventControllerMotion::new();
                    let button_weak = button.downgrade();
                    motion_controller.connect_enter(move |_, _, _| {
                        if let Some(button) = button_weak.upgrade() {
                            button.set_has_frame(true);
                        }
                    });
                    let button_weak = button.downgrade();
                    motion_controller.connect_leave(move |_| {
                        if let Some(button) = button_weak.upgrade() {
                            button.set_has_frame(false);
                        }
                    });
                    button.add_controller(motion_controller);

                    button.connect_clicked(move |_| {
                        crate::set_wallpaper(&path_clone);
                    });

                    flowbox.insert(&button, -1);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    cancel_flag_clone2.store(true, Ordering::Relaxed);
                    return ControlFlow::Break;
                }
            }
        }
        ControlFlow::Continue
    });

    image_loader.cancel_flag = Some(cancel_flag);
}

fn load_last_path() -> Option<PathBuf> {
    let config_path = shellexpand::tilde(CONFIG_FILE).into_owned();
    fs::File::open(config_path).ok().and_then(|mut file| {
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;
        contents
            .lines()
            .find(|line| line.starts_with("folder = "))
            .map(|line| {
                PathBuf::from(shellexpand::tilde(line.trim_start_matches("folder = ")).into_owned())
            })
    })
}

fn save_last_path(path: &Path) {
    let config_path = shellexpand::tilde(CONFIG_FILE).into_owned();
    if let Some(parent) = PathBuf::from(&config_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = format!("[Settings]\nfolder = {}", path.to_str().unwrap_or(""));
    let _ = fs::write(config_path, content);
}

fn set_random_wallpaper(_flowbox: &Rc<RefCell<FlowBox>>, image_loader: &Rc<RefCell<ImageLoader>>) {
    let image_loader = image_loader.borrow();
    if let Some(current_folder) = &image_loader.current_folder {
        if let Ok(entries) = fs::read_dir(current_folder) {
            let images: Vec<_> = entries
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        let path = e.path();
                        if path.is_file()
                            && matches!(
                                path.extension().and_then(|e| e.to_str()),
                                Some("png" | "jpg" | "jpeg")
                            )
                        {
                            Some(path)
                        } else {
                            None
                        }
                    })
                })
                .collect();

            if let Some(random_image) = images.choose(&mut rand::thread_rng()) {
                if let Some(path_str) = random_image.to_str() {
                    crate::set_wallpaper(path_str);
                }
            }
        }
    }
}
