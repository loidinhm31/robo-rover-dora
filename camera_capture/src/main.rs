use dora_node_api::{
    arrow::array::BinaryArray,
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution},
    Camera,
};
use robo_rover_lib::CameraFrame;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

struct CameraCapture {
    camera: Camera,
    frame_counter: u64,
    width: u32,
    height: u32,
    dropped_frames: u64,
    capture_errors: u64,
}

impl CameraCapture {
    fn new() -> Result<Self> {
        let camera_index = std::env::var("CAMERA_DEVICE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let width = std::env::var("FRAME_WIDTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(640);

        let height = std::env::var("FRAME_HEIGHT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(480);

        let fps = std::env::var("FPS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);

        info!("Initializing camera capture:");
        info!("  Device index: {}", camera_index);
        info!("  Resolution: {}x{}", width, height);
        info!("  Target FPS: {}", fps);

        let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let camera = Camera::new(CameraIndex::Index(camera_index), requested)
            .map_err(|e| eyre::eyre!("Failed to open camera {}: {}", camera_index, e))?;

        info!("Camera opened successfully");
        info!("  Camera info: {}", camera.info().human_name());

        Ok(Self {
            camera,
            frame_counter: 0,
            width,
            height,
            dropped_frames: 0,
            capture_errors: 0,
        })
    }

    fn capture_frame(&mut self) -> Result<CameraFrame> {
        let frame = self.camera.frame()
            .map_err(|e| {
                self.capture_errors += 1;
                eyre::eyre!("Failed to capture frame: {}", e)
            })?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Convert to RGB8 buffer
        let decoded = frame.decode_image::<RgbFormat>()
            .map_err(|e| eyre::eyre!("Failed to decode frame: {}", e))?;

        let width = decoded.width();
        let height = decoded.height();
        let data = decoded.into_raw();

        self.frame_counter += 1;

        if self.frame_counter % 100 == 0 {
            info!("📸 Captured {} frames (errors: {}, dropped: {})",
                  self.frame_counter, self.capture_errors, self.dropped_frames);
        }

        Ok(CameraFrame {
            timestamp,
            frame_id: self.frame_counter,
            width,
            height,
            format: "RGB8".to_string(),
            data,
        })
    }

    fn get_stats(&self) -> CameraStats {
        CameraStats {
            frames_captured: self.frame_counter,
            frames_dropped: self.dropped_frames,
            capture_errors: self.capture_errors,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CameraStats {
    frames_captured: u64,
    frames_dropped: u64,
    capture_errors: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    info!("🎥 Starting camera capture node");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let raw_frames_output = DataId::from("raw_frames".to_owned());
    let camera_status_output = DataId::from("camera_status".to_owned());

    let mut camera_capture = match CameraCapture::new() {
        Ok(cam) => cam,
        Err(e) => {
            error!("Failed to initialize camera: {}", e);
            error!("Make sure:");
            error!("   1. Camera is connected");
            error!("   2. You have permissions (try: sudo usermod -a -G video $USER)");
            error!("   3. No other application is using the camera");
            return Err(e.into());
        }
    };

    info!("Camera capture initialized successfully");
    info!("Publishing frames to: {}", raw_frames_output.as_str());

    let mut last_stats_time = SystemTime::now();
    let stats_interval = std::time::Duration::from_secs(5);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, metadata: _, data: _ } => {
                if id.as_str() == "tick" {
                    // Capture frame
                    match camera_capture.capture_frame() {
                        Ok(frame) => {
                            debug!("Captured frame {}: {}x{} ({} bytes)",
                                   frame.frame_id, frame.width, frame.height, frame.data.len());

                            // Serialize and send
                            match serde_json::to_vec(&frame) {
                                Ok(serialized) => {
                                    let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                    if let Err(e) = node.send_output(
                                        raw_frames_output.clone(),
                                        Default::default(),
                                        arrow_data,
                                    ) {
                                        warn!("Failed to send frame: {}", e);
                                        camera_capture.dropped_frames += 1;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to serialize frame: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Frame capture error: {}", e);
                        }
                    }

                    // Send stats periodically
                    if last_stats_time.elapsed().unwrap() >= stats_interval {
                        let stats = camera_capture.get_stats();
                        if let Ok(serialized) = serde_json::to_vec(&stats) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            let _ = node.send_output(
                                camera_status_output.clone(),
                                Default::default(),
                                arrow_data,
                            );
                        }
                        last_stats_time = SystemTime::now();
                    }
                }
            }

            Event::Stop(_) => {
                info!("Stop event received");
                let stats = camera_capture.get_stats();
                info!("Final statistics:");
                info!("   Frames captured: {}", stats.frames_captured);
                info!("   Frames dropped: {}", stats.frames_dropped);
                info!("   Capture errors: {}", stats.capture_errors);
                break;
            }

            _ => {}
        }
    }

    info!("Camera capture node shutting down");
    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}