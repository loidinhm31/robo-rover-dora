use dora_node_api::{
    arrow::array::{Array, AsArray, BinaryArray},
    dora_core::config::DataId,
    DoraNode,
    Event
};
use eyre::Result;
use robo_rover_lib::{
    ArmCommand, CommandMetadata, CommandPriority,
    InputSource, RoverCommand
};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid;

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting dispatcher keyboard - dispatcher node");

    let (mut node, mut events) = DoraNode::init_from_env()?;

    // Output channels for different controllers
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());

    let mut dispatcher = KeyboardDispatcher::new();

    println!("Dispatcher keyboard initialized");
    println!("Available commands:");
    println!("  ROVER control (w,a,s,d,q,r):");
    println!("    w/s - Throttle forward/backward");
    println!("    a/d - Steer left/right");
    println!("    q - Brake");
    println!("    r - Reset to stopped state");
    println!("  ARM control (j,k,l,i,u,o,h,space):");
    println!("    k/j - Move X axis forward/backward");
    println!("    l/i - Move Y axis right/left");
    println!("    u/o - Move Z axis up/down");
    println!("    h - Return to home position");
    println!("    space - Stop arm movement");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, metadata: _, data } => {
                if id.as_str() == "keyboard" {
                    if let Some(string_array) = data.as_string_opt::<i32>() {
                        if string_array.len() > 0 {
                            let char_data = string_array.value(0);
                            let trimmed_char = char_data.trim();

                            println!("Processing keyboard input: '{}'", trimmed_char);

                            // Process the keyboard input and get commands
                            let commands = dispatcher.process_input(trimmed_char);

                            // Send commands to appropriate controllers
                            for command in commands {
                                match command {
                                    DispatchedCommand::Arm(arm_cmd, metadata) => {
                                        let cmd_with_metadata = ArmCommandWithMetadata {
                                            command: Some(arm_cmd.clone()),
                                            metadata,
                                        };

                                        let serialized = serde_json::to_vec(&cmd_with_metadata)?;
                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                        node.send_output(
                                            arm_command_output.clone(),
                                            Default::default(),
                                            arrow_data
                                        )?;

                                        println!("Sent ARM command: {:?}", arm_cmd);
                                    }
                                    DispatchedCommand::Rover(rover_cmd, metadata) => {
                                        let cmd_with_metadata = RoverCommandWithMetadata {
                                            command: rover_cmd.clone(),
                                            metadata,
                                        };

                                        let serialized = serde_json::to_vec(&cmd_with_metadata)?;
                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                        node.send_output(
                                            rover_command_output.clone(),
                                            Default::default(),
                                            arrow_data
                                        )?;

                                        println!("Sent ROVER command: throttle={:.2}, brake={:.2}, steer={:.2}",
                                                 rover_cmd.throttle, rover_cmd.brake, rover_cmd.steering_angle);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Event::Stop => {
                println!("Stop event received - shutting down dispatcher keyboard");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ArmCommandWithMetadata {
    command: Option<ArmCommand>,
    metadata: CommandMetadata,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}

#[derive(Debug)]
enum DispatchedCommand {
    Arm(ArmCommand, CommandMetadata),
    Rover(RoverCommand, CommandMetadata),
}

struct KeyboardDispatcher {
    move_scale: f64,
    steer_scale: f64,
    throttle_scale: f64,
    current_rover_state: RoverState,
}

#[derive(Debug, Clone)]
struct RoverState {
    throttle: f64,
    brake: f64,
    steering_angle: f64,
    is_reverse: bool,  // Track if we're in reverse mode
}

impl Default for RoverState {
    fn default() -> Self {
        Self {
            throttle: 0.0,
            brake: 0.0,
            steering_angle: 0.0,
            is_reverse: false,
        }
    }
}

impl KeyboardDispatcher {
    fn new() -> Self {
        Self {
            move_scale: 0.01,        // 1cm for arm movements
            steer_scale: 5.0,        // 5 degrees for steering
            throttle_scale: 0.2,     // 20% throttle increment
            current_rover_state: RoverState::default(),
        }
    }

    fn process_input(&mut self, input: &str) -> Vec<DispatchedCommand> {
        let mut commands = Vec::new();

        match input.to_lowercase().as_str() {
            // ROVER CONTROLS (w,a,s,d,q,r)
            "w" => {
                // Throttle forward (clear reverse and increase forward throttle)
                self.current_rover_state.brake = 0.0;
                self.current_rover_state.is_reverse = false;
                self.current_rover_state.throttle = (self.current_rover_state.throttle + self.throttle_scale).min(1.0);
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                println!("ROVER: Throttle forward ({:.2})", self.current_rover_state.throttle);
            }
            "s" => {
                // Throttle backward (Unity uses negative throttle for reverse)
                self.current_rover_state.throttle = (self.current_rover_state.throttle - self.throttle_scale).max(-1.0);
                self.current_rover_state.brake = 0.0;  // Clear brakes for movement
                self.current_rover_state.is_reverse = self.current_rover_state.throttle < 0.0;
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                if self.current_rover_state.is_reverse {
                    println!("ROVER: Reverse movement (throttle: {:.2})", self.current_rover_state.throttle);
                } else {
                    println!("ROVER: Slowing down (throttle: {:.2})", self.current_rover_state.throttle);
                }
            }
            "a" => {
                // Steer left (positive steering angle)
                self.current_rover_state.steering_angle = (self.current_rover_state.steering_angle + self.steer_scale).min(15.0);
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                println!("ROVER: Steer left ({:.1} degrees)", self.current_rover_state.steering_angle);
            }
            "d" => {
                // Steer right (negative steering angle)
                self.current_rover_state.steering_angle = (self.current_rover_state.steering_angle - self.steer_scale).max(-15.0);
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                println!("ROVER: Steer right ({:.1} degrees)", self.current_rover_state.steering_angle);
            }
            "q" => {
                // Emergency brake (stop everything)
                self.current_rover_state.throttle = 0.0;
                self.current_rover_state.brake = 1.0;
                self.current_rover_state.is_reverse = false;
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                println!("ROVER: Emergency brake applied");
            }
            "r" => {
                // Reset rover to stopped state
                self.current_rover_state = RoverState::default();
                let rover_cmd = self.create_rover_command();
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                println!("ROVER: Reset to stopped state");
            }

            // ARM CONTROLS (j,k,l,i,u,o,h) - matching original directional behavior
            "k" => {
                // Move X axis forward (like original 'w')
                let arm_cmd = ArmCommand::CartesianMove {
                    x: self.move_scale,
                    y: 0.0,
                    z: 0.0,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move X axis forward ({:.3} m)", self.move_scale);
            }
            "j" => {
                // Move X axis backward (like original 's')
                let arm_cmd = ArmCommand::CartesianMove {
                    x: -self.move_scale,
                    y: 0.0,
                    z: 0.0,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move X axis backward ({:.3} m)", -self.move_scale);
            }
            "i" => {
                // Move Y axis left (like original 'a')
                let arm_cmd = ArmCommand::CartesianMove {
                    x: 0.0,
                    y: -self.move_scale,
                    z: 0.0,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move Y axis left ({:.3} m)", -self.move_scale);
            }
            "l" => {
                // Move Y axis right (like original 'd')
                let arm_cmd = ArmCommand::CartesianMove {
                    x: 0.0,
                    y: self.move_scale,
                    z: 0.0,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move Y axis right ({:.3} m)", self.move_scale);
            }
            "u" => {
                // Move Z axis up
                let arm_cmd = ArmCommand::CartesianMove {
                    x: 0.0,
                    y: 0.0,
                    z: self.move_scale,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move Z axis up ({:.3} m)", self.move_scale);
            }
            "o" => {
                // Move Z axis down
                let arm_cmd = ArmCommand::CartesianMove {
                    x: 0.0,
                    y: 0.0,
                    z: -self.move_scale,
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Move Z axis down ({:.3} m)", -self.move_scale);
            }
            "h" | "home" => {
                // Return to home position
                let arm_cmd = ArmCommand::JointPosition {
                    joint_angles: vec![0.0; 6], // Assuming 6 DOF arm
                    max_velocity: None,
                };
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Return to home position");
            }
            " " | "space" => {
                // Stop arm movement
                let arm_cmd = ArmCommand::Stop;
                let metadata = self.create_metadata();
                commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                println!("ARM: Stop movement");
            }

            _ => {
                println!("Unknown command: '{}'. Use w,a,s,d,q,r for rover or k,j,i,l,u,o,h,space for arm", input);
            }
        }

        commands
    }

    fn create_rover_command(&self) -> RoverCommand {
        // Unity uses negative throttle for reverse movement, so preserve the sign
        RoverCommand {
            throttle: self.current_rover_state.throttle.clamp(-1.0, 1.0), // Allow negative for reverse
            brake: self.current_rover_state.brake.clamp(0.0, 1.0),
            steering_angle: self.current_rover_state.steering_angle.clamp(-15.0, 15.0),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            command_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn create_metadata(&self) -> CommandMetadata {
        CommandMetadata {
            command_id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            source: InputSource::Local,
            priority: CommandPriority::Normal,
        }
    }
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