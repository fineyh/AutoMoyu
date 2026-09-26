//! AutoMoyu 的 Windows 平台层。只做 IO，不含判断逻辑（逻辑都在 moyu-core）。

pub mod audio;
pub mod capture;
pub mod dpi;
pub mod hook;
pub mod input;
pub mod power;
pub mod window;

pub use capture::{Bgra, ScreenCapture};
pub use window::GameWindow;
