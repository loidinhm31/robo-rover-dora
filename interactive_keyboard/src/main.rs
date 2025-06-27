use dora_node_api::{
    arrow::array::{BinaryArray, Array, AsArray},
    dora_core::config::DataId,
    DoraNode,
    Event
};
use robo_rover_lib::{
    ArmCommand, RoverCommand, KeyboardInput, CommandMetadata,
    CommandPriority, InputSource
};
use eyre::Result;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
enum ControlMode {
    Arm,
    Rover,
}

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting interactive keyboard dispatcher node");

    let (mut node, mut events) = DoraNode::init_from_env()?;

    // Output channels for different controllers
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());

    let mut dispatcher = KeyboardDispatcher::new();

    println!("Interactive keyboard dispatcher initialized");
    println!("Available commands:");
    println!("  TAB - Switch between ARM and ROVER modes");
    println!("  ARM mode:");
    println!("    w/s - Move X axis forward/backward");
    println!("    a/d - Move Y axis left/right");
    println!("    q/e - Move Z axis up/down");
    println!("    space - Stop");
    println!("    home - Return to home position");
    println!("  ROVER mode:");
    println!("    w/s - Throttle forward/backward");
    println!("    a/d - Steer left/right");
    println!("    space - Brake");
    println!("    r - Reset to stopped state");

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
                println!("Stop event received - shutting down interactive keyboard dispatcher");
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
    current_mode: ControlMode,
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
}

impl Default for RoverState {
    fn default() -> Self {
        Self {
            throttle: 0.0,
            brake: 0.0,
            steering_angle: 0.0,
        }
    }
}

impl KeyboardDispatcher {
    fn new() -> Self {
        Self {
            current_mode: ControlMode::Rover,
            move_scale: 0.01,        // 1cm for arm movements
            steer_scale: 5.0,        // 5 degrees for steering
            throttle_scale: 0.2,     // 20% throttle increment
            current_rover_state: RoverState::default(),
        }
    }

    fn process_input(&mut self, input: &str) -> Vec<DispatchedCommand> {
        let mut commands = Vec::new();

        match input.to_lowercase().as_str() {
            // Mode switching
            "\t" | "tab" => {
                self.switch_mode();
                return commands; // No command to send, just mode switch
            }

            // Process commands based on current mode
            _ => {
                match self.current_mode {
                    ControlMode::Arm => {
                        if let Some(arm_cmd) = self.map_to_arm_command(input) {
                            let metadata = self.create_metadata();
                            commands.push(DispatchedCommand::Arm(arm_cmd, metadata));
                        }
                    }
                    ControlMode::Rover => {
                        if let Some(rover_cmd) = self.map_to_rover_command(input) {
                            let metadata = self.create_metadata();
                            commands.push(DispatchedCommand::Rover(rover_cmd, metadata));
                        }
                    }
                }
            }
        }

        commands
    }

    fn switch_mode(&mut self) {
        self.current_mode = match self.current_mode {
            ControlMode::Arm => {
                println!("Switched to ROVER control mode");
                ControlMode::Rover
            }
            ControlMode::Rover => {
                println!("Switched to ARM control mode");
                ControlMode::Arm
            }
        };
    }

    fn map_to_arm_command(&self, input: &str) -> Option<ArmCommand> {
        match input.to_lowercase().as_str() {
            "w" => Some(ArmCommand::CartesianMove {
                x: self.move_scale, y: 0.0, z: 0.0,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "s" => Some(ArmCommand::CartesianMove {
                x: -self.move_scale, y: 0.0, z: 0.0,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "a" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: self.move_scale, z: 0.0,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "d" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: -self.move_scale, z: 0.0,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "q" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: 0.0, z: self.move_scale,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            "e" => Some(ArmCommand::CartesianMove {
                x: 0.0, y: 0.0, z: -self.move_scale,
                roll: 0.0, pitch: 0.0, yaw: 0.0,
                max_velocity: None
            }),
            " " | "space" => Some(ArmCommand::Stop),
            "home" => Some(ArmCommand::Home),
            _ => {
                println!("Unknown ARM command: '{}'", input);
                None
            }
        }
    }

    fn map_to_rover_command(&mut self, input: &str) -> Option<RoverCommand> {
        match input.to_lowercase().as_str() {
            "w" => {
                // Increase forward throttle
                self.current_rover_state.throttle =
                    (self.current_rover_state.throttle + self.throttle_scale).min(1.0);
                self.current_rover_state.brake = 0.0;
                Some(self.create_rover_command())
            }
            "s" => {
                // Increase reverse throttle (negative throttle or brake)
                if self.current_rover_state.throttle > 0.0 {
                    // If moving forward, apply brakes first
                    self.current_rover_state.brake =
                        (self.current_rover_state.brake + self.throttle_scale).min(1.0);
                    self.current_rover_state.throttle =
                        (self.current_rover_state.throttle - self.throttle_scale).max(0.0);
                } else {
                    // Reverse throttle (implement as negative velocity handling)
                    self.current_rover_state.throttle =
                        (self.current_rover_state.throttle - self.throttle_scale).max(-1.0);
                    self.current_rover_state.brake = 0.0;
                }
                Some(self.create_rover_command())
            }
            "a" => {
                // Steer left
                self.current_rover_state.steering_angle =
                    (self.current_rover_state.steering_angle + self.steer_scale).min(15.0);
                Some(self.create_rover_command())
            }
            "d" => {
                // Steer right
                self.current_rover_state.steering_angle =
                    (self.current_rover_state.steering_angle - self.steer_scale).max(-15.0);
                Some(self.create_rover_command())
            }
            " " | "space" => {
                // Emergency brake
                self.current_rover_state.brake = 1.0;
                self.current_rover_state.throttle = 0.0;
                Some(self.create_rover_command())
            }
            "r" => {
                // Reset to stopped state
                self.current_rover_state = RoverState::default();
                Some(self.create_rover_command())
            }
            _ => {
                println!("Unknown ROVER command: '{}'", input);
                None
            }
        }
    }

    fn create_rover_command(&self) -> RoverCommand {
        RoverCommand::new(
            self.current_rover_state.throttle.abs(), // RoverCommand expects positive throttle
            self.current_rover_state.brake,
            self.current_rover_state.steering_angle
        )
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
        .finish();

    tracing::subscriber::set_default(subscriber)
}