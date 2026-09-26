//! AutoMoyu 核心算法。
//!
//! 这里的一切都是纯函数/纯状态机（sans-IO）：不截图、不点击、不读时钟。
//! 平台层（moyu-win）负责采集画面/声音和执行点击，把观测喂进来、把动作取出去。
//! 因此每条逻辑路径都能在任何机器上用脚本化输入单测，也能用录像离线回放。

pub mod bite_audio;
pub mod calibrate;
pub mod engine;
pub mod image;
pub mod rod_state;

pub use calibrate::{CalibrationResult, Calibrator, Geometry, Quality, Source};
pub use engine::{Engine, EngineConfig, Event, Mode, Observation, Phase, PauseReason, StopReason, WindowObs};
pub use image::{CellRect, Grid, RelRect};
pub use rod_state::{Classification, RodModel, RodState};
