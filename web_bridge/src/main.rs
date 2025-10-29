use dora_node_api::{
    arrow::array::{Array, AsArray, BinaryArray, UInt8Array},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};
use robo_rover_lib::{
    ArmCommand, ArmCommandWithMetadata, AudioAction, AudioControl, CameraAction, CameraControl,
    CommandMetadata, CommandPriority, InputSource, RoverCommand, RoverCommandWithMetadata,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid;

use axum::http::Method;
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JointPositions {
    pub shoulder_pan: f64,
    pub shoulder_lift: f64,
    pub elbow_flex: f64,
    pub wrist_flex: f64,
    pub wrist_roll: f64,
    pub gripper: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebArmCommand {
    pub command_type: String,  // "joint_position", "cartesian", "home", "stop"
    pub joint_positions: Option<JointPositions>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebRoverCommand {
    pub command_type: String,
    pub wheel1: Option<f64>,
    pub wheel2: Option<f64>,
    pub wheel3: Option<f64>,
    pub wheel4: Option<f64>,
}

// Client state for video and audio streaming
#[derive(Clone)]
struct ClientState {
    socket_id: String,
    video_enabled: bool,
    audio_enabled: bool,
    target_fps: u8,
    jpeg_quality: u8,
    last_video_sent: Arc<Mutex<SystemTime>>,
    last_audio_sent: Arc<Mutex<SystemTime>>,
    video_frames_sent: Arc<Mutex<u64>>,
    audio_frames_sent: Arc<Mutex<u64>>,
    frames_dropped: Arc<Mutex<u64>>,
}

impl ClientState {
    fn new(socket_id: String) -> Self {
        Self {
            socket_id,
            video_enabled: true,
            audio_enabled: true,
            target_fps: 30,
            jpeg_quality: 80,
            last_video_sent: Arc::new(Mutex::new(SystemTime::now())),
            last_audio_sent: Arc::new(Mutex::new(SystemTime::now())),
            video_frames_sent: Arc::new(Mutex::new(0)),
            audio_frames_sent: Arc::new(Mutex::new(0)),
            frames_dropped: Arc::new(Mutex::new(0)),
        }
    }

    fn should_send_video(&self) -> bool {
        if !self.video_enabled {
            return false;
        }

        let last_sent = self.last_video_sent.lock().unwrap();
        let elapsed = last_sent.elapsed().unwrap_or(Duration::from_secs(1));
        let min_interval = Duration::from_millis((1000 / self.target_fps as u64).max(1));

        elapsed >= min_interval
    }

    fn mark_video_sent(&self) {
        *self.last_video_sent.lock().unwrap() = SystemTime::now();
        *self.video_frames_sent.lock().unwrap() += 1;
    }

    fn should_send_audio(&self) -> bool {
        if !self.audio_enabled {
            return false;
        }
        // Audio is less frequent, so we send every frame
        true
    }

    fn mark_audio_sent(&self) {
        *self.last_audio_sent.lock().unwrap() = SystemTime::now();
        *self.audio_frames_sent.lock().unwrap() += 1;
    }

    fn mark_frame_dropped(&self) {
        *self.frames_dropped.lock().unwrap() += 1;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebCameraCommand {
    pub command: String,  // "start" or "stop"
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebAudioCommand {
    pub command: String,  // "start" or "stop"
}

#[derive(Clone)]
struct SharedState {
    pub arm_command_queue: Arc<Mutex<Vec<WebArmCommand>>>,
    pub rover_command_queue: Arc<Mutex<Vec<WebRoverCommand>>>,
    pub camera_command_queue: Arc<Mutex<Vec<WebCameraCommand>>>,
    pub audio_command_queue: Arc<Mutex<Vec<WebAudioCommand>>>,
    pub video_clients: Arc<Mutex<Vec<ClientState>>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            arm_command_queue: Arc::new(Mutex::new(Vec::new())),
            rover_command_queue: Arc::new(Mutex::new(Vec::new())),
            camera_command_queue: Arc::new(Mutex::new(Vec::new())),
            audio_command_queue: Arc::new(Mutex::new(Vec::new())),
            video_clients: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

fn setup_socketio(shared_state: SharedState) -> (SocketIo, socketioxide::layer::SocketIoLayer) {
    let (layer, io) = SocketIo::new_layer();

    io.ns("/", move |socket: SocketRef| {
        let socket_id = socket.id.to_string();
        println!("Client connected: {}", socket_id);

        // Add client to video streaming list
        let client_state = ClientState::new(socket_id.clone());
        shared_state.video_clients.lock().unwrap().push(client_state);

        let shared_state_clone = shared_state.clone();
        socket.on("arm_command", move |_socket: SocketRef, Data::<Value>(data)| {
            if let Ok(web_cmd) = serde_json::from_value::<WebArmCommand>(data) {
                println!("Received arm command: {:?}", web_cmd.command_type);
                shared_state_clone
                    .arm_command_queue
                    .lock()
                    .unwrap()
                    .push(web_cmd);
            }
        });

        let shared_state_clone = shared_state.clone();
        socket.on(
            "rover_command",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Ok(web_cmd) = serde_json::from_value::<WebRoverCommand>(data) {
                    println!("Received rover command: {:?}", web_cmd.command_type);
                    shared_state_clone
                        .rover_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on(
            "camera_control",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Ok(web_cmd) = serde_json::from_value::<WebCameraCommand>(data) {
                    println!("Received camera control: {:?}", web_cmd.command);
                    shared_state_clone
                        .camera_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on(
            "audio_control",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Ok(web_cmd) = serde_json::from_value::<WebAudioCommand>(data) {
                    println!("Received audio control: {:?}", web_cmd.command);
                    shared_state_clone
                        .audio_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on_disconnect(move |socket: SocketRef| {
            let socket_id = socket.id.to_string();
            println!("Client disconnected: {}", socket_id);

            // Remove client from video list
            if let Ok(mut clients) = shared_state_clone.video_clients.lock() {
                clients.retain(|c| c.socket_id != socket_id);
            }
        });
    });

    (io, layer)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Web Bridge...");

    let (node, mut events) = DoraNode::init_from_env()?;
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());
    let camera_command_output = DataId::from("camera_command".to_owned());
    let audio_command_output = DataId::from("audio_command".to_owned());

    let shared_state = SharedState::new();
    let (io, layer) = setup_socketio(shared_state.clone());
    let io_handle = Arc::new(Mutex::new(Some(io.clone())));

    // Start Socket.IO server
    let socketio_handle = tokio::spawn(async move {
        let app = axum::Router::new()
            .layer(
                ServiceBuilder::new()
                    .layer(
                        CorsLayer::new()
                            .allow_origin(Any)
                            .allow_methods([Method::GET, Method::POST])
                            .allow_headers(Any),
                    )
                    .layer(layer),
            );

        let listener = tokio::net::TcpListener::bind("0.0.0.0:3030")
            .await
            .unwrap();

        println!("Socket.IO server listening on http://0.0.0.0:3030");
        axum::serve(listener, app).await.unwrap();
    });

    // Process commands
    let node_clone_arm = Arc::new(Mutex::new(node));
    let node_clone_rover = node_clone_arm.clone();
    let node_clone_camera = node_clone_arm.clone();
    let node_clone_audio = node_clone_arm.clone();
    let state_clone_arm = shared_state.clone();

    let arm_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_arm.arm_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(arm_cmd) = convert_web_command_to_arm_command(&web_cmd) {
                        let metadata = create_metadata();
                        let cmd_with_metadata = ArmCommandWithMetadata {
                            command: Some(arm_cmd),
                            metadata,
                        };

                        if let Ok(serialized) = serde_json::to_vec(&cmd_with_metadata) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone_arm.lock() {
                                let _ = node_guard.send_output(
                                    arm_command_output.clone(),
                                    Default::default(),
                                    arrow_data,
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Process rover commands
    let state_clone_rover = shared_state.clone();
    let rover_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_rover.rover_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(rover_cmd) = convert_web_command_to_rover_command(&web_cmd) {
                        let metadata = create_metadata();
                        let cmd_with_metadata = RoverCommandWithMetadata {
                            command: rover_cmd,
                            metadata,
                        };

                        if let Ok(serialized) = serde_json::to_vec(&cmd_with_metadata) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone_rover.lock() {
                                let _ = node_guard.send_output(
                                    rover_command_output.clone(),
                                    Default::default(),
                                    arrow_data,
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Process camera control commands
    let state_clone_camera = shared_state.clone();
    let camera_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_camera.camera_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(camera_cmd) = convert_web_command_to_camera_command(&web_cmd) {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        let camera_control = CameraControl {
                            command: camera_cmd,
                            timestamp,
                        };

                        if let Ok(serialized) = serde_json::to_vec(&camera_control) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone_camera.lock() {
                                let _ = node_guard.send_output(
                                    camera_command_output.clone(),
                                    Default::default(),
                                    arrow_data,
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Process audio control commands
    let state_clone_audio = shared_state.clone();
    let audio_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_audio.audio_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(audio_cmd) = convert_web_command_to_audio_command(&web_cmd) {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        let audio_control = AudioControl {
                            command: audio_cmd,
                            timestamp,
                        };

                        if let Ok(serialized) = serde_json::to_vec(&audio_control) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone_audio.lock() {
                                let _ = node_guard.send_output(
                                    audio_command_output.clone(),
                                    Default::default(),
                                    arrow_data,
                                );
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    println!(" Web Bridge initialized - waiting for...");

    // Event loop - handle video frames
    let state_for_video = shared_state.clone();
    let io_for_video = io_handle.clone();
    let mut frame_counter = 0u64;

    loop {
        if let Some(event) = events.recv() {
            match event {
                Event::Input { id, data, metadata, .. } => match id.as_str() {
                    "audio_frame" => {
                        // Handle audio
                        // Try multiple array types since dora-microphone format may vary
                        let audio_bytes_opt: Option<Vec<u8>> = if let Some(float32_array) = data.as_any().downcast_ref::<dora_node_api::arrow::array::Float32Array>() {
                            // Float32Array - normalized audio [-1.0, 1.0]
                            // Convert to Int16 (S16LE) for transmission
                            let mut bytes = Vec::with_capacity(float32_array.len() * 2);

                            // Debug: Log first conversion for quality check
                            // static mut CONVERSION_COUNT: u32 = 0;
                            // unsafe {
                            //     if CONVERSION_COUNT < 3 {
                            //         println!("Converting Float32 -> S16LE: {} samples", float32_array.len());
                            //         if float32_array.len() > 0 {
                            //             let first_samples: Vec<f32> = float32_array.values()[..10.min(float32_array.len())].to_vec();
                            //             println!("   First 10 float samples: {:?}", first_samples);
                            //         }
                            //         CONVERSION_COUNT += 1;
                            //     }
                            // }

                            for &sample in float32_array.values() {
                                // Convert float32 [-1.0, 1.0] to int16 [-32768, 32767]
                                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                bytes.extend_from_slice(&sample_i16.to_le_bytes());
                            }

                            // Debug: Log first conversion result
                            // unsafe {
                            //     if CONVERSION_COUNT <= 3 && bytes.len() >= 20 {
                            //         let first_bytes: Vec<u8> = bytes[..20].to_vec();
                            //         println!("   First 20 S16LE bytes: {:?}", first_bytes);
                            //     }
                            // }

                            Some(bytes)
                        } else if let Some(list_array) = data.as_list_opt::<i32>() {
                            // ListArray containing audio data
                            if list_array.len() > 0 {
                                let values = list_array.value(0);
                                if let Some(uint8_array) = values.as_any().downcast_ref::<UInt8Array>() {
                                    Some(uint8_array.values().to_vec())
                                } else if let Some(int16_array) = values.as_any().downcast_ref::<dora_node_api::arrow::array::Int16Array>() {
                                    // Convert i16 to bytes
                                    let mut bytes = Vec::with_capacity(int16_array.len() * 2);
                                    for sample in int16_array.values() {
                                        bytes.extend_from_slice(&sample.to_le_bytes());
                                    }
                                    Some(bytes)
                                } else if let Some(float32_array) = values.as_any().downcast_ref::<dora_node_api::arrow::array::Float32Array>() {
                                    // Float32 in list
                                    let mut bytes = Vec::with_capacity(float32_array.len() * 2);
                                    for &sample in float32_array.values() {
                                        let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                        bytes.extend_from_slice(&sample_i16.to_le_bytes());
                                    }
                                    Some(bytes)
                                } else {
                                    eprintln!("Audio list values type: {:?}", values.data_type());
                                    None
                                }
                            } else {
                                None
                            }
                        } else if let Some(int16_array) = data.as_any().downcast_ref::<dora_node_api::arrow::array::Int16Array>() {
                            // Direct Int16Array
                            let mut bytes = Vec::with_capacity(int16_array.len() * 2);
                            for sample in int16_array.values() {
                                bytes.extend_from_slice(&sample.to_le_bytes());
                            }
                            Some(bytes)
                        } else if let Some(uint8_array) = data.as_any().downcast_ref::<UInt8Array>() {
                            // Direct UInt8Array
                            Some(uint8_array.values().to_vec())
                        } else if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            // BinaryArray
                            if binary_array.len() > 0 {
                                Some(binary_array.value(0).to_vec())
                            } else {
                                None
                            }
                        } else if let Some(fixed_binary) = data.as_any().downcast_ref::<dora_node_api::arrow::array::FixedSizeBinaryArray>() {
                            // FixedSizeBinaryArray
                            if fixed_binary.len() > 0 {
                                Some(fixed_binary.value(0).to_vec())
                            } else {
                                None
                            }
                        } else {
                            eprintln!("Unknown audio array type: {:?}", data.data_type());
                            None
                        };

                        if let Some(audio_bytes) = audio_bytes_opt {
                            // Extract audio metadata
                            let sample_rate = metadata.parameters.get("sample_rate")
                                .and_then(|v| match v {
                                    dora_node_api::Parameter::Integer(i) => Some(*i as u32),
                                    _ => None,
                                })
                                .unwrap_or(16000);

                            let channels = metadata.parameters.get("channels")
                                .and_then(|v| match v {
                                    dora_node_api::Parameter::Integer(i) => Some(*i as u16),
                                    _ => None,
                                })
                                .unwrap_or(1);

                            frame_counter += 1;

                            // Create audio frame for JSON transport
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;

                            let audio_frame_data = serde_json::json!({
                                "timestamp": timestamp,
                                "frame_id": frame_counter,
                                "sample_rate": sample_rate,
                                "channels": channels,
                                "format": "s16le",
                                "data": audio_bytes,
                            });

                            // Send audio to all connected clients
                            if let Ok(clients) = state_for_video.video_clients.lock() {
                                for client in clients.iter() {
                                    if client.should_send_audio() {
                                        if let Some(ref io) = *io_for_video.lock().unwrap() {
                                            if let Some(socket) = io
                                                .of("/")
                                                .unwrap()
                                                .get_socket((&client.socket_id).parse().unwrap())
                                            {
                                                let _ = socket.emit("audio_frame", audio_frame_data.clone());
                                                client.mark_audio_sent();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "video_frame" => {
                        frame_counter += 1;

                        // Extract metadata (width, height, encoding)
                        let width = metadata.parameters.get("width")
                            .and_then(|v| match v {
                                dora_node_api::Parameter::Integer(i) => Some(*i as u32),
                                _ => None,
                            })
                            .unwrap_or(640);
                        let height = metadata.parameters.get("height")
                            .and_then(|v| match v {
                                dora_node_api::Parameter::Integer(i) => Some(*i as u32),
                                _ => None,
                            })
                            .unwrap_or(480);

                        // Get RGB8 data from gst-camera (sent as raw bytes)
                        if let Some(rgb_data) = data.as_any().downcast_ref::<UInt8Array>() {
                            let rgb_bytes = rgb_data.values().as_ref();

                            // Verify expected size
                            let expected_size = (width * height * 3) as usize; // RGB8 = 3 bytes per pixel
                            if rgb_bytes.len() != expected_size {
                                eprintln!("Frame size mismatch: got {} bytes, expected {}",
                                          rgb_bytes.len(), expected_size);
                                continue;
                            }

                            // Create image buffer from RGB data
                            if let Some(img_buf) = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, rgb_bytes) {
                                // Encode to JPEG
                                let mut jpeg_data = Vec::new();
                                {
                                    let mut cursor = Cursor::new(&mut jpeg_data);
                                    let mut encoder = JpegEncoder::new_with_quality(&mut cursor, 80);
                                    if let Err(e) = encoder.encode(
                                        &img_buf,
                                        width,
                                        height,
                                        image::ExtendedColorType::Rgb8
                                    ) {
                                        eprintln!("JPEG encoding error: {}", e);
                                        continue;
                                    }
                                }

                                // Send JPEG to all connected clients
                                if let Ok(clients) = state_for_video.video_clients.lock() {
                                    for client in clients.iter() {
                                        if client.should_send_video() {
                                            if let Some(ref io) = *io_for_video.lock().unwrap() {
                                                let timestamp = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_millis() as u64;

                                                let frame_data = serde_json::json!({
                                                    "timestamp": timestamp,
                                                    "frame_id": frame_counter,
                                                    "width": width,
                                                    "height": height,
                                                    "codec": "jpeg",
                                                    "data": jpeg_data, // JPEG binary data
                                                });

                                                if let Some(socket) = io
                                                    .of("/")
                                                    .unwrap()
                                                    .get_socket((&client.socket_id).parse().unwrap())
                                                {
                                                    let _ = socket.emit("video_frame", frame_data);
                                                    client.mark_video_sent();
                                                }
                                            }
                                        } else {
                                            client.mark_frame_dropped();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Event::Stop(_) => {
                    println!("Stop event received");
                    break;
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Cleanup
    socketio_handle.abort();
    arm_command_processor.abort();
    rover_command_processor.abort();
    camera_command_processor.abort();
    println!("Web Bridge shutdown complete");

    Ok(())
}

fn convert_web_command_to_arm_command(web_cmd: &WebArmCommand) -> Option<ArmCommand> {
    match web_cmd.command_type.as_str() {
        "joint_position" => {
            if let Some(ref positions) = web_cmd.joint_positions {
                Some(ArmCommand::JointPosition {
                    joint_angles: vec![
                        positions.shoulder_pan,
                        positions.shoulder_lift,
                        positions.elbow_flex,
                        positions.wrist_flex,
                        positions.wrist_roll,
                        positions.gripper,
                    ],
                    max_velocity: None,
                })
            } else {
                None
            }
        }
        "home" => Some(ArmCommand::Home),
        "stop" => Some(ArmCommand::Stop),
        _ => None,
    }
}

fn convert_web_command_to_rover_command(web_cmd: &WebRoverCommand) -> Option<RoverCommand> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid;

    match web_cmd.command_type.as_str() {
        "wheel_positions" => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let command_id = uuid::Uuid::new_v4().to_string();

            Some(RoverCommand::JointPositions {
                wheel1: web_cmd.wheel1.unwrap_or(0.0),
                wheel2: web_cmd.wheel2.unwrap_or(0.0),
                wheel3: web_cmd.wheel3.unwrap_or(0.0),
                timestamp,
                command_id,
            })
        }
        "stop" => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let command_id = uuid::Uuid::new_v4().to_string();

            Some(RoverCommand::Stop {
                timestamp,
                command_id,
            })
        }
        _ => None,
    }
}

fn convert_web_command_to_camera_command(web_cmd: &WebCameraCommand) -> Option<CameraAction> {
    match web_cmd.command.as_str() {
        "start" => Some(CameraAction::Start),
        "stop" => Some(CameraAction::Stop),
        _ => None,
    }
}

fn convert_web_command_to_audio_command(web_cmd: &WebAudioCommand) -> Option<AudioAction> {
    match web_cmd.command.as_str() {
        "start" => Some(AudioAction::Start),
        "stop" => Some(AudioAction::Stop),
        _ => None,
    }
}

fn create_metadata() -> CommandMetadata {
    CommandMetadata {
        source: InputSource::WebBridge,
        priority: CommandPriority::Normal,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        command_id: uuid::Uuid::new_v4().to_string(),
    }
}