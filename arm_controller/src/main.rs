use dora_node_api::{DoraNode, Event, dora_core::config::DataId, arrow::array::BinaryArray, arrow::array::types::GenericBinaryType};
use dora_node_api::arrow::array::{Array, AsArray};
use robo_rover_lib::{ArmCommand, ArmStatus, ArmConfig, CommandPriority, CommandMetadata, InputSource, ForwardKinematics};
use eyre::Result;
use std::error::Error;
use tracing::{info, warn, debug};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Initializing arm_controller node...");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("arm_command".to_owned());

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
    println!("Starting event loop - waiting for events...");

    let mut event_count = 0;

    // Enhanced event loop with debugging
    while let Some(event) = events.recv() {
        event_count += 1;
        println!("📨 Event #{}: Received event", event_count);

        match event {
            Event::Input { id, metadata: _, data } => {
                println!("Input event - ID: '{}', Data size: {} bytes", id.as_str(), data.len());

                match id.as_str() {
                    "keyboard" => {
                        println!("⌨️  Processing keyboard input...");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            println!("Bytes array length: {}", bytes_array.len());

                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                println!("Raw bytes: {:?}", bytes);

                                if let Ok(char_data) = std::str::from_utf8(bytes) {
                                    let trimmed_char = char_data.trim();
                                    println!("Parsed character: '{}'", trimmed_char);

                                    if let Some(command) = controller.map_char_to_command(trimmed_char) {
                                        println!("Mapped to command: {:?}", command);

                                        controller.execute_command(command.clone())?;

                                        // Send command to simulation
                                        let cmd_with_metadata = CommandWithMetadata {
                                            command: Some(command),
                                            metadata: CommandMetadata {
                                                command_id: uuid::Uuid::new_v4().to_string(),
                                                timestamp: std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)?
                                                    .as_millis() as u64,
                                                source: InputSource::Local,
                                                priority: CommandPriority::Normal,
                                            }
                                        };

                                        let serialized = serde_json::to_vec(&cmd_with_metadata)?;
                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                        node.send_output(
                                            output_id.clone(),
                                            Default::default(),
                                            arrow_data
                                        )?;

                                        println!("Sent command to output: arm_command");
                                    } else {
                                        println!("No command mapping for character: '{}'", trimmed_char);
                                    }
                                } else {
                                    println!("Failed to parse UTF-8 from bytes: {:?}", bytes);
                                }
                            } else {
                                println!("Empty bytes array");
                            }
                        } else {
                            println!("❌ Failed to parse as bytes array");
                        }
                    }
                    "joint_feedback" => {
                        println!("Processing joint feedback...");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                if let Ok(status) = serde_json::from_slice::<ArmStatus>(bytes) {
                                    println!("📊 Joint feedback: {} joints, moving: {}, homed: {}",
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
                println!("Other event type: {:?}", other);
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
            std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string())
        )
        .finish();

    tracing::subscriber::set_default(subscriber)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CommandWithMetadata {
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

    fn map_char_to_command(&mut self, char_input: &str) -> Option<ArmCommand> {
        let move_scale = 0.01;

        let command = match char_input.to_lowercase().as_str() {
            "w" => Some(ArmCommand::CartesianMove {
                x: move_scale, y: 0.0, z: 0.0, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "s" => Some(ArmCommand::CartesianMove {
                x: -move_scale, y: 0.0, z: 0.0, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "a" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: move_scale, z: 0.0, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "d" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: -move_scale, z: 0.0, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "q" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: 0.0, z: move_scale, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "e" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: 0.0, z: -move_scale, roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            " " | "space" => Some(ArmCommand::Stop),
            "home" => Some(ArmCommand::Home),
            _ => None,
        };

        if let Some(ref cmd) = command {
            println!("Mapped char '{}' to command: {:?}", char_input, cmd);
        } else {
            println!("No mapping found for char '{}'", char_input);
        }

        command
    }

    fn execute_command(&mut self, command: ArmCommand) -> Result<()> {
        println!("Executing command: {:?}", command);

        match &command {
            ArmCommand::JointPosition { joint_angles, .. } => {
                if joint_angles.len() != self.config.dof {
                    println!("❌ Joint angles count doesn't match DOF");
                    return Ok(());
                }
                self.target_joint_positions = joint_angles.clone();
                println!("Set target joint positions: {:?}", self.target_joint_positions);
            }
            ArmCommand::Home => {
                self.target_joint_positions = vec![0.0; self.config.dof];
                println!("Moving to home position");
            }
            ArmCommand::Stop => {
                self.target_joint_positions = self.current_joint_positions.clone();
                println!("Stop command issued");
            }
            _ => {
                info!("Other command executed: {:?}", command);
            }
        }

        self.current_target_command = Some(command);
        Ok(())
    }

    fn update_current_state(&mut self, status: ArmStatus) {
        self.current_joint_positions = status.joint_state.positions.clone();
        self.current_status = status;
        println!("Updated current state: {} joints", self.current_joint_positions.len());
    }
}