use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::arrow::datatypes::GenericBinaryType;
use dora_node_api::{DoraNode, Event};
use eyre::Result;
use robo_rover_lib::{ArmCommand, ArmCommandWithMetadata, ArmTelemetry, RoverTelemetry};
use std::collections::HashMap;
use std::error::Error;
use tracing::{debug, info, warn};

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    info!("Starting enhanced monitor node with detailed debugging...");

    let (_node, mut events) = DoraNode::init_from_env()?;

    info!("Monitor node initialized successfully");
    info!("Monitoring all dataflow activity with enhanced logging...");

    let mut event_count = 0;
    let mut input_stats: HashMap<String, usize> = HashMap::new();

    while let Some(event) = events.recv() {
        event_count += 1;
        debug!("Event #{}: New event received", event_count);

        match event {
            Event::Input { id, metadata, data } => {
                let input_id = id.as_str();
                *input_stats.entry(input_id.to_string()).or_insert(0) += 1;

                let data_len = data.len();
                debug!("INPUT EVENT:");
                debug!("   ID: '{}'", input_id);
                debug!("   Data length: {} bytes", data_len);
                debug!("   Count for this input: {}", input_stats[input_id]);
                debug!("   Metadata: {:?}", metadata);
                debug!("   Data type: {:?}", data.data_type());

                // Process specific input types
                match input_id {
                    "keyboard" => {
                        debug!("   KEYBOARD INPUT DETAILS:");

                        if let Some(string_array) = data.as_string_opt::<i32>() {
                            if string_array.len() > 0 {
                                let char_data = string_array.value(0);
                                let trimmed = char_data.trim();
                                info!("Keyboard character: '{}'", trimmed);
                                debug!("      Length: {} chars", trimmed.len());
                                debug!("      Raw string: {:?}", char_data);
                                debug!(
                                    "      ASCII values: {:?}",
                                    trimmed.chars().map(|c| c as u32).collect::<Vec<_>>()
                                );

                                // Check for special characters
                                match trimmed {
                                    " " => debug!("      Detected: SPACE character"),
                                    "w" | "a" | "s" | "d" | "q" | "e" => {
                                        info!("      Detected: Movement command '{}'", trimmed);
                                    }
                                    "home" => info!("      Detected: HOME command"),
                                    _ => debug!("      Unknown/unmapped character"),
                                }
                            } else {
                                warn!("      Empty keyboard input array");
                            }
                        } else {
                            warn!("      Failed to parse keyboard input as string array");
                        }
                    }

                    "dispatcher_arm" | "web_arm" => {
                        info!("   ARM COMMAND INPUT:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<ArmCommandWithMetadata>(bytes) {
                                    Ok(cmd_with_metadata) => {
                                        info!(
                                            "      Command ID: {}",
                                            cmd_with_metadata.metadata.command_id
                                        );
                                        info!(
                                            "      Source: {:?}",
                                            cmd_with_metadata.metadata.source
                                        );
                                        info!(
                                            "      Priority: {:?}",
                                            cmd_with_metadata.metadata.priority
                                        );
                                        if let Some(command) = &cmd_with_metadata.command {
                                            match command {
                                                ArmCommand::JointPosition {
                                                    joint_angles,
                                                    max_velocity,
                                                } => {
                                                    info!("      Type: JointPosition");
                                                    info!("      Joint angles: {:?}", joint_angles);
                                                    info!("      Max velocity: {:?}", max_velocity);
                                                }
                                                ArmCommand::CartesianMove {
                                                    x,
                                                    y,
                                                    z,
                                                    roll,
                                                    pitch,
                                                    yaw,
                                                    max_velocity,
                                                } => {
                                                    info!("      Type: CartesianMove");
                                                    info!(
                                                        "      Position: ({:.3}, {:.3}, {:.3})",
                                                        x, y, z
                                                    );
                                                    info!(
                                                        "      Orientation: ({:.3}, {:.3}, {:.3})",
                                                        roll, pitch, yaw
                                                    );
                                                    info!("      Max velocity: {:?}", max_velocity);
                                                }
                                                ArmCommand::RelativeMove { delta_joints } => {
                                                    info!("      Type: RelativeMove");
                                                    info!("      Delta joints: {:?}", delta_joints);
                                                }
                                                ArmCommand::Stop => {
                                                    info!("      Type: Stop");
                                                }
                                                ArmCommand::Home => {
                                                    info!("      Type: Home");
                                                }
                                                ArmCommand::EmergencyStop => {
                                                    warn!("      Type: EmergencyStop");
                                                }
                                            }
                                        } else {
                                            warn!("      No command in metadata wrapper");
                                        }
                                    }
                                    Err(e) => {
                                        warn!("      Failed to parse arm command: {}", e);
                                        debug!(
                                            "      Raw bytes: {:?}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            } else {
                                warn!("      Empty arm command array");
                            }
                        } else {
                            warn!("      Failed to parse arm command as binary array");
                        }
                    }

                    "arm_processed" => {
                        info!("   PROCESSED ARM COMMAND:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<ArmCommand>(bytes) {
                                    Ok(command) => {
                                        info!("      Processed command: {:?}", command);
                                    }
                                    Err(e) => {
                                        warn!("      Failed to parse processed arm command: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    "arm_telemetry" => {
                        info!("   ARM TELEMETRY INPUT:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<ArmTelemetry>(bytes) {
                                    Ok(telemetry) => {
                                        info!("      ARM TELEMETRY DATA:");
                                        info!("         Source: {}", telemetry.source);
                                        info!("         Timestamp: {}", telemetry.timestamp);
                                        info!("         Is moving: {}", telemetry.is_moving);
                                        info!("         End effector pose: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                              telemetry.end_effector_pose[0], telemetry.end_effector_pose[1],
                                              telemetry.end_effector_pose[2], telemetry.end_effector_pose[3],
                                              telemetry.end_effector_pose[4], telemetry.end_effector_pose[5]);

                                        if let Some(ref joint_angles) = telemetry.joint_angles {
                                            info!(
                                                "         Joint angles: [{}]",
                                                joint_angles
                                                    .iter()
                                                    .map(|x| format!("{:.3}", x))
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            );
                                        } else {
                                            debug!("         Joint angles: None");
                                        }

                                        if let Some(ref joint_velocities) =
                                            telemetry.joint_velocities
                                        {
                                            debug!(
                                                "         Joint velocities: [{}]",
                                                joint_velocities
                                                    .iter()
                                                    .map(|x| format!("{:.3}", x))
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            );
                                        } else {
                                            debug!("         Joint velocities: None");
                                        }

                                        // Additional analysis
                                        let position_magnitude = (telemetry.end_effector_pose[0]
                                            .powi(2)
                                            + telemetry.end_effector_pose[1].powi(2)
                                            + telemetry.end_effector_pose[2].powi(2))
                                        .sqrt();
                                        debug!(
                                            "         Position magnitude: {:.3}",
                                            position_magnitude
                                        );

                                        if let Some(ref joint_angles) = telemetry.joint_angles {
                                            let max_joint_angle = joint_angles
                                                .iter()
                                                .fold(0.0_f64, |a, &b| a.max(b.abs()));
                                            debug!(
                                                "         Max joint angle: {:.3} rad ({:.1}°)",
                                                max_joint_angle,
                                                max_joint_angle.to_degrees()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!("      Failed to parse arm telemetry: {}", e);
                                        debug!(
                                            "      Raw string attempt: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            } else {
                                warn!("      Empty arm telemetry array");
                            }
                        } else {
                            warn!("      Failed to parse arm telemetry as binary array");
                        }
                    }

                    "joint_feedback" => {
                        debug!("   JOINT FEEDBACK INPUT:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<ArmTelemetry>(bytes) {
                                    Ok(feedback) => {
                                        debug!("      Joint feedback from Unity");
                                        debug!("      Source: {}", feedback.source);
                                        if let Some(ref joint_angles) = feedback.joint_angles {
                                            debug!(
                                                "      Joint positions: [{}]",
                                                joint_angles
                                                    .iter()
                                                    .map(|x| format!("{:.3}", x))
                                                    .collect::<Vec<_>>()
                                                    .join(", ")
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        warn!("      Failed to parse joint feedback: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    "rover_telemetry" => {
                        debug!("   ROVER TELEMETRY INPUT:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                match serde_json::from_slice::<RoverTelemetry>(bytes) {
                                    Ok(telemetry) => {
                                        info!(
                                            "      Rover position: ({:.2}, {:.2})",
                                            telemetry.position.0, telemetry.position.1
                                        );
                                        info!(
                                            "      Rover yaw: {:.2} rad ({:.1}°)",
                                            telemetry.yaw,
                                            telemetry.yaw.to_degrees()
                                        );
                                        info!("      Rover velocity: {:.2}", telemetry.velocity);
                                    }
                                    Err(e) => {
                                        warn!("      Failed to parse rover telemetry: {}", e);
                                        debug!(
                                            "      Raw string attempt: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            }
                        } else {
                            warn!("      Failed to parse rover_telemetry as binary array");
                        }
                    }

                    "dispatcher_rover" | "web_rover" => {
                        debug!("   ROVER COMMAND INPUT:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                debug!("      Rover command data size: {} bytes", bytes.len());
                            }
                        }
                    }

                    "rover_processed" => {
                        debug!("   PROCESSED ROVER COMMAND:");
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);
                                debug!("      Processed rover command size: {} bytes", bytes.len());
                            }
                        }
                    }

                    _ => {
                        debug!("   UNKNOWN INPUT TYPE: '{}'", input_id);
                        debug!("      Data type: {:?}", data.data_type());
                    }
                }

                debug!("    Input processing complete");
            }

            Event::Stop(_) => {
                info!("STOP EVENT RECEIVED");
                info!("Final statistics:");
                info!("   Total events processed: {}", event_count);
                info!("   Input breakdown:");
                for (input_id, count) in &input_stats {
                    info!("      {}: {} events", input_id, count);
                }
                info!("Monitor node stopping gracefully");
                break;
            }

            other_event => {
                debug!("   OTHER EVENT TYPE: {:?}", other_event);
            }
        }
    }

    info!(
        "Monitor node finished after processing {} events",
        event_count
    );
    Ok(())
}

fn init_tracing() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .finish();

    tracing::subscriber::set_default(subscriber)
}
