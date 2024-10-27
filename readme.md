<div align='center'>

## Hyprwall
An unofficial GUI for setting wallpapers with multiple backends, built with GTK4 and Rust. 🚀🦀<br>

![Logo](.github/hyprwall.png)

</div>

## Differences between other GUI wallpaper pickers:
- **Rust** - Built with Rust, so it's blazingly fast and memory-safe.
- **No dependencies** - Unlike other GUI wallpaper pickers, Hyprwall doesn't have any package dependencies (other than rust), so it's lightweight and easy to install.
- **Minimalist** - Hyprwall is minimalist, the source code is very small compared to other wallpaper pickers e.g. (waypaper).
- **Wrapping** - Hyprwall supports wrapping, so if you choose to you can have a lot of wallpapers shown in the GUI at once (wraps with window size).
- **Performance** - Hyprwall is designed to be performant, it uses a thread pool to load images in parallel and caches images.
- **High capacity** - Hyprwall can handle a large number of wallpapers (over 1000 at one time!) without any issues.
- **Multiple monitors** - Hyprwall supports setting wallpapers on **Multiple** monitors at once.
- **True async** - Hyprwall is built to be asynchronous, it uses tokio to run commands in this manner massively improving performance.
- **Cross display protocol/server support** - Hyprwall supports both **wayland** (swaybg, swww, hyprpaper, wallutils) and **x11** (feh, wallutils).
- **Cli args** - Hyprwall supports command line arguments, to view these type **`hyprwall --help`**, **--restore** is one of them, if you wish you can restore your last used wallpaper in the gui with this argument.
- **GIF support** - Hyprwall supports GIFs, but only if the **swww** backend is used.
- **Supports swaybg, swww, wallutils, feh, and hyprpaper** - Hyprwall supports a variety of wallpaper backends, so you can use it with your preferred wallpaper tool.

<div align='center'>

## Preview
![Preview](.github/preview.png)

</div>

## Requirements
- IPC enabled **(only for hyprland / hyprpaper users)**
- any backend listed above installed
- GTK-4 installed

## Installation

### GitHub Releases
See Hyprwall's [releases page](https://github.com/nnyyxxxx/hyprwall/releases) for downloadable binaries.

### Arch Linux
There are 2 different [AUR](https://aur.archlinux.org) packages available:

- [hyprwall](https://aur.archlinux.org/packages/hyprwall) - Latest release built from source
- [hyprwall-bin](https://aur.archlinux.org/packages/hyprwall-bin) - Latest release in binary form

Install the preferred package with:
```bash
git clone https://aur.archlinux.org/<package>.git
cd <package>
makepkg -si
```

Or, if you're using an [AUR Helper](https://wiki.archlinux.org/title/AUR_helpers), it's even simpler (using [paru](https://github.com/Morganamilo/paru) as an example):
```bash
paru -S <package>
```

## Building from source
1. Install Rust (preferably `rustup`) through your distro's package or [the official script](https://www.rust-lang.org/tools/install)
2. Install `git`, `pango`, and `gtk4`
3. Clone this repository:
`git clone https://github.com/nnyyxxxx/hyprwall && cd hyprwall`
4. Compile the app with `cargo build --release` or run it directly with `cargo run --release`

## TODO:
- [x] Implement GUI
- [x] Implement wrapping

## Credits:
- [Nyx](https://github.com/nnyyxxxx) - Implementing the GUI and maintaining the project
- [Adam](https://github.com/adamperkowski) - Rust improvements, maintaining the project
- [Vaxry](https://github.com/vaxerski) - Hyprpaper
- [rust-gtk](https://github.com/gtk-rs/gtk4-rs) - The GTK4 library
- [Hyprland](https://github.com/hyprwm/Hyprland) - The window manager
