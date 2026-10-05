//! moyu-bench：不开界面就能验证平台层、录制真实片段、离线评估算法。
//!
//! ```text
//! moyu-bench probe                      # 找窗口、截一张图，确认截得到游戏画面
//! moyu-bench record fixtures/day-machine --scenario machine
//! moyu-bench analyze fixtures/day-machine
//! moyu-bench dusk fixtures/day-machine fixtures/night-machine
//! ```

mod analyze;
mod dusk;
mod fixture;
mod probe;
mod record;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, about = "AutoMoyu 录制与离线评估工具")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scenario {
    /// 有钓鱼机：你的每次右键都是甩竿，机器负责收竿。
    Machine,
    /// 没有钓鱼机：你的右键交替为甩竿、收竿（从收回状态开始）。
    Manual,
}

#[derive(Subcommand)]
enum Cmd {
    /// 找 Minecraft 窗口并截一张图，检查平台层是否可用。
    Probe {
        /// 截图保存位置
        #[arg(long, default_value = "probe.png")]
        out: PathBuf,
    },
    /// 录制：画面（候选区 + 浮漂区）、游戏声音、你自己的右键。按 Ctrl+C 结束。
    Record {
        dir: PathBuf,
        #[arg(long, value_enum)]
        scenario: Scenario,
        #[arg(long, default_value_t = 10)]
        fps: u32,
        /// 最长录制分钟数
        #[arg(long, default_value_t = 15)]
        minutes: u32,
    },
    /// 离线评估一段录像：竿状态区分度/准确率、声音咬钩召回率与误报。
    Analyze {
        dir: PathBuf,
        /// 把报告写成 JSON
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// 模拟黄昏：白天录像校准，画面逐渐过渡到夜晚录像，看竿状态跟不跟得住。
    Dusk {
        day: PathBuf,
        night: PathBuf,
        /// 过渡用多少秒
        #[arg(long, default_value_t = 90)]
        seconds: u32,
        /// 夜晚背景额外压暗的倍数（竿不压），可给多个
        #[arg(long, value_delimiter = ',', default_value = "1,0.6,0.35,0.2")]
        darken: Vec<f32>,
        /// 夜晚竿也一起压暗的倍数
        #[arg(long, default_value_t = 1.0)]
        rod_darken: f32,
        #[arg(long, default_value_t = 5)]
        seeds: u32,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    moyu_win::dpi::set_per_monitor_v2();
    match Cli::parse().cmd {
        Cmd::Probe { out } => probe::run(&out),
        Cmd::Record { dir, scenario, fps, minutes } => record::run(&dir, scenario, fps, minutes),
        Cmd::Analyze { dir, json } => analyze::run(&dir, json.as_deref()),
        Cmd::Dusk { day, night, seconds, darken, rod_darken, seeds } => dusk::run(&day, &night, seconds, &darken, rod_darken, seeds),
    }
}
