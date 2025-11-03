use dora_node_api::{
    arrow::array::{Array, AsArray, BinaryArray, Float32Array, UInt8Array},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};
use robo_rover_lib::{
    ArmCommand, ArmCommandWithMetadata, AudioAction, AudioControl, CameraAction, CameraControl,
    CommandMetadata, CommandPriority, InputSource, RoverCommand, RoverCommandWithMetadata,
    init_tracing,
};
use robo_rover_lib::types::{DetectionFrame, TrackingCommand, TrackingTelemetry, SpeechTranscription, SystemMetrics};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid;

use axum::http::{Method, HeaderValue};
use serde_json::Value;
use socketioxide::{
    extract::{Data, SocketRef, TryData},
    SocketIo,
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use std::env;
use log::info;

mod security;
use security::{AuthRateLimiter, CommandRateLimiter, parse_allowed_origins, log_auth_attempt, log_rate_limit_exceeded, log_validation_error};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebTrackingCommand {
    pub command_type: String,  // "enable", "disable", "select_target", "clear_target"
    pub tracking_id: Option<u32>,  // For "select_target"
    pub detection_index: Option<usize>,  // For "select_target" by index
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebTtsCommand {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebAudioStream {
    pub audio_data: Vec<f32>,  // Float32 audio samples from Web UI microphone
}

#[derive(Clone)]
struct SharedState {
    pub arm_command_queue: Arc<Mutex<Vec<WebArmCommand>>>,
    pub rover_command_queue: Arc<Mutex<Vec<WebRoverCommand>>>,
    pub camera_command_queue: Arc<Mutex<Vec<WebCameraCommand>>>,
    pub audio_command_queue: Arc<Mutex<Vec<WebAudioCommand>>>,
    pub tracking_command_queue: Arc<Mutex<Vec<WebTrackingCommand>>>,
    pub tts_command_queue: Arc<Mutex<Vec<WebTtsCommand>>>,
    pub audio_stream_queue: Arc<Mutex<Vec<WebAudioStream>>>,
    pub voice_command_audio_queue: Arc<Mutex<Vec<WebAudioStream>>>,
    pub video_clients: Arc<Mutex<Vec<ClientState>>>,
    pub performance_monitoring_enabled: Arc<Mutex<bool>>,
    pub auth_rate_limiter: Arc<AuthRateLimiter>,
    pub command_rate_limiter: Arc<CommandRateLimiter>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            arm_command_queue: Arc::new(Mutex::new(Vec::new())),
            rover_command_queue: Arc::new(Mutex::new(Vec::new())),
            camera_command_queue: Arc::new(Mutex::new(Vec::new())),
            audio_command_queue: Arc::new(Mutex::new(Vec::new())),
            tracking_command_queue: Arc::new(Mutex::new(Vec::new())),
            tts_command_queue: Arc::new(Mutex::new(Vec::new())),
            audio_stream_queue: Arc::new(Mutex::new(Vec::new())),
            voice_command_audio_queue: Arc::new(Mutex::new(Vec::new())),
            video_clients: Arc::new(Mutex::new(Vec::new())),
            performance_monitoring_enabled: Arc::new(Mutex::new(true)),
            auth_rate_limiter: Arc::new(AuthRateLimiter::new()),
            command_rate_limiter: Arc::new(CommandRateLimiter::new()),
        }
    }
}


fn setup_socketio(shared_state: SharedState) -> (SocketIo, socketioxide::layer::SocketIoLayer) {
    let (layer, io) = SocketIo::new_layer();

    // Get authentication credentials from environment variables
    let auth_username = env::var("AUTH_USERNAME").unwrap_or_else(|_| {
        tracing::warn!("AUTH_USERNAME not set, using default 'admin' - CHANGE THIS IN PRODUCTION!");
        "admin".to_string()
    });
    let auth_password = env::var("AUTH_PASSWORD").unwrap_or_else(|_| {
        tracing::warn!("AUTH_PASSWORD not set, using default 'password' - CHANGE THIS IN PRODUCTION!");
        "password".to_string()
    });

    tracing::info!("Authentication enabled - Username: {}", auth_username);
    tracing::info!("Security features: Rate limiting enabled, Input validation enabled");

    io.ns("/", move |socket: SocketRef, TryData::<AuthCredentials>(auth)| {
        let socket_id = socket.id.to_string();

        // Check rate limit for authentication attempts
        if !shared_state.auth_rate_limiter.check_auth_attempt(&socket_id) {
            log_rate_limit_exceeded(&socket_id, "auth");
            tracing::warn!("Rate limit exceeded for auth attempt from: {}", socket_id);
            socket.disconnect().ok();
            return;
        }

        // Validate authentication
        let (is_authenticated, username) = match auth {
            Ok(credentials) => {
                let auth_ok = credentials.username == auth_username && credentials.password == auth_password;
                (auth_ok, credentials.username.clone())
            }
            Err(_) => (false, "unknown".to_string()),
        };

        // Log authentication attempt
        log_auth_attempt(&socket_id, &username, is_authenticated);

        if !is_authenticated {
            tracing::warn!("Authentication failed for connection attempt from: {}", socket_id);
            socket.disconnect().ok();
            return;
        }

        tracing::info!("Client authenticated and connected: {}", socket_id);

        // Reset rate limiter on successful auth
        shared_state.auth_rate_limiter.reset(&socket_id);

        // Add client to video streaming list
        let client_state = ClientState::new(socket_id.clone());
        shared_state.video_clients.lock().unwrap().push(client_state);

        let shared_state_clone = shared_state.clone();
        let socket_id_clone = socket_id.clone();
        socket.on("arm_command", move |_socket: SocketRef, Data::<Value>(data)| {
            // Check rate limit
            if !shared_state_clone.command_rate_limiter.check_command(&socket_id_clone) {
                log_rate_limit_exceeded(&socket_id_clone, "arm_command");
                return;
            }

            if let Ok(web_cmd) = serde_json::from_value::<WebArmCommand>(data) {
                // Validate joint positions if present
                if let Some(ref positions) = web_cmd.joint_positions {
                    let joint_values = vec![
                        positions.shoulder_pan, positions.shoulder_lift, positions.elbow_flex,
                        positions.wrist_flex, positions.wrist_roll, positions.gripper
                    ];
                    for (i, &angle) in joint_values.iter().enumerate() {
                        if let Err(e) = security::validation::validate_joint_position(angle) {
                            log_validation_error(&socket_id_clone, &format!("Arm joint {}: {}", i, e));
                            tracing::warn!("Arm command validation failed: {}", e);
                            return;
                        }
                    }
                }

                tracing::debug!("Received arm command: {:?}", web_cmd.command_type);
                shared_state_clone
                    .arm_command_queue
                    .lock()
                    .unwrap()
                    .push(web_cmd);
            }
        });

        let shared_state_clone = shared_state.clone();
        let socket_id_clone = socket_id.clone();
        socket.on(
            "rover_command",
            move |_socket: SocketRef, Data::<Value>(data)| {
                // Check rate limit
                if !shared_state_clone.command_rate_limiter.check_command(&socket_id_clone) {
                    log_rate_limit_exceeded(&socket_id_clone, "rover_command");
                    return;
                }

                if let Ok(web_cmd) = serde_json::from_value::<WebRoverCommand>(data) {
                    // Validate wheel velocities if present
                    let wheels = [web_cmd.wheel1, web_cmd.wheel2, web_cmd.wheel3, web_cmd.wheel4];
                    for (i, wheel_opt) in wheels.iter().enumerate() {
                        if let Some(velocity) = wheel_opt {
                            if let Err(e) = security::validation::validate_wheel_velocity(*velocity) {
                                log_validation_error(&socket_id_clone, &format!("Wheel {}: {}", i+1, e));
                                tracing::warn!("Rover command validation failed: {}", e);
                                return;
                            }
                        }
                    }

                    tracing::debug!("Received rover command: {:?}", web_cmd.command_type);
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
                    tracing::debug!("Received camera control: {:?}", web_cmd.command);
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
                    tracing::debug!("Received audio control: {:?}", web_cmd.command);
                    shared_state_clone
                        .audio_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on(
            "tracking_command",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Ok(web_cmd) = serde_json::from_value::<WebTrackingCommand>(data) {
                    tracing::debug!("Received tracking command: {:?}", web_cmd.command_type);
                    shared_state_clone
                        .tracking_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        let socket_id_clone = socket_id.clone();
        socket.on(
            "tts_command",
            move |_socket: SocketRef, Data::<Value>(data)| {
                // Check rate limit
                if !shared_state_clone.command_rate_limiter.check_command(&socket_id_clone) {
                    log_rate_limit_exceeded(&socket_id_clone, "tts_command");
                    return;
                }

                if let Ok(web_cmd) = serde_json::from_value::<WebTtsCommand>(data) {
                    // Validate TTS text
                    if let Err(e) = security::validation::validate_tts_text(&web_cmd.text) {
                        log_validation_error(&socket_id_clone, &format!("TTS text: {}", e));
                        tracing::warn!("TTS command validation failed: {}", e);
                        return;
                    }

                    tracing::debug!("Received TTS command: {}", web_cmd.text);
                    shared_state_clone
                        .tts_command_queue
                        .lock()
                        .unwrap()
                        .push(web_cmd);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        let socket_id_clone = socket_id.clone();
        socket.on(
            "audio_stream",
            move |_socket: SocketRef, Data::<Value>(data)| {
                // Check rate limit (audio streams are less restricted)
                if !shared_state_clone.command_rate_limiter.check_command(&socket_id_clone) {
                    log_rate_limit_exceeded(&socket_id_clone, "audio_stream");
                    return;
                }

                if let Ok(web_audio) = serde_json::from_value::<WebAudioStream>(data) {
                    // Validate audio data
                    if let Err(e) = security::validation::validate_audio_data(&web_audio.audio_data) {
                        log_validation_error(&socket_id_clone, &format!("Audio stream: {}", e));
                        tracing::warn!("Audio stream validation failed: {}", e);
                        return;
                    }

                    tracing::debug!("Received audio stream: {} samples", web_audio.audio_data.len());
                    shared_state_clone
                        .audio_stream_queue
                        .lock()
                        .unwrap()
                        .push(web_audio);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on(
            "voice_command_audio",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Ok(web_audio) = serde_json::from_value::<WebAudioStream>(data) {
                    tracing::debug!("Received voice command audio: {} samples", web_audio.audio_data.len());
                    shared_state_clone
                        .voice_command_audio_queue
                        .lock()
                        .unwrap()
                        .push(web_audio);
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on(
            "performance_control",
            move |_socket: SocketRef, Data::<Value>(data)| {
                if let Some(enabled) = data.get("enabled").and_then(|v| v.as_bool()) {
                    tracing::info!("Performance monitoring {}", if enabled { "enabled" } else { "disabled" });
                    *shared_state_clone.performance_monitoring_enabled.lock().unwrap() = enabled;
                }
            },
        );

        let shared_state_clone = shared_state.clone();
        socket.on_disconnect(move |socket: SocketRef| {
            let socket_id = socket.id.to_string();
            tracing::info!("Client disconnected: {}", socket_id);

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
    let _guard = init_tracing();

    tracing::info!("Starting Web Bridge...");

    let (node, mut events) = DoraNode::init_from_env()?;
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());
    let camera_command_output = DataId::from("camera_command".to_owned());
    let audio_command_output = DataId::from("audio_command".to_owned());
    let tracking_command_output = DataId::from("tracking_command".to_owned());
    let tts_command_output = DataId::from("tts_command".to_owned());
    let audio_stream_output = DataId::from("audio_stream".to_owned());
    let voice_command_audio_output = DataId::from("voice_command_audio".to_owned());

    let shared_state = SharedState::new();
    let (io, layer) = setup_socketio(shared_state.clone());
    let io_handle = Arc::new(Mutex::new(Some(io.clone())));

    // Start Socket.IO server
    let socketio_handle = tokio::spawn(async move {
        // Get allowed origins from environment
        let allowed_origins = parse_allowed_origins();
        tracing::info!("CORS allowed origins: {:?}", allowed_origins);

        // Convert to HeaderValue for CORS layer
        let origins: Vec<HeaderValue> = allowed_origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect();

        // Define allowed headers explicitly (required when using credentials)
        let allowed_headers = [
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ];

        let cors_layer = if origins.is_empty() {
            tracing::warn!("No valid CORS origins configured, defaulting to localhost");
            CorsLayer::new()
                .allow_origin([
                    "http://localhost:3000".parse::<HeaderValue>().unwrap(),
                    "http://localhost:5173".parse::<HeaderValue>().unwrap(),
                ])
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(allowed_headers)
                .allow_credentials(true)
        } else {
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(allowed_headers)
                .allow_credentials(true)
        };

        let app = axum::Router::new()
            .layer(
                ServiceBuilder::new()
                    .layer(cors_layer)
                    .layer(layer),
            );

        let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("SOCKET_IO_PORT").unwrap_or_else(|_| "3030".to_string());
        let addr = format!("{}:{}", bind_address, port);

        tracing::info!("Binding Socket.IO server to: {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .unwrap();

        info!("Socket.IO server listening on http://{}", addr);
        axum::serve(listener, app).await.unwrap();
    });

    // Process commands
    let node_clone_arm = Arc::new(Mutex::new(node));
    let node_clone_rover = node_clone_arm.clone();
    let node_clone_camera = node_clone_arm.clone();
    let node_clone_audio = node_clone_arm.clone();
    let node_clone_tracking = node_clone_arm.clone();
    let node_clone_tts = node_clone_arm.clone();
    let node_clone_audio_stream = node_clone_arm.clone();
    let node_clone_voice_command = node_clone_arm.clone();
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
    let _audio_command_processor = tokio::spawn(async move {
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

    // Process tracking commands
    let state_clone_tracking = shared_state.clone();
    let _tracking_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_tracking.tracking_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(tracking_cmd) = convert_web_command_to_tracking_command(&web_cmd) {
                        if let Ok(serialized) = serde_json::to_vec(&tracking_cmd) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone_tracking.lock() {
                                let _ = node_guard.send_output(
                                    tracking_command_output.clone(),
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

    // Process TTS commands
    let state_clone_tts = shared_state.clone();
    let _tts_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_tts.tts_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    let tts_cmd = convert_web_command_to_tts_command(&web_cmd);
                    if let Ok(serialized) = serde_json::to_vec(&tts_cmd) {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                        if let Ok(mut node_guard) = node_clone_tts.lock() {
                            let _ = node_guard.send_output(
                                tts_command_output.clone(),
                                Default::default(),
                                arrow_data,
                            );
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Process audio stream from Web UI microphone (walkie-talkie mode)
    let state_clone_audio_stream = shared_state.clone();
    let _audio_stream_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_audio_stream.audio_stream_queue.lock() {
                if !queue.is_empty() {
                    let web_audio = queue.remove(0);
                    tracing::debug!("Processing audio stream: {} samples", web_audio.audio_data.len());

                    // Send audio data directly as Float32Array to audio_playback node
                    let arrow_data = Float32Array::from(web_audio.audio_data);
                    if let Ok(mut node_guard) = node_clone_audio_stream.lock() {
                        let _ = node_guard.send_output(
                            audio_stream_output.clone(),
                            Default::default(),
                            arrow_data,
                        );
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Process voice command audio from Web UI microphone (voice command mode)
    let state_clone_voice_command = shared_state.clone();
    let _voice_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_voice_command.voice_command_audio_queue.lock() {
                if !queue.is_empty() {
                    let web_audio = queue.remove(0);
                    tracing::debug!("Processing voice command audio: {} samples", web_audio.audio_data.len());

                    // Send audio data as Float32Array to speech_recognizer node
                    let arrow_data = Float32Array::from(web_audio.audio_data);
                    if let Ok(mut node_guard) = node_clone_voice_command.lock() {
                        let _ = node_guard.send_output(
                            voice_command_audio_output.clone(),
                            Default::default(),
                            arrow_data,
                        );
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    tracing::info!("Web Bridge initialized - waiting for events...");

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
                                    tracing::warn!("Audio list values type: {:?}", values.data_type());
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
                            tracing::warn!("Unknown audio array type: {:?}", data.data_type());
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
                                tracing::error!("Frame size mismatch: got {} bytes, expected {}",
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
                                        tracing::error!("JPEG encoding error: {}", e);
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
                    "detections" => {
                        // Handle detection frames from object_detector
                        if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let detection_data = binary_array.value(0);

                                // Deserialize DetectionFrame
                                match serde_json::from_slice::<DetectionFrame>(detection_data) {
                                    Ok(detection_frame) => {
                                        // Forward detections to all connected clients
                                        if let Ok(clients) = state_for_video.video_clients.lock() {
                                            if let Some(ref io) = *io_for_video.lock().unwrap() {
                                                // Emit to all clients via Socket.IO
                                                let _ = io.of("/").unwrap().emit("detections", serde_json::to_value(&detection_frame).unwrap());
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to deserialize detections: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    "tracked_detections" => {
                        // Handle tracked detection frames from object_tracker
                        if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let detection_data = binary_array.value(0);

                                // Deserialize DetectionFrame with tracking IDs
                                match serde_json::from_slice::<DetectionFrame>(detection_data) {
                                    Ok(detection_frame) => {
                                        // Forward tracked detections to all connected clients
                                        if let Some(ref io) = *io_for_video.lock().unwrap() {
                                            let _ = io.of("/").unwrap().emit("tracked_detections", serde_json::to_value(&detection_frame).unwrap());
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to deserialize tracked detections: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    "tracking_telemetry" => {
                        // Handle tracking telemetry from object_tracker
                        if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let telemetry_data = binary_array.value(0);

                                // Deserialize TrackingTelemetry
                                match serde_json::from_slice::<TrackingTelemetry>(telemetry_data) {
                                    Ok(telemetry) => {
                                        // Forward telemetry to all connected clients
                                        if let Some(ref io) = *io_for_video.lock().unwrap() {
                                            let _ = io.of("/").unwrap().emit("tracking_telemetry", serde_json::to_value(&telemetry).unwrap());
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to deserialize tracking telemetry: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    "servo_telemetry" => {
                        // Handle servo telemetry from visual-servo-controller
                        // This includes distance estimation and control mode (auto/manual)
                        if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let telemetry_data = binary_array.value(0);

                                // Deserialize TrackingTelemetry (with enhanced distance and mode)
                                match serde_json::from_slice::<TrackingTelemetry>(telemetry_data) {
                                    Ok(telemetry) => {
                                        // Forward enhanced telemetry to all connected clients
                                        if let Some(ref io) = *io_for_video.lock().unwrap() {
                                            let _ = io.of("/").unwrap().emit("servo_telemetry", serde_json::to_value(&telemetry).unwrap());
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to deserialize servo telemetry: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    "transcription" => {
                        // Handle speech transcription from speech_recognizer
                        if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                            if binary_array.len() > 0 {
                                let transcription_data = binary_array.value(0);

                                // Deserialize SpeechTranscription
                                match serde_json::from_slice::<SpeechTranscription>(transcription_data) {
                                    Ok(transcription) => {
                                        tracing::info!("Transcription received: \"{}\" (confidence: {:.2})",
                                            transcription.text, transcription.confidence);

                                        // Forward transcription to all connected clients
                                        if let Some(ref io) = *io_for_video.lock().unwrap() {
                                            let _ = io.of("/").unwrap().emit("transcription", serde_json::to_value(&transcription).unwrap());
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to deserialize transcription: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    "performance_metrics" => {
                        // Handle performance metrics from performance_monitor
                        // Only forward if monitoring is enabled
                        let monitoring_enabled = *state_for_video.performance_monitoring_enabled.lock().unwrap();

                        if monitoring_enabled {
                            if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                                if binary_array.len() > 0 {
                                    let metrics_data = binary_array.value(0);

                                    // Deserialize SystemMetrics
                                    match serde_json::from_slice::<SystemMetrics>(metrics_data) {
                                        Ok(metrics) => {
                                            tracing::trace!(
                                                "Performance metrics - CPU: {:.1}%, Memory: {:.0}MB, FPS: {:.1}, Latency: {:.1}ms",
                                                metrics.total_cpu_percent,
                                                metrics.total_memory_mb,
                                                metrics.dataflow_fps,
                                                metrics.end_to_end_latency_ms
                                            );

                                            // Forward metrics to all connected clients
                                            if let Some(ref io) = *io_for_video.lock().unwrap() {
                                                let _ = io.of("/").unwrap().emit("performance_metrics", serde_json::to_value(&metrics).unwrap());
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to deserialize performance metrics: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Event::Stop(_) => {
                    tracing::info!("Stop event received");
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
    tracing::info!("Web Bridge shutdown complete");

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

fn convert_web_command_to_tracking_command(web_cmd: &WebTrackingCommand) -> Option<TrackingCommand> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    match web_cmd.command_type.as_str() {
        "enable" => Some(TrackingCommand::Enable { timestamp }),
        "disable" => Some(TrackingCommand::Disable { timestamp }),
        "select_target" => {
            if let Some(tracking_id) = web_cmd.tracking_id {
                Some(TrackingCommand::SelectTargetById { tracking_id, timestamp })
            } else if let Some(detection_index) = web_cmd.detection_index {
                Some(TrackingCommand::SelectTarget { detection_index, timestamp })
            } else {
                None
            }
        }
        "clear_target" => Some(TrackingCommand::ClearTarget { timestamp }),
        _ => None,
    }
}

fn convert_web_command_to_tts_command(web_cmd: &WebTtsCommand) -> robo_rover_lib::TtsCommand {
    use robo_rover_lib::TtsPriority;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    robo_rover_lib::TtsCommand {
        text: web_cmd.text.clone(),
        timestamp,
        priority: TtsPriority::Normal,
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