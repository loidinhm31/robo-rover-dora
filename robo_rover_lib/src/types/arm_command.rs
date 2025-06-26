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
pub struct KeyboardInput {
    pub key: String,
    pub source: InputSource,
    pub timestamp: u64,
    pub modifiers: Vec<String>, // Ctrl, Shift, Alt
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputSource {
    Local,
    Unity,
    ControlPad, // For future use
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub command_id: String,
    pub timestamp: u64,
    pub source: InputSource,
    pub priority: CommandPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandPriority {
    Low,
    Normal,
    High,
    Emergency,
}

impl KeyboardInput {
    pub fn new_local(key: String) -> Self {
        Self {
            key,
            source: InputSource::Local,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            modifiers: Vec::new(),
        }
    }

    pub fn new_unity(key: String, modifiers: Vec<String>) -> Self {
        Self {
            key,
            source: InputSource::Unity,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            modifiers,
        }
    }
}