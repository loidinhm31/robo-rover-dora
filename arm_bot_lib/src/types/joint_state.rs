use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointState {
    pub positions: Vec<f64>,
    pub velocities: Vec<f64>,
    pub efforts: Vec<f64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmStatus {
    pub joint_state: JointState,
    pub end_effector_pose: [f64; 6], // x, y, z, roll, pitch, yaw
    pub is_moving: bool,
    pub is_homed: bool,
    pub error_state: Option<String>,
    pub current_command: Option<String>,
    pub reachability_status: ReachabilityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReachabilityStatus {
    Reachable,
    NearLimit,
    OutOfReach,
    Collision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryPoint {
    pub joint_positions: Vec<f64>,
    pub joint_velocities: Vec<f64>,
    pub time_from_start: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub points: Vec<TrajectoryPoint>,
    pub total_time: f64,
}

impl JointState {
    pub fn new(dof: usize) -> Self {
        Self {
            positions: vec![0.0; dof],
            velocities: vec![0.0; dof],
            efforts: vec![0.0; dof],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    pub fn update_timestamp(&mut self) {
        self.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}

impl ArmStatus {
    pub fn new(dof: usize) -> Self {
        Self {
            joint_state: JointState::new(dof),
            end_effector_pose: [0.0; 6],
            is_moving: false,
            is_homed: false,
            error_state: None,
            current_command: None,
            reachability_status: ReachabilityStatus::Reachable,
        }
    }
}