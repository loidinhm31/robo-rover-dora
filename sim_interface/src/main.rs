use dora_node_api::{
    arrow::array::{Array, AsArray, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use robo_rover_lib::{ArmCommand, ArmCommandWithMetadata, ArmTelemetry};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio;

// Joint positions structure matching urdf-viz API
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct JointPositions {
    pub shoulder_pan: f64,
    pub shoulder_lift: f64,
    pub elbow_flex: f64,
    pub wrist_flex: f64,
    pub wrist_roll: f64,
    pub gripper: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct UrdfVizCommand {
    names: Vec<String>,
    positions: Vec<f64>,
}

// urdf-viz response structure
#[derive(Deserialize, Debug)]
struct UrdfVizResponse {
    pub is_ok: bool,
    pub reason: String,
}

impl JointPositions {
    /// Create home position for LeKiwi arm
    pub fn home() -> Self {
        Self {
            shoulder_pan: 0.0,
            shoulder_lift: 0.0,
            elbow_flex: 0.0,
            wrist_flex: 0.0,
            wrist_roll: 0.0,
            gripper: 0.0,
        }
    }

    /// Create zero position
    pub fn zero() -> Self {
        Self {
            shoulder_pan: 0.0,
            shoulder_lift: 0.0,
            elbow_flex: 0.0,
            wrist_flex: 0.0,
            wrist_roll: 0.0,
            gripper: 0.0,
        }
    }

    /// Convert to array
    pub fn to_array(&self) -> [f64; 6] {
        [
            self.shoulder_pan,
            self.shoulder_lift,
            self.elbow_flex,
            self.wrist_flex,
            self.wrist_roll,
            self.gripper,
        ]
    }

    /// Create from array
    pub fn from_array(arr: &[f64]) -> Self {
        assert!(arr.len() >= 6, "Array must have at least 6 elements");
        Self {
            shoulder_pan: arr[0],
            shoulder_lift: arr[1],
            elbow_flex: arr[2],
            wrist_flex: arr[3],
            wrist_roll: arr[4],
            gripper: arr[5],
        }
    }

    /// Validate joint limits
    pub fn validate(&self) -> Result<()> {
        if self.shoulder_pan < -3.14 || self.shoulder_pan > 3.14 {
            eyre::bail!("shoulder_pan out of range: {} (expected -3.14 to 3.14)", self.shoulder_pan);
        }
        if self.shoulder_lift < -1.57 || self.shoulder_lift > 1.57 {
            eyre::bail!("shoulder_lift out of range: {} (expected -1.57 to 1.57)", self.shoulder_lift);
        }
        if self.elbow_flex < -2.09 || self.elbow_flex > 2.09 {
            eyre::bail!("elbow_flex out of range: {} (expected -2.09 to 2.09)", self.elbow_flex);
        }
        if self.wrist_flex < -3.14 || self.wrist_flex > 3.14 {
            eyre::bail!("wrist_flex out of range: {} (expected -3.14 to 3.14)", self.wrist_flex);
        }
        if self.wrist_roll < -1.57 || self.wrist_roll > 1.57 {
            eyre::bail!("wrist_roll out of range: {} (expected -1.57 to 1.57)", self.wrist_roll);
        }
        if self.gripper < -3.14 || self.gripper > 3.14 {
            eyre::bail!("gripper out of range: {} (expected -3.14 to 3.14)", self.gripper);
        }
        Ok(())
    }

    /// Convert to urdf-viz command
    fn to_command(&self) -> UrdfVizCommand {
        UrdfVizCommand {
            names: vec![
                "STS3215_03a-v1_Revolute-45".to_string(),
                "STS3215_03a-v1-1_Revolute-49".to_string(),
                "STS3215_03a-v1-2_Revolute-51".to_string(),
                "STS3215_03a-v1-3_Revolute-53".to_string(),
                "STS3215_03a-v1-3_Revolute-53".to_string(),
                "STS3215_03a-v1-4_Revolute-57".to_string(),
            ],
            positions: vec![
                self.shoulder_pan,
                self.shoulder_lift,
                self.elbow_flex,
                self.wrist_flex,
                self.wrist_roll,
                self.gripper,
            ],
        }
    }
}

/// Robot controller for urdf-viz simulation
pub struct UrdfVizController {
    url: String,
    client: reqwest::Client,
    current_position: JointPositions,
}

impl UrdfVizController {
    /// Create a new controller with default urdf-viz URL
    pub fn new() -> Self {
        Self {
            url: "http://127.0.0.1:7777".to_string(),
            client: reqwest::Client::new(),
            current_position: JointPositions::home(),
        }
    }

    /// Create with custom URL
    pub fn with_url(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
            current_position: JointPositions::home(),
        }
    }

    /// Send joint positions to urdf-viz
    pub async fn send_positions(&mut self, positions: JointPositions) -> Result<()> {
        println!("Sending joint positions to urdf-viz:");
        println!("  shoulder_pan:  {:.3} rad ({:.1}°)", positions.shoulder_pan, positions.shoulder_pan.to_degrees());
        println!("  shoulder_lift: {:.3} rad ({:.1}°)", positions.shoulder_lift, positions.shoulder_lift.to_degrees());
        println!("  elbow_flex:    {:.3} rad ({:.1}°)", positions.elbow_flex, positions.elbow_flex.to_degrees());
        println!("  wrist_flex:    {:.3} rad ({:.1}°)", positions.wrist_flex, positions.wrist_flex.to_degrees());
        println!("  wrist_roll:    {:.3} rad ({:.1}°)", positions.wrist_roll, positions.wrist_roll.to_degrees());
        println!("  gripper:       {:.3} rad ({:.1}°)", positions.gripper, positions.gripper.to_degrees());

        // Validate before sending
        positions.validate()?;

        let command = positions.to_command();
        let endpoint = format!("{}/set_joint_positions", self.url);

        let response = self.client
            .post(&endpoint)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&command)
            .send()
            .await?;

        if response.status().is_success() {
            let result: UrdfVizResponse = response.json().await?;
            if result.is_ok {
                println!("✓ Command sent successfully to urdf-viz!");
                self.current_position = positions;
            } else {
                eyre::bail!("urdf-viz error: {}", result.reason);
            }
        } else {
            eyre::bail!("HTTP error: {}", response.status());
        }

        Ok(())
    }

    /// Send joint positions from array
    pub async fn send_array(&mut self, positions: &[f64]) -> Result<()> {
        let joint_positions = JointPositions::from_array(positions);
        self.send_positions(joint_positions).await
    }

    /// Move to home position
    pub async fn home(&mut self) -> Result<()> {
        self.send_positions(JointPositions::home()).await
    }

    /// Interpolate between two positions over a duration
    pub async fn interpolate(
        &mut self,
        start: JointPositions,
        end: JointPositions,
        duration_ms: u64,
        steps: usize,
    ) -> Result<()> {
        let step_duration = duration_ms / steps as u64;

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let interpolated = JointPositions {
                shoulder_pan: start.shoulder_pan + t * (end.shoulder_pan - start.shoulder_pan),
                shoulder_lift: start.shoulder_lift + t * (end.shoulder_lift - start.shoulder_lift),
                elbow_flex: start.elbow_flex + t * (end.elbow_flex - start.elbow_flex),
                wrist_flex: start.wrist_flex + t * (end.wrist_flex - start.wrist_flex),
                wrist_roll: start.wrist_roll + t * (end.wrist_roll - start.wrist_roll),
                gripper: start.gripper + t * (end.gripper - start.gripper),
            };

            self.send_positions(interpolated).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(step_duration)).await;
        }

        Ok(())
    }

    /// Get current position
    pub fn current_position(&self) -> &JointPositions {
        &self.current_position
    }

    /// Generate telemetry from current position
    /// Uses simplified forward kinematics (can be enhanced with full Modern Robotics FK later)
    pub fn generate_telemetry(&self) -> ArmTelemetry {
        // Simplified forward kinematics for end-effector position
        // This is a placeholder - full implementation would use Product of Exponentials
        // from Modern Robotics (Chapter 4)

        // For now, approximate end-effector position based on joint angles
        let l1 = 0.3; // Link lengths (should match LeKiwi URDF)
        let l2 = 0.25;
        let l3 = 0.15;

        let theta1 = self.current_position.shoulder_pan;
        let theta2 = self.current_position.shoulder_lift;
        let theta3 = self.current_position.elbow_flex;

        // Simplified planar forward kinematics (2D projection)
        let x = l1 * theta2.cos() + l2 * (theta2 + theta3).cos();
        let y = theta1.sin() * (l1 * theta2.cos() + l2 * (theta2 + theta3).cos());
        let z = l1 * theta2.sin() + l2 * (theta2 + theta3).sin() + 0.1; // base height

        ArmTelemetry {
            end_effector_pose: [
                x,
                y,
                z,
                self.current_position.wrist_roll,
                self.current_position.wrist_flex,
                theta1,
            ],
            is_moving: false,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            joint_angles: Some(self.current_position.to_array().to_vec()),
            joint_velocities: Some(vec![0.0; 6]),
            source: "urdf_viz_simulation".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Simulation Interface for LeKiwi Arm");
    println!("Connecting to urdf-viz at http://127.0.0.1:7777");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let arm_telemetry_output = DataId::from("arm_telemetry".to_owned());

    // Get urdf-viz URL from environment or use default
    let urdf_viz_url = std::env::var("URDFVIZ_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7777".to_string());

    let mut controller = UrdfVizController::with_url(urdf_viz_url);

    println!("Simulation interface initialized");
    println!("Sending robot to home position...");

    // Initialize to home position
    if let Err(e) = controller.home().await {
        eprintln!("Warning: Failed to send home position to urdf-viz: {}", e);
        eprintln!("Make sure urdf-viz is running with:");
        eprintln!("  urdf-viz path/to/LeKiwi.urdf");
    }

    println!("Ready to receive commands");

    // Event loop
    loop {
        if let Some(event) = events.recv() {
            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();

                    match id_str {
                        "arm_command" => {
                            println!("Received arm command");

                            if let Some(binary_array) = data.as_binary_opt::<i32>() {
                                if binary_array.len() > 0 {
                                    let bytes = binary_array.value(0);

                                    match serde_json::from_slice::<ArmCommandWithMetadata>(bytes) {
                                        Ok(cmd_with_metadata) => {
                                            if let Some(arm_cmd) = cmd_with_metadata.command {
                                                println!("Processing arm command: {:?}", arm_cmd);

                                                match arm_cmd {
                                                    ArmCommand::JointPosition { joint_angles, .. } => {
                                                        if joint_angles.len() >= 6 {
                                                            let positions = JointPositions::from_array(&joint_angles);

                                                            match controller.send_positions(positions).await {
                                                                Ok(_) => {
                                                                    println!("Joint positions sent successfully");

                                                                    // Send telemetry
                                                                    let telemetry = controller.generate_telemetry();
                                                                    if let Ok(serialized) = serde_json::to_vec(&telemetry) {
                                                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                                                                        let _ = node.send_output(
                                                                            arm_telemetry_output.clone(),
                                                                            Default::default(),
                                                                            arrow_data,
                                                                        );
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    eprintln!("Failed to send joint positions: {}", e);
                                                                }
                                                            }
                                                        } else {
                                                            eprintln!("Invalid joint angles length: {}", joint_angles.len());
                                                        }
                                                    }
                                                    ArmCommand::Home => {
                                                        if let Err(e) = controller.home().await {
                                                            eprintln!("Failed to move to home: {}", e);
                                                        }
                                                    }
                                                    ArmCommand::Stop => {
                                                        println!("Stop command received");
                                                        // In simulation, this is a no-op
                                                    }
                                                    ArmCommand::EmergencyStop => {
                                                        println!("Emergency stop received");
                                                        // Move to safe position
                                                        let _ = controller.home().await;
                                                    }
                                                    _ => {
                                                        println!("Unsupported command type for simulation");
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to parse arm command: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            println!("Unknown input: {}", id_str);
                        }
                    }
                }

                Event::Stop(_) => {
                    println!("Stop event received");
                    break;
                }

                _ => {}
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    println!("Simulation interface shutting down");
    Ok(())
}