use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoverCommand {
    pub throttle: f64,
    pub brake: f64,
    pub steering_angle: f64,
    pub timestamp: u64,
    pub command_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoverTelemetry {
    pub position: (f64, f64),
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub velocity: f64,
    pub nav_angles: Option<Vec<f64>>,
    pub nav_dists: Option<Vec<f64>>,
    pub near_sample: bool,
    pub picking_up: bool,
    pub timestamp: u64,
}

impl RoverCommand {
    pub fn new(throttle: f64, brake: f64, steering_angle: f64) -> Self {
        Self {
            // Allow negative throttle for reverse movement (-1.0 to 1.0)
            throttle: throttle.clamp(-1.0, 1.0),
            brake: brake.clamp(0.0, 1.0),
            steering_angle: steering_angle.clamp(-15.0, 15.0),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            command_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}