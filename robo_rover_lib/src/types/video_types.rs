use serde::{Deserialize, Serialize};

/// Raw camera frame data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraFrame {
    pub timestamp: u64,
    pub frame_id: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,  // "RGB8", "BGR8", "GRAY8"
    pub data: Vec<u8>,   // Raw pixel data
}

impl CameraFrame {
    /// Calculate the expected data size for validation
    pub fn expected_size(&self) -> usize {
        let bytes_per_pixel = match self.format.as_str() {
            "RGB8" | "BGR8" => 3,
            "GRAY8" => 1,
            "RGBA8" | "BGRA8" => 4,
            _ => 3, // Default to 3
        };
        (self.width * self.height) as usize * bytes_per_pixel
    }

    /// Validate frame data integrity
    pub fn validate(&self) -> Result<(), String> {
        let expected = self.expected_size();
        if self.data.len() != expected {
            return Err(format!(
                "Frame data size mismatch: got {} bytes, expected {} bytes",
                self.data.len(),
                expected
            ));
        }
        Ok(())
    }
}

/// Processed video frame (compressed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedFrame {
    pub timestamp: u64,
    pub frame_id: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,  // "JPEG", "PNG", "WEBP"
    pub quality: u8,     // 1-100 for JPEG
    pub data: Vec<u8>,   // Compressed image data
    pub overlay_data: Option<OverlayData>,
}

/// Telemetry overlay information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayData {
    pub rover_position: Option<(f64, f64)>,
    pub rover_velocity: Option<f64>,
    pub arm_position: Option<[f64; 6]>,
    pub battery_level: Option<f64>,
    pub signal_strength: Option<u8>,
    pub timestamp_text: String,
}

/// Video streaming statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStats {
    pub timestamp: u64,
    pub frames_processed: u64,
    pub frames_dropped: u64,
    pub avg_frame_size_kb: f64,
    pub avg_processing_time_ms: f64,
    pub current_fps: f64,
    pub bandwidth_kbps: f64,
}

/// Video quality settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VideoQuality {
    Low,      // 320x240, JPEG quality 60
    Medium,   // 640x480, JPEG quality 75
    High,     // 640x480, JPEG quality 90
    UltraHigh, // 1280x720, JPEG quality 95
}

impl VideoQuality {
    pub fn get_resolution(&self) -> (u32, u32) {
        match self {
            VideoQuality::Low => (320, 240),
            VideoQuality::Medium => (640, 480),
            VideoQuality::High => (640, 480),
            VideoQuality::UltraHigh => (1280, 720),
        }
    }

    pub fn get_jpeg_quality(&self) -> u8 {
        match self {
            VideoQuality::Low => 60,
            VideoQuality::Medium => 75,
            VideoQuality::High => 90,
            VideoQuality::UltraHigh => 95,
        }
    }
}

/// Video control commands from web clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoControl {
    pub command: VideoCommand,
    pub quality: Option<VideoQuality>,
    pub max_fps: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCommand {
    Start,
    Stop,
    Pause,
    Resume,
    ChangeQuality,
}