use dora_node_api::arrow::array::{Array, AsArray};
use dora_node_api::arrow::datatypes::GenericBinaryType;
use dora_node_api::{DoraNode, Event};
use eyre::Result;
use robo_rover_lib::ArmStatus;
use std::collections::HashMap;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting enhanced monitor node with detailed debugging...");

    let (_node, mut events) = DoraNode::init_from_env()?;

    println!("Monitor node initialized successfully");
    println!("Monitoring all dataflow activity with enhanced logging...");

    let mut event_count = 0;
    let mut input_stats: HashMap<String, usize> = HashMap::new();

    while let Some(event) = events.recv() {
        event_count += 1;
        println!("\nEvent #{}: New event received", event_count);

        match event {
            Event::Input { id, metadata, data } => {
                let input_id = id.as_str();
                *input_stats.entry(input_id.to_string()).or_insert(0) += 1;

                let data_len = data.len();
                println!("INPUT EVENT:");
                println!("   ID: '{}'", input_id);
                println!("   Data length: {} bytes", data_len);
                println!("   Count for this input: {}", input_stats[input_id]);
                println!("   Metadata: {:?}", metadata);
                println!("   Data type: {:?}", data.data_type());

                // Process specific input types
                match input_id {
                    "keyboard" => {
                        println!("   KEYBOARD INPUT DETAILS:");

                        if let Some(string_array) = data.as_string_opt::<i32>() {
                            if string_array.len() > 0 {
                                let char_data = string_array.value(0);
                                let trimmed = char_data.trim();
                                println!("      Keyboard character: '{}'", trimmed);
                                println!("      Length: {} chars", trimmed.len());
                                println!("      Raw string: {:?}", char_data);
                                println!(
                                    "      ASCII values: {:?}",
                                    trimmed.chars().map(|c| c as u32).collect::<Vec<_>>()
                                );

                                // Check for special characters
                                match trimmed {
                                    " " => println!("      Detected: SPACE character"),
                                    "w" | "a" | "s" | "d" | "q" | "e" => {
                                        println!(
                                            "      Detected: Movement command '{}'",
                                            trimmed
                                        );
                                    }
                                    "home" => println!("      Detected: HOME command"),
                                    _ => println!("      Unknown/unmapped character"),
                                }
                            } else {
                                println!("      Empty string array for keyboard input");
                            }
                        } else {
                            println!("      Failed to parse keyboard data as string array");
                        }
                    }

                    "arm_command" => {
                        println!("   ARM COMMAND DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                // Try to parse as JSON to see the command structure
                                match serde_json::from_slice::<serde_json::Value>(bytes) {
                                    Ok(json_value) => {
                                        println!("       Successfully parsed JSON:");
                                        println!(
                                            "      Content: {}",
                                            serde_json::to_string_pretty(&json_value)
                                                .unwrap_or_else(
                                                    |_| "Failed to pretty print".to_string()
                                                )
                                        );

                                        if let Some(command) = json_value.get("command") {
                                            println!("      Command field found: {}", command);
                                        }
                                        if let Some(metadata) = json_value.get("metadata") {
                                            println!("      Metadata field found: {}", metadata);
                                        }
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse as JSON: {}", e);
                                        println!(
                                            "      Raw string attempt: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            }
                        } else {
                            println!("      Failed to parse arm_command as binary array");
                        }
                    }

                    "joint_feedback" => {
                        println!("   JOINT FEEDBACK DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<ArmStatus>(bytes) {
                                    Ok(status) => {
                                        println!("     Successfully parsed ArmStatus:");
                                        println!(
                                            "      Joint count: {}",
                                            status.joint_state.positions.len()
                                        );
                                        println!("      Is moving: {}", status.is_moving);
                                        println!("      Is homed: {}", status.is_homed);
                                        println!("      End effector pose: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                                 status.end_effector_pose[0], status.end_effector_pose[1], status.end_effector_pose[2],
                                                 status.end_effector_pose[3], status.end_effector_pose[4], status.end_effector_pose[5]);
                                        println!(
                                            "      Joint positions: {:?}",
                                            status.joint_state.positions
                                        );
                                        println!(
                                            "      Joint velocities: {:?}",
                                            status.joint_state.velocities
                                        );
                                        println!(
                                            "      Timestamp: {}",
                                            status.joint_state.timestamp
                                        );

                                        if let Some(ref error) = status.error_state {
                                            println!("      Error state: {}", error);
                                        }
                                        if let Some(ref cmd) = status.current_command {
                                            println!("      Current command: {}", cmd);
                                        }
                                        println!(
                                            "      Reachability: {:?}",
                                            status.reachability_status
                                        );
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse as ArmStatus: {}", e);
                                        println!(
                                            "      Raw string: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            }
                        } else {
                            println!("      Failed to parse joint_feedback as binary array");
                        }
                    }

                    "rover_command" => {
                        println!("   ROVER COMMAND DETAILS:");

                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<serde_json::Value>(bytes) {
                                    Ok(json_value) => {
                                        println!("       Successfully parsed JSON:");
                                        println!(
                                            "      Content: {}",
                                            serde_json::to_string_pretty(&json_value)
                                                .unwrap_or_else(
                                                    |_| "Failed to pretty print".to_string()
                                                )
                                        );
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse as JSON: {}", e);
                                        println!(
                                            "      Raw string attempt: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
                                    }
                                }
                            }
                        } else {
                            println!("      Failed to parse rover_command as binary array");
                        }
                    }

                    "rover_telemetry" => {
                        println!("   ROVER TELEMETRY DETAILS:");

                        // Still try to parse and show what we can
                        if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                            if bytes_array.len() > 0 {
                                let bytes = bytes_array.value(0);

                                match serde_json::from_slice::<serde_json::Value>(bytes) {
                                    Ok(json_value) => {
                                        println!("       Successfully parsed JSON:");
                                        println!(
                                            "      Content: {}",
                                            serde_json::to_string_pretty(&json_value)
                                                .unwrap_or_else(
                                                    |_| "Failed to pretty print".to_string()
                                                )
                                        );
                                    }
                                    Err(e) => {
                                        println!("      Failed to parse as JSON: {}", e);
                                        println!(
                                            "      Raw string attempt: {}",
                                            String::from_utf8_lossy(bytes)
                                        );
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
                    println!(
                        "      {}: {} events",
                        input_id,
                        count
                    );
                }
                println!("Monitor node stopping gracefully");
                break;
            }

            other_event => {
                println!("   OTHER EVENT TYPE: {:?}", other_event);
            }
        }
    }

    println!(
        "\nMonitor node finished after processing {} events",
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
        .finish();

    tracing::subscriber::set_default(subscriber)
}