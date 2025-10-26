// OPTIMIZED video_processor/src/main.rs - Fast version without text rendering
// Copy this file to: your-project/video_processor/src/main.rs

use dora_node_api::{
    arrow::array::{types::GenericBinaryType, Array, AsArray, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use image::{ImageBuffer, Rgb};
use robo_rover_lib::{
    ArmTelemetry, CameraFrame, OverlayData, ProcessedFrame, RoverTelemetry, VideoStats,
};
use std::error::Error;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

struct VideoProcessor {
    jpeg_quality: u8,
    target_width: u32,
    target_height: u32,
    overlay_enabled: bool,
    frame_counter: u64,
    processing_times: Vec<f64>,
    frame_sizes: Vec<usize>,
    last_rover_telemetry: Option<RoverTelemetry>,
    last_arm_telemetry: Option<ArmTelemetry>,
}

impl VideoProcessor {
    fn new() -> Result<Self> {
        let jpeg_quality = std::env::var("JPEG_QUALITY")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(70);

        let target_width = std::env::var("TARGET_WIDTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(320);

        let target_height = std::env::var("TARGET_HEIGHT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(240);

        let overlay_enabled = std::env::var("OVERLAY_TELEMETRY")
            .ok()
            .map(|s| s == "true")
            .unwrap_or(false);

        info!("🎬 Initializing OPTIMIZED video processor:");
        info!("  JPEG quality: {}", jpeg_quality);
        info!("  Target resolution: {}x{}", target_width, target_height);
        info!("  Overlay enabled: {}", overlay_enabled);

        Ok(Self {
            jpeg_quality,
            target_width,
            target_height,
            overlay_enabled,
            frame_counter: 0,
            processing_times: Vec::with_capacity(100),
            frame_sizes: Vec::with_capacity(100),
            last_rover_telemetry: None,
            last_arm_telemetry: None,
        })
    }

    fn process_frame(&mut self, camera_frame: CameraFrame) -> Result<ProcessedFrame> {
        let start_time = Instant::now();

        // Convert raw data to RgbImage - FAST PATH
        let mut img = if camera_frame.width == self.target_width
            && camera_frame.height == self.target_height {
            // No resize needed
            ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
                camera_frame.width,
                camera_frame.height,
                camera_frame.data,
            )
                .ok_or_else(|| eyre::eyre!("Failed to create image"))?
        } else {
            // Need to resize
            let temp_img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
                camera_frame.width,
                camera_frame.height,
                camera_frame.data,
            )
                .ok_or_else(|| eyre::eyre!("Failed to create image"))?;

            // Use FAST Nearest neighbor resize
            image::imageops::resize(
                &temp_img,
                self.target_width,
                self.target_height,
                image::imageops::FilterType::Nearest,
            )
        };

        // Minimal overlay (just crosshair)
        let overlay_data = if self.overlay_enabled {
            self.render_minimal_overlay(&mut img)?;
            Some(self.create_overlay_data())
        } else {
            None
        };

        // JPEG compression
        let mut jpeg_data = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
            &mut jpeg_data,
            self.jpeg_quality,
        );

        encoder.encode(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ColorType::Rgb8,
        )?;

        let processing_time = start_time.elapsed().as_secs_f64() * 1000.0;
        self.processing_times.push(processing_time);
        if self.processing_times.len() > 30 {
            self.processing_times.remove(0);
        }

        self.frame_sizes.push(jpeg_data.len());
        if self.frame_sizes.len() > 30 {
            self.frame_sizes.remove(0);
        }

        self.frame_counter += 1;

        if self.frame_counter % 30 == 0 {
            let avg_time = self.processing_times.iter().sum::<f64>() / self.processing_times.len() as f64;
            let avg_size = self.frame_sizes.iter().sum::<usize>() / self.frame_sizes.len();
            info!("📊 Frame {}: {:.1}ms, {:.1}KB", self.frame_counter, avg_time, avg_size as f64 / 1024.0);
        }

        Ok(ProcessedFrame {
            timestamp: camera_frame.timestamp,
            frame_id: camera_frame.frame_id,
            width: img.width(),
            height: img.height(),
            format: "JPEG".to_string(),
            quality: self.jpeg_quality,
            data: jpeg_data,
            overlay_data,
        })
    }

    fn render_minimal_overlay(&self, img: &mut image::RgbImage) -> Result<()> {
        // Simple crosshair - very fast
        let width = img.width() as i32;
        let height = img.height() as i32;
        let center_x = width / 2;
        let center_y = height / 2;
        let size = 10;

        for x in (center_x - size).max(0)..(center_x + size).min(width) {
            if let Some(pixel) = img.get_pixel_mut_checked(x as u32, center_y as u32) {
                *pixel = Rgb([255, 0, 0]);
            }
        }
        for y in (center_y - size).max(0)..(center_y + size).min(height) {
            if let Some(pixel) = img.get_pixel_mut_checked(center_x as u32, y as u32) {
                *pixel = Rgb([255, 0, 0]);
            }
        }

        Ok(())
    }

    fn create_overlay_data(&self) -> OverlayData {
        OverlayData {
            rover_position: self.last_rover_telemetry.as_ref().map(|t| t.position),
            rover_velocity: self.last_rover_telemetry.as_ref().map(|t| t.velocity),
            arm_position: self.last_arm_telemetry.as_ref().and_then(|t| {
                t.joint_angles.as_ref().and_then(|angles| {
                    if angles.len() >= 6 {
                        Some([angles[0], angles[1], angles[2], angles[3], angles[4], angles[5]])
                    } else {
                        None
                    }
                })
            }),
            battery_level: None,
            signal_strength: None,
            timestamp_text: String::new(),
        }
    }

    fn get_stats(&self) -> VideoStats {
        let avg_processing_time = if !self.processing_times.is_empty() {
            self.processing_times.iter().sum::<f64>() / self.processing_times.len() as f64
        } else {
            0.0
        };

        let avg_frame_size = if !self.frame_sizes.is_empty() {
            self.frame_sizes.iter().sum::<usize>() as f64 / self.frame_sizes.len() as f64
        } else {
            0.0
        };

        let avg_frame_size_kb = avg_frame_size / 1024.0;
        let current_fps = if avg_processing_time > 0.0 {
            1000.0 / avg_processing_time
        } else {
            0.0
        };
        let bandwidth_kbps = avg_frame_size_kb * 30.0 * 8.0;

        VideoStats {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            frames_processed: self.frame_counter,
            frames_dropped: 0,
            avg_frame_size_kb,
            avg_processing_time_ms: avg_processing_time,
            current_fps,
            bandwidth_kbps,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();
    info!("🎬 Starting OPTIMIZED video processor");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let processed_frames_output = DataId::from("processed_frames".to_owned());
    let stats_output = DataId::from("stats".to_owned());

    let mut processor = VideoProcessor::new()?;
    info!("✅ Optimized video processor initialized");

    let mut last_stats_time = SystemTime::now();
    let stats_interval = std::time::Duration::from_secs(3);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, metadata: _, data } => {
                match id.as_str() {
                    "raw_frames" => {
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<CameraFrame>(bytes) {
                                    Ok(camera_frame) => {
                                        match processor.process_frame(camera_frame) {
                                            Ok(processed) => {
                                                match serde_json::to_vec(&processed) {
                                                    Ok(serialized) => {
                                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                                                        if let Err(e) = node.send_output(processed_frames_output.clone(), Default::default(), arrow_data) {
                                                            warn!("Failed to send: {}", e);
                                                        }
                                                    }
                                                    Err(e) => warn!("Serialize failed: {}", e),
                                                }
                                            }
                                            Err(e) => warn!("Process failed: {}", e),
                                        }
                                    }
                                    Err(e) => warn!("Deserialize failed: {}", e),
                                }
                            }
                        }
                    }
                    "rover_telemetry" => {
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(telemetry) = serde_json::from_slice::<RoverTelemetry>(bytes) {
                                    processor.last_rover_telemetry = Some(telemetry);
                                }
                            }
                        }
                    }
                    "arm_telemetry" => {
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(telemetry) = serde_json::from_slice::<ArmTelemetry>(bytes) {
                                    processor.last_arm_telemetry = Some(telemetry);
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if last_stats_time.elapsed().unwrap() >= stats_interval {
                    let stats = processor.get_stats();
                    info!("📊 Stats: {:.1} FPS, {:.1} KB/frame, {:.1} Kbps",
                          stats.current_fps, stats.avg_frame_size_kb, stats.bandwidth_kbps);
                    if let Ok(serialized) = serde_json::to_vec(&stats) {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                        let _ = node.send_output(stats_output.clone(), Default::default(), arrow_data);
                    }
                    last_stats_time = SystemTime::now();
                }
            }
            Event::Stop(_) => {
                info!("🛑 Stop event received");
                break;
            }
            _ => {}
        }
    }

    info!("👋 Video processor shutting down");
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