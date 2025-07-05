use dora_node_api::arrow::array::Array;
use dora_node_api::{arrow::array::BinaryArray, dora_core::config::DataId, DoraNode, Event};
use eyre::Result;
use robo_rover_lib::{CommandMetadata, RoverCommand, RoverTelemetry};
use std::collections::HashMap;
use std::error::Error;
use tracing::{debug, info, warn};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    info!("Starting rover controller node...");

    let (mut node, mut events) = DoraNode::init_from_env()?;
    let output_id = DataId::from("processed_rover_command".to_owned());

    info!("Rover controller initialized successfully");
    info!("Ready to process rover commands (supporting reverse movement with negative throttle)");

    let mut rover_controller = RoverController::new();
    let mut event_count = 0;
    let mut input_stats: HashMap<String, usize> = HashMap::new();

    while let Some(event) = events.recv() {
        event_count += 1;
        info!("Event #{}: {:?}", event_count, event);

        match event {
            Event::Input {
                id,
                metadata: _,
                data,
            } => {
                let count = input_stats.entry(id.as_str().to_string()).or_insert(0);
                *count += 1;

                match id.as_str() {
                    "rover_command" => {
                        if let Some(array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if array.len() > 0 {
                                let bytes = array.value(0);

                                match serde_json::from_slice::<RoverCommandWithMetadata>(bytes) {
                                    Ok(cmd_with_metadata) => {
                                        if let Some(command) = cmd_with_metadata.command {
                                            info!("Received rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                                                 command.throttle,
                                                 command.brake,
                                                 command.steering_angle);

                                            match rover_controller.process_command(command) {
                                                Ok(_) => {
                                                    let processed_cmd = rover_controller.get_processed_command();
                                                    let serialized = serde_json::to_vec(&processed_cmd)?;
                                                    let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                                                    node.send_output(
                                                        output_id.clone(),
                                                        Default::default(),
                                                        arrow_data
                                                    )?;

                                                    info!("Sent processed rover command");
                                                }
                                                Err(e) => {
                                                    warn!("Failed to execute rover command: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse rover command: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    "rover_telemetry" => {
                        if let Some(array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if array.len() > 0 {
                                let bytes = array.value(0);
                                match serde_json::from_slice::<RoverTelemetry>(bytes) {
                                    Ok(telemetry) => {
                                        debug!("Received rover telemetry: pos=({:.2}, {:.2}), vel={:.2}",
                                               telemetry.position.0, telemetry.position.1, telemetry.velocity);
                                        rover_controller.update_telemetry(telemetry);
                                    }
                                    Err(e) => {
                                        warn!("Failed to deserialize rover telemetry: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        info!("Unknown input ID: '{}'", id.as_str());
                    }
                }
            }
            Event::Stop(_) => {
                info!("\nSTOP EVENT RECEIVED");
                info!("Final statistics:");
                info!("   Total events processed: {}", event_count);
                info!("   Input breakdown:");
                for (input_id, count) in &input_stats {
                    info!("      {}: {} events", input_id, count);
                }
                info!("Rover controller stopping gracefully");
                break;
            }

            other_event => {
                debug!("Other event type: {:?}", other_event);
            }
        }
    }

    info!(
        "Rover controller finished after processing {} events",
        event_count
    );
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: Option<RoverCommand>,
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
            max_throttle: 1.0,        // 100% throttle (forward)
            max_steering_angle: 15.0, // 15 degrees
            max_velocity: 5.0,        // 5 m/s max speed
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

    fn process_command(&mut self, mut command: RoverCommand) -> Result<()> {
        info!(
            "Executing ROVER command: throttle={:.2}, brake={:.2}, steer={:.2}°",
            command.throttle, command.brake, command.steering_angle
        );

        // Apply safety limits
        command.throttle = command.throttle.clamp(
            -self.safety_limits.max_throttle, // NEGATIVE THROTTLE for reverse movement
            self.safety_limits.max_throttle,
        );
        command.brake = command.brake.clamp(0.0, 1.0);
        command.steering_angle = command.steering_angle.clamp(
            -self.safety_limits.max_steering_angle,
            self.safety_limits.max_steering_angle,
        );

        // Check velocity safety limit
        if let Some(ref telemetry) = self.last_telemetry {
            if telemetry.velocity.abs() > self.safety_limits.max_velocity {
                info!(
                    "SAFETY: Velocity {:.2} exceeds limit {:.2}, applying brakes",
                    telemetry.velocity, self.safety_limits.max_velocity
                );
                command.throttle = 0.0;
                command.brake = 1.0;
            }
        }

        info!(
            "Final processed command: throttle={:.2}, brake={:.2}, steer={:.2}°",
            command.throttle, command.brake, command.steering_angle
        );

        // Store command
        self.command_history.push(command.clone());
        self.current_command = Some(command);

        // Keep only last 10 commands in history
        if self.command_history.len() > 10 {
            self.command_history.remove(0);
        }

        Ok(())
    }

    fn update_telemetry(&mut self, telemetry: RoverTelemetry) {
        debug!(
            "Updated rover telemetry: pos=({:.2}, {:.2}), vel={:.2}",
            telemetry.position.0, telemetry.position.1, telemetry.velocity
        );
        self.last_telemetry = Some(telemetry);
    }

    fn get_processed_command(&self) -> RoverCommandWithMetadata {
        let command = self
            .current_command
            .clone()
            .unwrap_or_else(|| RoverCommand {
                throttle: 0.0,
                brake: 0.0,
                steering_angle: 0.0,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                command_id: uuid::Uuid::new_v4().to_string(),
            });

        RoverCommandWithMetadata {
            command: Some(command),
            metadata: CommandMetadata {
                command_id: uuid::Uuid::new_v4().to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                source: robo_rover_lib::InputSource::Keyboard,
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
