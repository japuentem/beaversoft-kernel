// ============================================================================
// Project: Beaversoft Meta-Kernel
// Author: José Alberto Puente Maldonado (japuentem)
// Brand: Beaversoft
// Description: Graphical User Interface (GUI) Subsystem & Universal Compositor
// ============================================================================

pub mod font;
pub mod canvas;
pub mod desktop;
pub mod apps_system;
pub mod apps_media;
pub mod window;
pub mod note;
pub mod bmp;
pub mod icons;
pub mod splash;

pub use desktop::{render_desktop, render_fullscreen_cli};
pub use splash::show_boot_splash;
