use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::arrow::datatypes::GenericBinaryType;
use dora_node_api::{arrow::array::BinaryArray, dora_core::config::DataId, DoraNode, Event};
use eyre::Result;
use robo_rover_lib::{ArmCommand, ArmConfig, ArmStatus, CommandMetadata, ForwardKinematics, InputSource};
use std::error::Error;
use tracing::{debug, info};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Initializing arm_controller node...");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("processed_arm_command".to_owned());

    println!("DoraNode initialized successfully");

    // Load arm configuration
    let config_path = std::env::var("ARM_CONFIG")
        .unwrap_or_else(|_| "config/arm_6dof.toml".to_string());

    println!("Loading arm_config from: {}", config_path);

    let arm_config = ArmConfig::load_from_file(&config_path)?;
    arm_config.validate()?;

    println!("Loaded {} configuration with {} DOF", arm_config.name, arm_config.dof);

    let mut controller = ArmController::new(arm_config)?;

    println!("ArmController initialized successfully");
    println!("Waiting for arm commands from dispatcher-keyboard...");

    let mut event_count = 0;

    while let Some(event) = events.recv() {
        event_count += 1;
        println!("Event #{}: Received event", event_count);

        match event {
            Event::Input { id, metadata: _, data } => {
                println!("Input event - ID: '{}', Data size: {} bytes", id.as_str(), data.len());

                match id.as_str() {
                    "arm_command" => {
                        println!("Processing arm command from dispatcher-keyboard...");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<ArmCommandWithMetadata>(bytes) {
                                    Ok(cmd_with_metadata) => {
                                        if let Some(ref command) = cmd_with_metadata.command {
                                            println!("Received ARM command: {:?}", command);
                                            println!("Command metadata: source={:?}, priority={:?}",
                                                     cmd_with_metadata.metadata.source,
                                                     cmd_with_metadata.metadata.priority);

                                            // Execute the command
                                            controller.execute_command(command.clone())?;

                                            // Forward command to simulation interface
                                            let serialized = serde_json::to_vec(&cmd_with_metadata)?;
                                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                            node.send_output(
                                                output_id.clone(),
                                                Default::default(),
                                                arrow_data
                                            )?;

                                            println!("Processed and forwarded command to simulation");
                                        } else {
                                            println!("Received arm command metadata without command");
                                        }
                                    }
                                    Err(e) => {
                                        println!("Failed to deserialize arm command: {}", e);
                                        println!("Raw data: {}", String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        } else {
                            println!("Failed to parse arm command as binary array");
                        }
                    }
                    "joint_feedback" => {
                        println!("Processing joint feedback from simulation...");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(status) = serde_json::from_slice::<ArmStatus>(bytes) {
                                    debug!("Joint feedback: {} joints, moving: {}, homed: {}",
                                             status.joint_state.positions.len(),
                                             status.is_moving,
                                             status.is_homed
                                    );
                                    controller.update_current_state(status);
                                } else {
                                    println!("Failed to deserialize joint feedback");
                                }
                            }
                        }
                    }
                    _ => {
                        println!("Unknown input ID: '{}'", id.as_str());
                    }
                }
            }
            Event::Stop => {
                println!("Stop event received - shutting down arm controller");
                break;
            }
            other => {
                debug!("Other event type: {:?}", other);
            }
        }

        println!("Event #{} processed\n", event_count);
    }

    println!("Arm controller finished. Total events processed: {}", event_count);
    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        )
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ArmCommandWithMetadata {
    command: Option<ArmCommand>,
    metadata: CommandMetadata,
}

struct ArmController {
    config: ArmConfig,
    current_joint_positions: Vec<f64>,
    target_joint_positions: Vec<f64>,
    current_status: ArmStatus,
    current_target_command: Option<ArmCommand>,
    kinematics: ForwardKinematics,
}

impl ArmController {
    fn new(config: ArmConfig) -> Result<Self> {
        let dof = config.dof;
        let kinematics = ForwardKinematics::new(&config)?;

        Ok(Self {
            config,
            current_joint_positions: vec![0.0; dof],
            target_joint_positions: vec![0.0; dof],
            current_status: ArmStatus::new(dof),
            current_target_command: None,
            kinematics,
        })
    }

    fn execute_command(&mut self, command: ArmCommand) -> Result<()> {
        println!("Executing ARM command: {:?}", command);

        match &command {
            ArmCommand::JointPosition { joint_angles, .. } => {
                if joint_angles.len() != self.config.dof {
                    println!("Joint angles count doesn't match DOF");
                    return Ok(());
                }
                self.target_joint_positions = joint_angles.clone();
                println!("Set target joint positions: {:?}", self.target_joint_positions);
            }
            ArmCommand::CartesianMove { x, y, z, roll, pitch, yaw, .. } => {
                println!("Cartesian move: dx={:.3}, dy={:.3}, dz={:.3}, droll={:.3}, dpitch={:.3}, dyaw={:.3}",
                         x, y, z, roll, pitch, yaw);
                // In a real implementation, this would use inverse kinematics
                // For now, we'll simulate some joint movement
                info!("Cartesian movement command processed");
            }
            ArmCommand::RelativeMove { delta_joints } => {
                if delta_joints.len() != self.config.dof {
                    println!("Delta joints count doesn't match DOF");
                    return Ok(());
                }
                for (i, delta) in delta_joints.iter().enumerate() {
                    if i < self.target_joint_positions.len() {
                        self.target_joint_positions[i] += delta;
                        // Clamp to joint limits
                        let limits = &self.config.joint_limits[i];
                        self.target_joint_positions[i] = self.target_joint_positions[i]
                            .max(limits.min_angle)
                            .min(limits.max_angle);
                    }
                }
                println!("Applied relative joint movement: {:?}", delta_joints);
            }
            ArmCommand::Home => {
                self.target_joint_positions = vec![0.0; self.config.dof];
                println!("Moving to home position");
            }
            ArmCommand::Stop => {
                self.target_joint_positions = self.current_joint_positions.clone();
                println!("Stop command issued - holding current position");
            }
            ArmCommand::EmergencyStop => {
                self.target_joint_positions = self.current_joint_positions.clone();
                println!("EMERGENCY STOP - immediate halt");
            }
        }

        self.current_target_command = Some(command);
        Ok(())
    }

    fn update_current_state(&mut self, status: ArmStatus) {
        self.current_joint_positions = status.joint_state.positions.clone();
        self.current_status = status;
        debug!("Updated current state: {} joints", self.current_joint_positions.len());
    }
}