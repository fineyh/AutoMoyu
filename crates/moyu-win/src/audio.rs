//! 游戏声音采集：优先只录 Minecraft 进程自己的声音（WASAPI 进程级环回，Win10 2004+/Win11），
//! 音乐、语音聊天、视频都不会干扰；不支持时退回整机环回。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use anyhow::{anyhow, Result};
use serde::Serialize;
use wasapi::{AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

pub const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioSource {
    /// 只录游戏进程。
    Process,
    /// 整机环回（会混进其他声音）。
    System,
}

pub struct AudioCapture {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    pub source: AudioSource,
    pub sample_rate: u32,
}

fn open(pid: Option<u32>) -> Result<(AudioClient, AudioSource)> {
    let fmt = WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE as usize, CHANNELS, None);
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
    if let Some(pid) = pid {
        match AudioClient::new_application_loopback_client(pid, true)
            .and_then(|mut c| c.initialize_client(&fmt, &Direction::Capture, &mode).map(|_| c))
        {
            Ok(c) => return Ok((c, AudioSource::Process)),
            Err(e) => tracing::warn!("进程环回不可用，退回整机环回: {e}"),
        }
    }
    let dev = DeviceEnumerator::new()?.get_default_device(&Direction::Render)?;
    let mut c = dev.get_iaudioclient()?;
    c.initialize_client(&fmt, &Direction::Capture, &mode)?;
    Ok((c, AudioSource::System))
}

impl AudioCapture {
    /// 开始采集，单声道 f32 样本块通过 `tx` 送出。
    pub fn start(pid: Option<u32>, tx: Sender<Vec<f32>>) -> Result<AudioCapture> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = sync_channel::<Result<AudioSource>>(1);
        let stop2 = stop.clone();
        let join = std::thread::Builder::new().name("audio".into()).spawn(move || {
            let _ = wasapi::initialize_mta().ok();
            let setup = (|| -> Result<_> {
                let (client, source) = open(pid)?;
                let event = client.set_get_eventhandle()?;
                let cap = client.get_audiocaptureclient()?;
                client.start_stream()?;
                Ok((client, source, event, cap))
            })();
            let (client, source, event, cap) = match setup {
                Ok(v) => v,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(source));
            let mut q: VecDeque<u8> = VecDeque::new();
            let frame_bytes = 4 * CHANNELS;
            while !stop2.load(Ordering::Relaxed) {
                // 游戏静音时进程环回不会来事件，超时就继续等
                if event.wait_for_event(200).is_err() {
                    continue;
                }
                loop {
                    match cap.get_next_packet_size() {
                        Ok(Some(n)) if n > 0 => {
                            if cap.read_from_device_to_deque(&mut q).is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                let usable = q.len() / frame_bytes * frame_bytes;
                if usable == 0 {
                    continue;
                }
                let bytes: Vec<u8> = q.drain(..usable).collect();
                let mono: Vec<f32> = bytes
                    .chunks_exact(frame_bytes)
                    .map(|f| {
                        let l = f32::from_le_bytes([f[0], f[1], f[2], f[3]]);
                        let r = f32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                        0.5 * (l + r)
                    })
                    .collect();
                if tx.send(mono).is_err() {
                    break;
                }
            }
            let _ = client.stop_stream();
        })?;
        let source = ready_rx.recv().map_err(|_| anyhow!("声音线程意外退出"))??;
        Ok(AudioCapture { stop, join: Some(join), source, sample_rate: SAMPLE_RATE })
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
