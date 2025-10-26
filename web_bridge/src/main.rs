use dora_node_api::{
    arrow::array::{types::GenericBinaryType, Array, AsArray, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use robo_rover_lib::{
    ArmCommand, ArmCommandWithMetadata, CommandMetadata, CommandPriority, InputSource,
    ProcessedFrame, RoverCommand, RoverCommandWithMetadata, VideoControl,
};
use serde::{Deserialize, Serialize};
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

// Joint position structure for LeKiwi 6DOF arm
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JointPositions {
    pub shoulder_pan: f64,      // Joint 1: Base rotation
    pub shoulder_lift: f64,     // Joint 2: Shoulder pitch
    pub elbow_flex: f64,        // Joint 3: Elbow pitch
    pub wrist_flex: f64,        // Joint 4: Wrist pitch
    pub wrist_roll: f64,        // Joint 5: Wrist roll
    pub gripper: f64,           // Joint 6: Gripper
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebArmCommand {
    pub command_type: String,  // "joint_position", "cartesian", "home", "stop"
    pub joint_positions: Option<JointPositions>,
    pub max_velocity: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WebRoverCommand {
    command_type: String,  // "velocity", "joint_positions", "stop"
    // For velocity commands
    v_x: Option<f64>,      // Linear velocity in x (m/s)
    v_y: Option<f64>,      // Linear velocity in y (m/s)
    omega_z: Option<f64>,  // Angular velocity (rad/s)
    // For direct joint control
    wheel1: Option<f64>,
    wheel2: Option<f64>,
    wheel3: Option<f64>,
}


impl JointPositions {
    /// Validate joint limits for LeKiwi arm
    pub fn validate(&self) -> Result<(), String> {
        if self.shoulder_pan < -3.14 || self.shoulder_pan > 3.14 {
            return Err(format!("shoulder_pan out of range: {}", self.shoulder_pan));
        }
        if self.shoulder_lift < -1.57 || self.shoulder_lift > 1.57 {
            return Err(format!("shoulder_lift out of range: {}", self.shoulder_lift));
        }
        if self.elbow_flex < -2.09 || self.elbow_flex > 2.09 {
            return Err(format!("elbow_flex out of range: {}", self.elbow_flex));
        }
        if self.wrist_flex < -3.14 || self.wrist_flex > 3.14 {
            return Err(format!("wrist_flex out of range: {}", self.wrist_flex));
        }
        if self.wrist_roll < -1.57 || self.wrist_roll > 1.57 {
            return Err(format!("wrist_roll out of range: {}", self.wrist_roll));
        }
        if self.gripper < -3.14 || self.gripper > 3.14 {
            return Err(format!("gripper out of range: {}", self.gripper));
        }
        Ok(())
    }

    /// Convert to array [shoulder_pan, shoulder_lift, elbow_flex, wrist_flex, wrist_roll, gripper]
    pub fn to_array(&self) -> Vec<f64> {
        vec![
            self.shoulder_pan,
            self.shoulder_lift,
            self.elbow_flex,
            self.wrist_flex,
            self.wrist_roll,
            self.gripper,
        ]
    }

    pub fn home() -> Self {
        Self {
            shoulder_pan: 0.0,
            shoulder_lift: 0.0,
            elbow_flex: 0.0,
            wrist_flex: 0.0,
            wrist_roll: 0.0,
            gripper: 0.0,
        }
    }
}

// Client state for video streaming
#[derive(Clone)]
struct ClientVideoState {
    socket_id: String,
    streaming_enabled: bool,
    target_fps: u8,
    last_frame_sent: Arc<Mutex<SystemTime>>,
    frames_sent: Arc<Mutex<u64>>,
    frames_dropped: Arc<Mutex<u64>>,
}

impl ClientVideoState {
    fn new(socket_id: String) -> Self {
        Self {
            socket_id,
            streaming_enabled: false,
            target_fps: 30,
            last_frame_sent: Arc::new(Mutex::new(SystemTime::now())),
            frames_sent: Arc::new(Mutex::new(0)),
            frames_dropped: Arc::new(Mutex::new(0)),
        }
    }

    fn should_send_frame(&self) -> bool {
        if !self.streaming_enabled {
            return false;
        }

        let last_sent = self.last_frame_sent.lock().unwrap();
        let elapsed = last_sent.elapsed().unwrap_or(Duration::from_secs(1));
        let min_interval = Duration::from_millis((1000 / self.target_fps as u64).max(1));

        elapsed >= min_interval
    }

    fn mark_frame_sent(&self) {
        *self.last_frame_sent.lock().unwrap() = SystemTime::now();
        *self.frames_sent.lock().unwrap() += 1;
    }

    fn mark_frame_dropped(&self) {
        *self.frames_dropped.lock().unwrap() += 1;
    }
}

#[derive(Clone)]
struct SharedState {
    pub arm_command_queue: Arc<Mutex<Vec<WebArmCommand>>>,
    pub rover_command_queue: Arc<Mutex<Vec<WebRoverCommand>>>,
    pub video_clients: Arc<Mutex<Vec<ClientVideoState>>>,
    pub video_enabled: Arc<Mutex<bool>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            arm_command_queue: Arc::new(Mutex::new(Vec::new())),
            rover_command_queue: Arc::new(Mutex::new(Vec::new())),
            video_clients: Arc::new(Mutex::new(Vec::new())),
            video_enabled: Arc::new(Mutex::new(true)),
        }
    }
}

fn convert_web_command_to_arm_command(web_cmd: &WebArmCommand) -> Option<ArmCommand> {
    match web_cmd.command_type.as_str() {
        "joint_position" => {
            if let Some(ref positions) = web_cmd.joint_positions {
                // Validate positions
                if let Err(e) = positions.validate() {
                    eprintln!("Joint validation failed: {}", e);
                    return None;
                }

                Some(ArmCommand::JointPosition {
                    joint_angles: positions.to_array(),
                    max_velocity: web_cmd.max_velocity,
                })
            } else {
                None
            }
        }
        "home" => Some(ArmCommand::Home),
        "stop" => Some(ArmCommand::Stop),
        "emergency_stop" => Some(ArmCommand::EmergencyStop),
        _ => None,
    }
}

fn convert_web_command_to_rover_command(web_cmd: &WebRoverCommand) -> Option<RoverCommand> {
    match web_cmd.command_type.as_str() {
        "velocity" => {
            let v_x = web_cmd.v_x.unwrap_or(0.0);
            let v_y = web_cmd.v_y.unwrap_or(0.0);
            let omega_z = web_cmd.omega_z.unwrap_or(0.0);
            Some(RoverCommand::new_velocity(omega_z, v_x, v_y))
        }
        "joint_positions" => {
            let wheel1 = web_cmd.wheel1.unwrap_or(0.0);
            let wheel2 = web_cmd.wheel2.unwrap_or(0.0);
            let wheel3 = web_cmd.wheel3.unwrap_or(0.0);
            Some(RoverCommand::new_joint_positions(wheel1, wheel2, wheel3))
        }
        "stop" => Some(RoverCommand::new_stop()),
        _ => None,
    }
}

async fn start_socketio_server(shared_state: SharedState, io_handle: Arc<Mutex<Option<SocketIo>>>) -> Result<()> {
    println!("Starting SocketIO server on port 8080");

    let (layer, io) = SocketIo::new_layer();

    // Store io handle for broadcasting
    *io_handle.lock().unwrap() = Some(io.clone());

    io.ns("/", move |socket: SocketRef| {
        println!("Web client connected: {}", socket.id);

        let state = shared_state.clone();

        // Add client to connected list
        let client_state = ClientVideoState::new(socket.id.to_string());
        if let Ok(mut clients) = state.video_clients.lock() {
            clients.push(client_state.clone());
        }

        // Send welcome message
        let welcome_data = serde_json::json!({
            "type": "welcome",
            "message": "Connected to LeKiwi Controller",
            "client_id": socket.id.to_string(),
            "dof": 6,
            "supported_commands": ["joint_position", "home", "stop", "emergency_stop"],
            "joint_names": ["shoulder_pan", "shoulder_lift", "elbow_flex", "wrist_flex", "wrist_roll", "gripper"],
            "video_available": true
        });

        if let Err(e) = socket.emit("status", welcome_data) {
            println!("Failed to send welcome message: {}", e);
        }

        // Handle arm commands
        socket.on("arm_command", {
            let state = state.clone();
            move |socket: SocketRef, Data::<WebArmCommand>(cmd)| {
                println!("Received arm command: {:?}", cmd.command_type);

                // Validate and queue the command
                if cmd.command_type == "joint_position" {
                    if let Some(ref positions) = cmd.joint_positions {
                        match positions.validate() {
                            Ok(_) => {
                                if let Ok(mut queue) = state.arm_command_queue.lock() {
                                    queue.push(cmd.clone());
                                    let _ = socket.emit("command_ack", serde_json::json!({
                                        "status": "queued",
                                        "message": "Arm command queued"
                                    }));
                                }
                            }
                            Err(e) => {
                                let _ = socket.emit("error", serde_json::json!({
                                    "status": "error",
                                    "message": e
                                }));
                            }
                        }
                    }
                } else {
                    if let Ok(mut queue) = state.arm_command_queue.lock() {
                        queue.push(cmd.clone());
                        let _ = socket.emit("command_ack", serde_json::json!({
                            "status": "queued",
                            "message": format!("{} command queued", cmd.command_type)
                        }));
                    }
                }
            }
        });

        // Handle rover commands
        socket.on("rover_command", {
            let state = state.clone();
            move |socket: SocketRef, Data::<Value>(data)| {
                if let Ok(rover_cmd) = serde_json::from_value::<WebRoverCommand>(data) {
                    println!("Received rover command: {}", rover_cmd.command_type);
                    if let Ok(mut queue) = state.rover_command_queue.lock() {
                        queue.push(rover_cmd.clone());
                        let _ = socket.emit("rover_command_ack", serde_json::json!({
                            "status": "queued",
                            "message": format!("Rover {} command queued", rover_cmd.command_type)
                        }));
                    }
                }
            }
        });

        // Handle video control
        socket.on("video_control", {
            let state = state.clone();
            move |socket: SocketRef, Data::<Value>(data)| {
                println!("Video control received: {:?}", data);

                if let Ok(control) = serde_json::from_value::<VideoControl>(data) {
                    if let Ok(mut clients) = state.video_clients.lock() {
                        if let Some(client) = clients.iter_mut().find(|c| c.socket_id == socket.id.to_string()) {
                            match control.command {
                                robo_rover_lib::VideoCommand::Start => {
                                    client.streaming_enabled = true;
                                    if let Some(fps) = control.max_fps {
                                        client.target_fps = fps.min(60);
                                    }
                                    println!("Video streaming started for client {}", socket.id);
                                    let _ = socket.emit("video_status", serde_json::json!({
                                        "streaming": true,
                                        "fps": client.target_fps
                                    }));
                                }
                                robo_rover_lib::VideoCommand::Stop => {
                                    client.streaming_enabled = false;
                                    println!("Video streaming stopped for client {}", socket.id);
                                    let _ = socket.emit("video_status", serde_json::json!({
                                        "streaming": false
                                    }));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });

        // Handle disconnection
        socket.on_disconnect({
            let state = state.clone();
            move |socket: SocketRef| {
                println!("Client disconnected: {}", socket.id);
                if let Ok(mut clients) = state.video_clients.lock() {
                    clients.retain(|c| c.socket_id != socket.id.to_string());
                }
            }
        });
    });

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any);

    let app = axum::Router::new()
        .layer(ServiceBuilder::new().layer(cors).layer(layer));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("SocketIO server listening on :8080");

    axum::serve(listener, app).await?;
    Ok(())
}

fn create_metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: uuid::Uuid::new_v4().to_string(),
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        source: InputSource::WebBridge,
        priority: CommandPriority::Normal,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Web Bridge Node for LeKiwi Arm");

    let (node, mut events) = DoraNode::init_from_env()?;
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());

    let shared_state = SharedState::new();
    let io_handle: Arc<Mutex<Option<SocketIo>>> = Arc::new(Mutex::new(None));

    // Start SocketIO server
    let shared_state_clone = shared_state.clone();
    let io_handle_clone = io_handle.clone();
    let socketio_handle = tokio::spawn(async move {
        if let Err(e) = start_socketio_server(shared_state_clone, io_handle_clone).await {
            eprintln!("SocketIO server error: {}", e);
        }
    });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    let node_arc = Arc::new(Mutex::new(node));

    // Arm command processor
    let node_clone = node_arc.clone();
    let state_clone = shared_state.clone();
    let arm_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone.arm_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(arm_cmd) = convert_web_command_to_arm_command(&web_cmd) {
                        let cmd_with_metadata = ArmCommandWithMetadata {
                            command: Some(arm_cmd),
                            metadata: create_metadata(),
                        };

                        if let Ok(serialized) = serde_json::to_vec(&cmd_with_metadata) {
                            let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);
                            if let Ok(mut node_guard) = node_clone.lock() {
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

    // Rover command processor
    let node_clone_rover = node_arc.clone();
    let state_clone_rover = shared_state.clone();
    let rover_command_processor = tokio::spawn(async move {
        loop {
            if let Ok(mut queue) = state_clone_rover.rover_command_queue.lock() {
                if !queue.is_empty() {
                    let web_cmd = queue.remove(0);
                    if let Some(rover_cmd) = convert_web_command_to_rover_command(&web_cmd) {
                        let cmd_with_metadata = RoverCommandWithMetadata {
                            command: rover_cmd,
                            metadata: create_metadata(),
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

    println!("Web Bridge initialized - waiting for commands...");

    // Event loop - handle video frames
    let state_for_video = shared_state.clone();
    let io_for_video = io_handle.clone();

    loop {
        if let Some(event) = events.recv() {
            match event {
                Event::Input { id, data, .. } => {
                    match id.as_str() {
                        "video_frames" => {
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);

                                    if let Ok(frame) = serde_json::from_slice::<ProcessedFrame>(bytes) {
                                        // Encode frame data as base64
                                        let base64_data = base64_encode(&frame.data);

                                        // Broadcast to all streaming clients
                                        if let Ok(clients) = state_for_video.video_clients.lock() {
                                            for client in clients.iter() {
                                                if client.should_send_frame() {
                                                    if let Some(ref io) = *io_for_video.lock().unwrap() {
                                                        let frame_data = serde_json::json!({
                                                            "timestamp": frame.timestamp,
                                                            "frame_id": frame.frame_id,
                                                            "format": frame.format,
                                                            "width": frame.width,
                                                            "height": frame.height,
                                                            "data": base64_data,
                                                        });

                                                        if let Some(socket) = io.of("/").unwrap().get_socket((&client.socket_id).parse().unwrap()) {
                                                            let _ = socket.emit("video_frame", frame_data);
                                                            client.mark_frame_sent();
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
                        }
                        _ => {}
                    }
                }
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
    println!("Web Bridge shutdown complete");

    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(data)
}