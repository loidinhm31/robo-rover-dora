use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bounding box in normalized coordinates [0.0, 1.0]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x1: f32,  // Top-left x
    pub y1: f32,  // Top-left y
    pub x2: f32,  // Bottom-right x
    pub y2: f32,  // Bottom-right y
}

impl BoundingBox {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Get center coordinates
    pub fn center(&self) -> (f32, f32) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }

    /// Get width
    pub fn width(&self) -> f32 {
        self.x2 - self.x1
    }

    /// Get height
    pub fn height(&self) -> f32 {
        self.y2 - self.y1
    }

    /// Get area (normalized)
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Compute IoU (Intersection over Union) with another box
    pub fn iou(&self, other: &BoundingBox) -> f32 {
        let x1 = self.x1.max(other.x1);
        let y1 = self.y1.max(other.y1);
        let x2 = self.x2.min(other.x2);
        let y2 = self.y2.min(other.y2);

        if x2 < x1 || y2 < y1 {
            return 0.0;
        }

        let intersection = (x2 - x1) * (y2 - y1);
        let union = self.area() + other.area() - intersection;

        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }

    /// Convert to pixel coordinates given image dimensions
    pub fn to_pixels(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        (
            (self.x1 * width as f32) as u32,
            (self.y1 * height as f32) as u32,
            (self.x2 * width as f32) as u32,
            (self.y2 * height as f32) as u32,
        )
    }
}

/// Single detection result from object detector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub bbox: BoundingBox,
    pub class_id: usize,
    pub class_name: String,
    pub confidence: f32,
    pub tracking_id: Option<u32>,  // Assigned by tracker
}

impl DetectionResult {
    pub fn new(bbox: BoundingBox, class_id: usize, class_name: String, confidence: f32) -> Self {
        Self {
            bbox,
            class_id,
            class_name,
            confidence,
            tracking_id: None,
        }
    }
}

/// Frame containing all detections at a given timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionFrame {
    pub frame_id: u64,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub detections: Vec<DetectionResult>,
}

impl DetectionFrame {
    pub fn new(frame_id: u64, width: u32, height: u32, detections: Vec<DetectionResult>) -> Self {
        Self {
            frame_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            width,
            height,
            detections,
        }
    }
}

/// Target object selected for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingTarget {
    pub tracking_id: u32,
    pub class_name: String,
    pub bbox: BoundingBox,
    pub last_seen: u64,
    pub confidence: f32,
    pub lost_frames: u32,  // Number of consecutive frames target was lost
}

impl TrackingTarget {
    pub fn new(tracking_id: u32, class_name: String, bbox: BoundingBox, confidence: f32) -> Self {
        Self {
            tracking_id,
            class_name,
            bbox,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            confidence,
            lost_frames: 0,
        }
    }

    /// Check if target is considered lost (e.g., not seen for > 30 frames)
    pub fn is_lost(&self, max_lost_frames: u32) -> bool {
        self.lost_frames > max_lost_frames
    }
}

/// Commands to control tracking system from web UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TrackingCommand {
    /// Enable tracking mode
    Enable {
        timestamp: u64,
    },
    /// Disable tracking mode
    Disable {
        timestamp: u64,
    },
    /// Select a target by detection index in current frame
    SelectTarget {
        detection_index: usize,
        timestamp: u64,
    },
    /// Select a target by tracking ID
    SelectTargetById {
        tracking_id: u32,
        timestamp: u64,
    },
    /// Clear current target
    ClearTarget {
        timestamp: u64,
    },
}

impl TrackingCommand {
    pub fn new_enable() -> Self {
        Self::Enable {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn new_disable() -> Self {
        Self::Disable {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn new_select_target(detection_index: usize) -> Self {
        Self::SelectTarget {
            detection_index,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn new_select_target_by_id(tracking_id: u32) -> Self {
        Self::SelectTargetById {
            tracking_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn new_clear_target() -> Self {
        Self::ClearTarget {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

/// Current state of tracking system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrackingState {
    Disabled,
    Enabled,
    Tracking,
    TargetLost,
}

/// Telemetry data sent to web UI about tracking status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingTelemetry {
    pub state: TrackingState,
    pub target: Option<TrackingTarget>,
    pub distance_estimate: Option<f32>,  // Relative distance estimate (0.0-1.0)
    pub control_output: Option<ControlOutput>,
    pub timestamp: u64,
}

impl TrackingTelemetry {
    pub fn new(state: TrackingState, target: Option<TrackingTarget>) -> Self {
        Self {
            state,
            target,
            distance_estimate: None,
            control_output: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance_estimate = Some(distance);
        self
    }

    pub fn with_control(mut self, control: ControlOutput) -> Self {
        self.control_output = Some(control);
        self
    }
}

/// Control outputs for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlOutput {
    pub omega_z: f64,  // Angular velocity command
    pub v_x: f64,      // Linear velocity command
    pub error_x: f32,  // Horizontal error (pixels or normalized)
    pub error_size: f32,  // Size error for distance
}

impl ControlOutput {
    pub fn new(omega_z: f64, v_x: f64, error_x: f32, error_size: f32) -> Self {
        Self {
            omega_z,
            v_x,
            error_x,
            error_size,
        }
    }
}
