use robo_rover_lib::{RoverCommand, RoverTelemetry, CommandMetadata};
use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::{DoraNode, Event, dora_core::config::DataId, arrow::array::BinaryArray};
use eyre::Result;
use std::collections::HashMap;
use std::error::Error;
use dora_node_api::arrow::datatypes::GenericBinaryType;
use tracing::{info, warn, debug};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting rover controller node...");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("processed_rover_command".to_owned());

    println!("Rover controller initialized successfully");
    println!("Waiting for rover commands from interactive-keyboard...");

    let mut controller = RoverController::new();
    let mut event_count = 0;
    let mut input_stats: HashMap<String, usize> = HashMap::new();

    while let Some(event) = events.recv() {
        event_count += 1;
        println!("Event #{}: New event received", event_count);

        match event {
            Event::Input { id, metadata, data } => {
                let input_id = id.as_str();
                *input_stats.entry(input_id.to_string()).or_insert(0) += 1;

                let data_len = data.len();
                println!("INPUT EVENT:");
                println!("   ID: '{}'", input_id);
                println!("   Data length: {} bytes", data_len);
                println!("   Count for this input: {}", input_stats[input_id]);

                match input_id {
                    "rover_command" => {
                        println!("   ROVER COMMAND from interactive-keyboard:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<RoverCommandWithMetadata>(bytes) {
                                    Ok(cmd_with_metadata) => {
                                        let rover_cmd = &cmd_with_metadata.command;

                                        println!("      Successfully parsed rover command:");
                                        println!("      Throttle: {:.3}", rover_cmd.throttle);
                                        println!("      Brake: {:.3}", rover_cmd.brake);
                                        println!("      Steering: {:.3}°", rover_cmd.steering_angle);
                                        println!("      Command ID: {}", cmd_with_metadata.metadata.command_id);
                                        println!("      Source: {:?}", cmd_with_metadata.metadata.source);
                                        println!("      Priority: {:?}", cmd_with_metadata.metadata.priority);

                                        // Process the command
                                        controller.execute_command(rover_cmd.clone())?;

                                        // Forward processed command to simulation
                                        let processed_cmd = controller.get_processed_command();
                                        let serialized = serde_json::to_vec(&processed_cmd)?;
                                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                        node.send_output(
                                            output_id.clone(),
                                            Default::default(),
                                            arrow_data
                                        )?;

                                        println!("      Forwarded processed command to simulation interface");
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse rover command: {}", e);
                                        println!("      Raw string: {}", String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        } else {
                            println!("      Failed to parse rover_command as binary array");
                        }
                    }

                    "rover_telemetry" => {
                        println!("   ROVER TELEMETRY from simulation:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<RoverTelemetry>(bytes) {
                                    Ok(telemetry) => {
                                        println!("      Position: ({:.2}, {:.2})", telemetry.position.0, telemetry.position.1);
                                        println!("      Velocity: {:.2} m/s", telemetry.velocity);
                                        println!("      Orientation: yaw={:.1}°, pitch={:.1}°, roll={:.1}°",
                                                 telemetry.yaw.to_degrees(),
                                                 telemetry.pitch.to_degrees(),
                                                 telemetry.roll.to_degrees());
                                        println!("      Near sample: {}", telemetry.near_sample);
                                        println!("      Picking up: {}", telemetry.picking_up);

                                        // Update controller state with telemetry
                                        controller.update_telemetry(telemetry);
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse rover telemetry: {}", e);
                                        println!("      Raw string: {}", String::from_utf8_lossy(bytes));
                                    }
                                }
                            }
                        } else {
                            println!("      Failed to parse rover_telemetry as binary array");
                        }
                    }

                    _ => {
                        println!("   UNKNOWN INPUT TYPE: '{}'", input_id);
                        println!("      Data type: {:?}", data.data_type());
                    }
                }

                println!("    Input processing complete");
            }

            Event::Stop => {
                println!("\nSTOP EVENT RECEIVED");
                println!("Final statistics:");
                println!("   Total events processed: {}", event_count);
                println!("   Input breakdown:");
                for (input_id, count) in &input_stats {
                    println!("      {}: {} events", input_id, count);
                }
                println!("Rover controller stopping gracefully");
                break;
            }

            other_event => {
                debug!("Other event type: {:?}", other_event);
            }
        }
    }

    println!("Rover controller finished after processing {} events", event_count);
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}

struct RoverController {
    current_command: Option<RoverCommand>,
    last_telemetry: Option<RoverTelemetry>,
    safety_limits: SafetyLimits,
    command_history: Vec<RoverCommand>,
}

#[derive(Debug)]
struct SafetyLimits {
    max_throttle: f64,
    max_steering_angle: f64,
    max_velocity: f64,
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_throttle: 1.0,      // 100% throttle
            max_steering_angle: 15.0, // 15 degrees
            max_velocity: 5.0,       // 5 m/s max speed
        }
    }
}

impl RoverController {
    fn new() -> Self {
        Self {
            current_command: None,
            last_telemetry: None,
            safety_limits: SafetyLimits::default(),
            command_history: Vec::new(),
        }
    }

    fn execute_command(&mut self, mut command: RoverCommand) -> Result<()> {
        println!("Executing ROVER command: throttle={:.2}, brake={:.2}, steer={:.2}°",
                 command.throttle, command.brake, command.steering_angle);

        // Apply safety limits
        command.throttle = command.throttle.clamp(0.0, self.safety_limits.max_throttle);
        command.brake = command.brake.clamp(0.0, 1.0);
        command.steering_angle = command.steering_angle.clamp(
            -self.safety_limits.max_steering_angle,
            self.safety_limits.max_steering_angle
        );

        // Check velocity safety limit
        if let Some(ref telemetry) = self.last_telemetry {
            if telemetry.velocity.abs() > self.safety_limits.max_velocity {
                println!("SAFETY: Velocity {:.2} exceeds limit {:.2}, applying brakes",
                         telemetry.velocity, self.safety_limits.max_velocity);
                command.throttle = 0.0;
                command.brake = 1.0;
            }
        }

        // Store command
        self.current_command = Some(command.clone());
        self.command_history.push(command);

        // Keep only last 10 commands in history
        if self.command_history.len() > 10 {
            self.command_history.remove(0);
        }

        Ok(())
    }

    fn update_telemetry(&mut self, telemetry: RoverTelemetry) {
        debug!("Updated rover telemetry: pos=({:.2}, {:.2}), vel={:.2}", 
               telemetry.position.0, telemetry.position.1, telemetry.velocity);
        self.last_telemetry = Some(telemetry);
    }

    fn get_processed_command(&self) -> RoverCommandWithMetadata {
        let command = self.current_command.clone()
            .unwrap_or_else(|| RoverCommand::new(0.0, 0.0, 0.0));

        RoverCommandWithMetadata {
            command,
            metadata: CommandMetadata {
                command_id: uuid::Uuid::new_v4().to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                source: robo_rover_lib::InputSource::Local,
                priority: robo_rover_lib::CommandPriority::Normal,
            },
        }
    }
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}