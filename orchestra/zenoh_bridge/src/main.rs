/// Orchestra Zenoh Bridge
/// Runs on workstation - subscribes FROM rover, publishes TO rover

use dora_node_api::{
    arrow::array::{Array, BinaryArray, Float32Array},
    dora_core::config::DataId,
    DoraNode, Event, Parameter,
};
use eyre::Result;
use robo_rover_lib::init_tracing;
use zenoh::Config;
use std::collections::BTreeMap;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = init_tracing();

    tracing::info!("Starting Orchestra Zenoh Bridge");

    // Get entity IDs from environment
    let entity_id = std::env::var("ENTITY_ID").unwrap_or_else(|_| "orchestra".to_string());
    let selected_entity = std::env::var("SELECTED_ENTITY")
        .unwrap_or_else(|_| "rover-kiwi".to_string());

    tracing::info!("Orchestra ID: {}", entity_id);
    tracing::info!("Selected rover: {}", selected_entity);

    // Video frame configuration (should match rover's camera settings)
    let frame_width = std::env::var("VIDEO_WIDTH")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(640);
    let frame_height = std::env::var("VIDEO_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(480);

    tracing::info!("Expected video frame dimensions: {}x{}", frame_width, frame_height);

    // Audio configuration (should match rover's audio_capture settings)
    let audio_sample_rate = std::env::var("AUDIO_SAMPLE_RATE")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(16000);
    let audio_channels = std::env::var("AUDIO_CHANNELS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    tracing::info!("Expected audio format: {}Hz, {} channels", audio_sample_rate, audio_channels);

    // Initialize Dora node
    let (mut node, mut events) = DoraNode::init_from_env()?;

    // Initialize Zenoh session
    let mut config = Config::default();
    config.insert_json5("mode", "\"peer\"")
        .map_err(|e| eyre::eyre!("Failed to set Zenoh mode: {}", e))?;
    let session = zenoh::open(config).await
        .map_err(|e| eyre::eyre!("Failed to open Zenoh session: {}", e))?;

    tracing::info!("Zenoh session ID: {}", session.zid());

    // =========================================================================
    // SUBSCRIBERS: Receive data FROM selected rover via Zenoh
    // =========================================================================

    let video_topic = format!("rover/{}/video/raw", selected_entity);
    let video_sub = session
        .declare_subscriber(&video_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", video_topic, e))?;
    tracing::info!("Subscriber: {}", video_topic);

    let audio_topic = format!("rover/{}/audio/raw", selected_entity);
    let audio_sub = session
        .declare_subscriber(&audio_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", audio_topic, e))?;
    tracing::info!("Subscriber: {}", audio_topic);

    let rover_telemetry_topic = format!("rover/{}/telemetry/rover", selected_entity);
    let rover_telemetry_sub = session
        .declare_subscriber(&rover_telemetry_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", rover_telemetry_topic, e))?;
    tracing::info!("Subscriber: {}", rover_telemetry_topic);

    let arm_telemetry_topic = format!("rover/{}/telemetry/arm", selected_entity);
    let arm_telemetry_sub = session
        .declare_subscriber(&arm_telemetry_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", arm_telemetry_topic, e))?;
    tracing::info!("Subscriber: {}", arm_telemetry_topic);

    let servo_telemetry_topic = format!("rover/{}/telemetry/servo", selected_entity);
    let servo_telemetry_sub = session
        .declare_subscriber(&servo_telemetry_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", servo_telemetry_topic, e))?;
    tracing::info!("Subscriber: {}", servo_telemetry_topic);

    let metrics_topic = format!("rover/{}/metrics", selected_entity);
    let metrics_sub = session
        .declare_subscriber(&metrics_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare subscriber {}: {}", metrics_topic, e))?;
    tracing::info!("Subscriber: {}", metrics_topic);

    // =========================================================================
    // PUBLISHERS: Send commands TO selected rover via Zenoh
    // =========================================================================

    let rover_cmd_topic = format!("rover/{}/cmd/movement", selected_entity);
    let rover_cmd_pub = session
        .declare_publisher(&rover_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", rover_cmd_topic, e))?;
    tracing::info!("Publisher: {}", rover_cmd_topic);

    let arm_cmd_topic = format!("rover/{}/cmd/arm", selected_entity);
    let arm_cmd_pub = session
        .declare_publisher(&arm_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", arm_cmd_topic, e))?;
    tracing::info!("Publisher: {}", arm_cmd_topic);

    let camera_cmd_topic = format!("rover/{}/cmd/camera", selected_entity);
    let camera_cmd_pub = session
        .declare_publisher(&camera_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", camera_cmd_topic, e))?;
    tracing::info!("Publisher: {}", camera_cmd_topic);

    let audio_cmd_topic = format!("rover/{}/cmd/audio", selected_entity);
    let audio_cmd_pub = session
        .declare_publisher(&audio_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", audio_cmd_topic, e))?;
    tracing::info!("Publisher: {}", audio_cmd_topic);

    let tracking_cmd_topic = format!("rover/{}/cmd/tracking", selected_entity);
    let tracking_cmd_pub = session
        .declare_publisher(&tracking_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", tracking_cmd_topic, e))?;
    tracing::info!("Publisher: {}", tracking_cmd_topic);

    let tracking_telemetry_topic = format!("rover/{}/cmd/tracking_telemetry", selected_entity);
    let _tracking_telemetry_pub = session
        .declare_publisher(&tracking_telemetry_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", tracking_telemetry_topic, e))?;
    tracing::info!("Publisher: {}", tracking_telemetry_topic);

    let tts_cmd_topic = format!("rover/{}/cmd/tts", selected_entity);
    let tts_cmd_pub = session
        .declare_publisher(&tts_cmd_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", tts_cmd_topic, e))?;
    tracing::info!("Publisher: {}", tts_cmd_topic);

    let audio_stream_topic = format!("rover/{}/cmd/audio_stream", selected_entity);
    let audio_stream_pub = session
        .declare_publisher(&audio_stream_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", audio_stream_topic, e))?;
    tracing::info!("Publisher: {}", audio_stream_topic);

    let detections_topic = format!("rover/{}/video/detections", selected_entity);
    let detections_pub = session
        .declare_publisher(&detections_topic)
        .await
        .map_err(|e| eyre::eyre!("Failed to declare publisher {}: {}", detections_topic, e))?;
    tracing::info!("Publisher: {}", detections_topic);

    // =========================================================================
    // Dora output DataIds
    // =========================================================================

    let video_frame_output = DataId::from("video_frame".to_owned());
    let audio_frame_output = DataId::from("audio_frame".to_owned());
    let rover_telemetry_output = DataId::from("rover_telemetry".to_owned());
    let arm_telemetry_output = DataId::from("arm_telemetry".to_owned());
    let servo_telemetry_output = DataId::from("servo_telemetry".to_owned());
    let performance_metrics_output = DataId::from("performance_metrics".to_owned());

    // Statistics
    let mut video_count: u64 = 0;
    let mut audio_count: u64 = 0;
    let mut cmd_count: u64 = 0;

    // Create channel to bridge Dora's sync events to async
    let (dora_tx, dora_rx) = flume::unbounded();

    // Spawn task to read Dora events
    std::thread::spawn(move || {
        while let Some(event) = events.recv() {
            if dora_tx.send(event).is_err() {
                break;
            }
        }
    });

    tracing::info!("Entering main event loop...");

    // =========================================================================
    // Main event loop
    // =========================================================================

    loop {
        tokio::select! {
            // Handle Dora events (commands FROM local dataflow TO publish to Zenoh)
            Ok(event) = dora_rx.recv_async() => {
                match event {
                    Event::Input { id, data, .. } => {
                        // Handle audio_stream (Float32Array from web_bridge)
                        if id.as_str() == "audio_stream_web" {
                            if let Some(float32_array) = data.as_any().downcast_ref::<Float32Array>() {
                                if float32_array.len() > 0 {
                                    // Convert Float32Array to bytes for Zenoh transport
                                    let float_slice = float32_array.values().as_ref();
                                    let bytes: &[u8] = unsafe {
                                        std::slice::from_raw_parts(
                                            float_slice.as_ptr() as *const u8,
                                            float_slice.len() * std::mem::size_of::<f32>()
                                        )
                                    };
                                    let _ = audio_stream_pub.put(bytes).await;
                                }
                            }
                        }
                        // Handle other commands (BinaryArray - JSON serialized)
                        else if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let bytes = binary_array.value(0);

                                match id.as_str() {
                                    "rover_command_web" | "rover_command_parser" => {
                                        let _ = rover_cmd_pub.put(bytes).await;
                                        cmd_count += 1;
                                    }
                                    "arm_command_web" | "arm_command_parser" => {
                                        let _ = arm_cmd_pub.put(bytes).await;
                                        cmd_count += 1;
                                    }
                                    "camera_command_web" | "camera_control_parser" => {
                                        let _ = camera_cmd_pub.put(bytes).await;
                                    }
                                    "audio_command_web" => {
                                        let _ = audio_cmd_pub.put(bytes).await;
                                    }
                                    "tracking_command_web" | "tracking_command_parser" => {
                                        let _ = tracking_cmd_pub.put(bytes).await;
                                    }
                                    "tts_command_web" | "tts_command_parser" => {
                                        let _ = tts_cmd_pub.put(bytes).await;
                                    }
                                    "detections" => {
                                        let _ = detections_pub.put(bytes).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Event::Stop(_) => {
                        tracing::info!("Stop signal received");
                        tracing::info!("Stats: video={}, audio={}, commands={}", video_count, audio_count, cmd_count);
                        break;
                    }
                    _ => {}
                }
            }

            // Handle Zenoh subscriptions
            Ok(sample) = video_sub.recv_async() => {
                let payload = sample.payload().to_bytes();

                // Create metadata for RGB8 frame
                let mut params = BTreeMap::new();
                params.insert("encoding".to_owned(), Parameter::String("RGB8".to_string()));
                params.insert("width".to_owned(), Parameter::Integer(frame_width));
                params.insert("height".to_owned(), Parameter::Integer(frame_height));

                // Send as raw bytes (UInt8Array) with metadata, just like kornia_capture does
                let _ = node.send_output_bytes(
                    video_frame_output.clone(),
                    params,
                    payload.len(),
                    payload.as_ref()
                );

                video_count += 1;
                if video_count % 30 == 0 {
                    tracing::info!("Received {} video frames", video_count);
                }
            }

            Ok(sample) = audio_sub.recv_async() => {
                let payload = sample.payload().to_bytes();

                // Convert raw bytes back to Float32Array
                // The rover sent Float32 samples as raw bytes, so we need to reconstruct them
                let float_slice: &[f32] = unsafe {
                    std::slice::from_raw_parts(
                        payload.as_ref().as_ptr() as *const f32,
                        payload.len() / std::mem::size_of::<f32>()
                    )
                };
                let audio_array = Float32Array::from(float_slice.to_vec());

                // Create metadata matching audio_capture format
                let mut params = BTreeMap::new();
                params.insert("sample_rate".to_owned(), Parameter::Integer(audio_sample_rate));
                params.insert("channels".to_owned(), Parameter::Integer(audio_channels));
                params.insert("format".to_owned(), Parameter::String("f32le".to_string()));

                // Send as Float32Array with metadata
                let _ = node.send_output(audio_frame_output.clone(), params, audio_array);
                audio_count += 1;
            }

            Ok(sample) = rover_telemetry_sub.recv_async() => {
                let payload = sample.payload().to_bytes();
                let arrow_data = BinaryArray::from_vec(vec![payload.as_ref()]);
                let _ = node.send_output(rover_telemetry_output.clone(), Default::default(), arrow_data);
            }

            Ok(sample) = arm_telemetry_sub.recv_async() => {
                let payload = sample.payload().to_bytes();
                let arrow_data = BinaryArray::from_vec(vec![payload.as_ref()]);
                let _ = node.send_output(arm_telemetry_output.clone(), Default::default(), arrow_data);
            }

            Ok(sample) = servo_telemetry_sub.recv_async() => {
                let payload = sample.payload().to_bytes();
                let arrow_data = BinaryArray::from_vec(vec![payload.as_ref()]);
                let _ = node.send_output(servo_telemetry_output.clone(), Default::default(), arrow_data);
            }

            Ok(sample) = metrics_sub.recv_async() => {
                let payload = sample.payload().to_bytes();
                let arrow_data = BinaryArray::from_vec(vec![payload.as_ref()]);
                let _ = node.send_output(performance_metrics_output.clone(), Default::default(), arrow_data);
            }
        }
    }

    Ok(())
}
