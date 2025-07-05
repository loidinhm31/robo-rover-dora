use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArmCommand {
    JointPosition {
        joint_angles: Vec<f64>,
        max_velocity: Option<f64>,
    },
    CartesianMove {
        x: f64,
        y: f64,
        z: f64,
        roll: f64,
        pitch: f64,
        yaw: f64,
        max_velocity: Option<f64>,
    },
    RelativeMove {
        delta_joints: Vec<f64>,
    },
    Stop,
    Home,
    EmergencyStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmCommandWithMetadata {
    pub command: Option<ArmCommand>,
    pub metadata: CommandMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub command_id: String,
    pub timestamp: u64,
    pub source: InputSource,
    pub priority: CommandPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputSource {
    Keyboard,
    WebBridge,
    Autonomous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Emergency = 4,
}