//! Off-main-thread `#AVI` movie decoding via `video-rs`/FFmpeg.
//!
//! One [`MovieWorker`] owns one decode thread and a bounded (capacity two)
//! [`FrameQueue`]. The Bevy side pulls the newest due frame each render frame;
//! stale frames are dropped so decoding never delays gameplay. Movie audio is
//! ignored — chart BGM and keysounds stay authoritative.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfVideo.cs:266-285`

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use video_rs::decode::Decoder;
use video_rs::location::Location;
use video_rs::Frame;

/// Maximum decoded frames buffered ahead of the consumer.
const QUEUE_CAP: usize = 2;
/// Drift (ms) beyond which the decoder seeks instead of decoding forward.
const SEEK_DRIFT_MS: i64 = 100;

/// A single decoded RGBA frame tagged with its presentation time (ms).
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Presentation timestamp in ms from the movie start.
    pub timestamp_ms: i64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Tightly-packed RGBA8 pixels (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Bounded latest-frame queue shared between the decode thread and the Bevy
/// consumer. Never holds more than [`QUEUE_CAP`] frames.
#[derive(Clone, Default)]
pub struct FrameQueue(Arc<Mutex<VecDeque<DecodedFrame>>>);

impl FrameQueue {
    /// Push a freshly decoded frame, dropping the oldest if the queue is full.
    /// A poisoned lock is treated as an empty queue (the decode thread's error
    /// channel surfaces the real failure).
    pub fn push(&self, frame: DecodedFrame) {
        if let Ok(mut q) = self.0.lock() {
            while q.len() >= QUEUE_CAP {
                q.pop_front();
            }
            q.push_back(frame);
        }
    }

    /// Remove every frame whose timestamp is at or before `target_ms` and
    /// return the newest of them; frames in the future stay queued.
    pub fn newest_due(&self, target_ms: i64) -> Option<DecodedFrame> {
        let mut q = self.0.lock().ok()?;
        let mut newest: Option<DecodedFrame> = None;
        while let Some(front) = q.front() {
            if front.timestamp_ms > target_ms {
                break;
            }
            newest = q.pop_front();
        }
        newest
    }

    /// Current buffered frame count.
    pub fn len(&self) -> usize {
        self.0.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Whether the queue holds no frames.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop all buffered frames (used after a seek).
    pub fn clear(&self) {
        if let Ok(mut q) = self.0.lock() {
            q.clear();
        }
    }
}

/// One-time global FFmpeg initialization result.
fn ensure_init() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| video_rs::init().map_err(|e| format!("ffmpeg init failed: {e}")))
        .clone()
}

/// True for the end-of-stream errors that mean "no more frames", as opposed to
/// a real decode failure.
fn is_stream_end(error: &video_rs::Error) -> bool {
    matches!(
        error,
        video_rs::Error::DecodeExhausted | video_rs::Error::ReadExhausted
    )
}

/// Convert a `video-rs` RGB frame (`ndarray` H×W×3) into packed RGBA bytes.
fn frame_to_rgba(frame: &Frame) -> (u32, u32, Vec<u8>) {
    let (h, w, _c) = frame.dim();
    let mut rgba = Vec::with_capacity(w * h * 4);
    if let Some(slice) = frame.as_slice() {
        for px in slice.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    } else {
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[
                    frame[[y, x, 0]],
                    frame[[y, x, 1]],
                    frame[[y, x, 2]],
                    255,
                ]);
            }
        }
    }
    (w as u32, h as u32, rgba)
}

/// Owns one movie decode thread feeding a bounded [`FrameQueue`].
pub struct MovieWorker {
    queue: FrameQueue,
    target_ms: Arc<AtomicI64>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    handle: Option<JoinHandle<()>>,
}

impl MovieWorker {
    /// Spawn a decode thread for `path`. FFmpeg init and decoder-open failures
    /// are surfaced through [`MovieWorker::take_error`] rather than panicking.
    pub fn spawn(path: PathBuf) -> Self {
        let queue = FrameQueue::default();
        let target_ms = Arc::new(AtomicI64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));

        let handle = {
            let queue = queue.clone();
            let target_ms = Arc::clone(&target_ms);
            let stop = Arc::clone(&stop);
            let error = Arc::clone(&error);
            std::thread::Builder::new()
                .name("dtx-bga-movie".into())
                .spawn(move || decode_loop(path, queue, target_ms, stop, error))
                .ok()
        };

        Self {
            queue,
            target_ms,
            stop,
            error,
            handle,
        }
    }

    /// Set the desired presentation time (ms) the consumer wants to display.
    pub fn set_target_ms(&self, target_ms: i64) {
        self.target_ms.store(target_ms, Ordering::Release);
    }

    /// Take the newest decoded frame due at `target_ms`, dropping older ones.
    pub fn newest_due_frame(&self, target_ms: i64) -> Option<DecodedFrame> {
        self.queue.newest_due(target_ms)
    }

    /// Take the first decode error, if any (clears it).
    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut e| e.take())
    }

    /// Signal the decode thread to stop and join it.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MovieWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn record_error(error: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut slot) = error.lock() {
        if slot.is_none() {
            *slot = Some(message);
        }
    }
}

fn decode_loop(
    path: PathBuf,
    queue: FrameQueue,
    target_ms: Arc<AtomicI64>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
) {
    if let Err(message) = ensure_init() {
        record_error(&error, message);
        return;
    }

    let mut decoder = match Decoder::new(Location::File(path)) {
        Ok(decoder) => decoder,
        Err(e) => {
            record_error(&error, format!("movie open failed: {e}"));
            return;
        }
    };

    let mut last_timestamp: i64 = 0;

    while !stop.load(Ordering::Acquire) {
        let target = target_ms.load(Ordering::Acquire).max(0);

        // Seek when the desired time is behind the decoder or far ahead of it.
        if target + SEEK_DRIFT_MS < last_timestamp || target > last_timestamp + 500 {
            if decoder.seek(target).is_err() {
                record_error(&error, "movie seek failed".into());
                break;
            }
            queue.clear();
            last_timestamp = target;
        }

        // Don't run far ahead of what the consumer wants; let it catch up.
        if last_timestamp > target + SEEK_DRIFT_MS || queue.len() >= QUEUE_CAP {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }

        match decoder.decode() {
            Ok((time, frame)) => {
                let (width, height, rgba) = frame_to_rgba(&frame);
                last_timestamp = (time.as_secs_f64() * 1000.0).round() as i64;
                queue.push(DecodedFrame {
                    timestamp_ms: last_timestamp,
                    width,
                    height,
                    rgba,
                });
            }
            Err(ref e) if is_stream_end(e) => break,
            Err(e) => {
                record_error(&error, format!("movie decode failed: {e}"));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ts: i64) -> DecodedFrame {
        DecodedFrame {
            timestamp_ms: ts,
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        }
    }

    #[test]
    fn frame_queue_never_exceeds_two_and_returns_newest_due() {
        let queue = FrameQueue::default();
        queue.push(frame(0));
        queue.push(frame(333));
        queue.push(frame(666));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.newest_due(700).map(|f| f.timestamp_ms), Some(666));
    }

    #[test]
    fn frame_queue_keeps_future_frames() {
        let queue = FrameQueue::default();
        queue.push(frame(100));
        queue.push(frame(900));
        // Target before the second frame: only the first is due, second stays.
        assert_eq!(queue.newest_due(200).map(|f| f.timestamp_ms), Some(100));
        assert_eq!(queue.len(), 1);
        assert!(queue.newest_due(150).is_none());
        assert_eq!(queue.len(), 1);
    }
}
