use dora_node_api::{
    arrow::array::{types::GenericBinaryType, Array, AsArray, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event
};
use eyre::Result;
use robo_rover_lib::{ArmCommand, ArmCommandWithMetadata, ArmTelemetry, CommandMetadata, CommandPriority, InputSource, RoverCommand, RoverTelemetry};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid;

use axum::http::Method;
use socketioxide::{extract::{Data, SocketRef}, SocketIo};
use tokio;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct SharedState {
    latest_arm_telemetry: Arc<Mutex<Option<ArmTelemetry>>>,
    latest_rover_telemetry: Arc<Mutex<Option<RoverTelemetry>>>,
    connected_clients: Arc<Mutex<Vec<String>>>,
    stats: Arc<Mutex<WebBridgeStats>>,
    // Command queues
    arm_command_queue: Arc<Mutex<VecDeque<ArmCommandWithMetadata>>>,
    rover_command_queue: Arc<Mutex<VecDeque<RoverCommandWithMetadata>>>,
}

#[derive(Debug, Clone)]
struct WebBridgeStats {
    commands_received: u64,
    commands_sent: u64,
    clients_connected: u64,
    uptime_start: SystemTime,
}

impl SharedState {
    fn new() -> Self {
        Self {
            latest_arm_telemetry: Arc::new(Mutex::new(None)),
            latest_rover_telemetry: Arc::new(Mutex::new(None)),
            connected_clients: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(WebBridgeStats {
                commands_received: 0,
                commands_sent: 0,
                clients_connected: 0,
                uptime_start: SystemTime::now(),
            })),
            arm_command_queue: Arc::new(Mutex::new(VecDeque::new())),
            rover_command_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct WebArmCommand {
    #[serde(rename = "type")]
    command_type: String,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    roll: Option<f64>,
    pitch: Option<f64>,
    yaw: Option<f64>,
    joint_angles: Option<Vec<f64>>,
    delta_joints: Option<Vec<f64>>,
    max_velocity: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct WebRoverCommand {
    throttle: f64,
    brake: f64,
    steering_angle: f64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let _guard = init_tracing();

    println!("Starting Web Bridge Node with SocketIO Server on port 8080");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        if let Err(e) = web_bridge_async().await {
            eprintln!("Web bridge error: {}", e);
            std::process::exit(1);
        }
    });

    Ok(())
}

async fn web_bridge_async() -> Result<()> {
    // Check if port 8080 is available
    println!("Checking if port 8080 is available...");
    match tokio::net::TcpListener::bind("127.0.0.1:8080").await {
        Ok(test_listener) => {
            drop(test_listener);
            println!("Port 8080 is available");
        }
        Err(e) => {
            println!("Port 8080 is not available: {}", e);
            return Err(e.into());
        }
    }

    let (node, mut events) = DoraNode::init_from_env()?;
    let arm_command_output = DataId::from("arm_command".to_owned());
    let rover_command_output = DataId::from("rover_command".to_owned());

    let shared_state = SharedState::new();

    // Start SocketIO server for web clients
    let shared_state_clone = shared_state.clone();
    let socketio_handle = tokio::spawn(async move {
        start_web_socketio_server(shared_state_clone).await
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Wrap the node in Arc<Mutex<>> for sharing between tasks
    let node_arc = Arc::new(Mutex::new(node));
    let node_clone = node_arc.clone();
    let state_clone = shared_state.clone();
    let arm_output_clone = arm_command_output.clone();
    let rover_output_clone = rover_command_output.clone();

    // Start command processor task with actual node
    let command_processor_handle = tokio::spawn(async move {
        command_processor_loop(node_clone, state_clone, arm_output_clone, rover_output_clone).await;
    });

    println!("Web Bridge initialized");
    println!("SocketIO server running on http://127.0.0.1:8080");
    println!("Waiting for dora events and web client connections...");

    // Main event loop
    loop {
        let event_future = tokio::time::timeout(Duration::from_millis(50), async {
            events.recv()
        });

        if let Ok(Some(event)) = event_future.await {
            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();

                    match id_str {
                        "rover_telemetry" => {
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(telemetry) = serde_json::from_slice::<RoverTelemetry>(bytes) {
                                        if let Ok(mut rover_tel) = shared_state.latest_rover_telemetry.lock() {
                                            *rover_tel = Some(telemetry);
                                        }
                                    }
                                }
                            }
                        }

                        "arm_telemetry" => {
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>() {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    match serde_json::from_slice::<ArmTelemetry>(bytes) {
                                        Ok(telemetry) => {
                                            println!("Received arm telemetry: moving={}, pose=[{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                                     telemetry.is_moving,
                                                     telemetry.end_effector_pose[0], telemetry.end_effector_pose[1], telemetry.end_effector_pose[2],
                                                     telemetry.end_effector_pose[3], telemetry.end_effector_pose[4], telemetry.end_effector_pose[5]);

                                            // Store the telemetry for broadcasting to web clients
                                            if let Ok(mut arm_tel) = shared_state.latest_arm_telemetry.lock() {
                                                *arm_tel = Some(telemetry);
                                            }
                                        }
                                        Err(e) => {
                                            println!("Failed to parse arm telemetry: {}", e);
                                        }
                                    }
                                }
                            }
                        }

                        _ => {
                            println!("Unknown input: '{}'", id_str);
                        }
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
    command_processor_handle.abort();
    println!("Web Bridge shutdown complete");
    Ok(())
}

async fn start_web_socketio_server(shared_state: SharedState) -> Result<()> {
    println!("Starting Web SocketIO server on port 8080");

    let (layer, io) = SocketIo::new_layer();

    // Handle web client connections
    io.ns("/", move |socket: SocketRef| {
        println!("Web client connected: {}", socket.id);

        let state = shared_state.clone();

        // Add client to connected list
        if let Ok(mut clients) = state.connected_clients.lock() {
            clients.push(socket.id.to_string());
        }
        if let Ok(mut stats) = state.stats.lock() {
            stats.clients_connected += 1;
        }

        // Send welcome message
        let welcome_data = serde_json::json!({
            "type": "welcome",
            "message": "Connected to Robo Rover Web Bridge",
            "client_id": socket.id.to_string(),
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });

        if let Err(e) = socket.emit("status", welcome_data) {
            println!("Failed to send welcome message: {}", e);
        }

        // Handle arm commands from web clients
        socket.on("arm_command", {
            let state = state.clone();
            move |socket: SocketRef, Data::<WebArmCommand>(cmd)| {
                println!("Received arm command from web: {:?}", cmd);

                if let Ok(mut stats) = state.stats.lock() {
                    stats.commands_received += 1;
                }

                // Convert web command to ArmCommand
                match convert_web_arm_command(cmd) {
                    Ok(arm_command) => {
                        println!("Converted arm command: {:?}", arm_command);

                        // Create command with metadata
                        let cmd_with_metadata = ArmCommandWithMetadata {
                            command: Some(arm_command),
                            metadata: create_metadata(),
                        };

                        // Queue the command for processing
                        if let Ok(mut queue) = state.arm_command_queue.lock() {
                            queue.push_back(cmd_with_metadata);
                            println!("Queued arm command for processing (queue size: {})", queue.len());
                        }
                    }
                    Err(e) => {
                        println!("Failed to convert arm command: {}", e);
                        let error_response = serde_json::json!({
                            "type": "error",
                            "message": format!("Invalid arm command: {}", e)
                        });
                        let _ = socket.emit("error", error_response);
                    }
                }
            }
        });

        // Handle rover commands from web clients
        socket.on("rover_command", {
            let state = state.clone();
            move |_socket: SocketRef, Data::<WebRoverCommand>(cmd)| {
                println!("Received rover command from web: {:?}", cmd);

                if let Ok(mut stats) = state.stats.lock() {
                    stats.commands_received += 1;
                }

                let rover_command = RoverCommand {
                    throttle: cmd.throttle.clamp(-1.0, 1.0),
                    brake: cmd.brake.clamp(0.0, 1.0),
                    steering_angle: cmd.steering_angle.clamp(-15.0, 15.0),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                    command_id: uuid::Uuid::new_v4().to_string(),
                };

                println!("Converted rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                         rover_command.throttle, rover_command.brake, rover_command.steering_angle);

                // Create command with metadata
                let cmd_with_metadata = RoverCommandWithMetadata {
                    command: rover_command,
                    metadata: create_metadata(),
                };

                // Queue the command for processing
                if let Ok(mut queue) = state.rover_command_queue.lock() {
                    queue.push_back(cmd_with_metadata);
                    println!("Queued rover command for processing (queue size: {})", queue.len());
                }
            }
        });

        // Handle status requests
        socket.on("get_status", {
            let state = state.clone();
            move |socket: SocketRef| {
                let status_data = if let Ok(stats) = state.stats.lock() {
                    serde_json::json!({
                        "type": "system_status",
                        "commands_received": stats.commands_received,
                        "commands_sent": stats.commands_sent,
                        "clients_connected": stats.clients_connected,
                        "uptime_seconds": SystemTime::now()
                            .duration_since(stats.uptime_start)
                            .unwrap_or_default()
                            .as_secs()
                    })
                } else {
                    serde_json::json!({
                        "type": "error",
                        "message": "Failed to get system status"
                    })
                };

                let _ = socket.emit("status", status_data);
            }
        });

        // Handle disconnect
        socket.on_disconnect({
            let state = state.clone();
            move |socket: SocketRef| {
                println!("Web client disconnected: {}", socket.id);
                if let Ok(mut clients) = state.connected_clients.lock() {
                    clients.retain(|id| id != &socket.id.to_string());
                }
            }
        });

        // Start telemetry broadcaster for this client
        let socket_clone = socket.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            telemetry_broadcaster_loop(socket_clone, state_clone).await;
        });
    });

    // Create HTTP app with CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = axum::Router::new()
        .layer(ServiceBuilder::new().layer(cors).layer(layer));

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("Web SocketIO server listening on 127.0.0.1:8080");
    println!("Test with: curl -X GET http://127.0.0.1:8080/socket.io/");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn telemetry_broadcaster_loop(socket: SocketRef, state: SharedState) {
    let mut interval = tokio::time::interval(Duration::from_millis(200)); // 5 Hz

    loop {
        interval.tick().await;

        // Send arm telemetry if available
        if let Ok(arm_tel_opt) = state.latest_arm_telemetry.lock() {
            if let Some(ref telemetry) = *arm_tel_opt {
                let telemetry_data = serde_json::json!({
                    "type": "arm_telemetry",
                    "end_effector_pose": telemetry.end_effector_pose,
                    "is_moving": telemetry.is_moving,
                    "timestamp": telemetry.timestamp,
                    "joint_angles": telemetry.joint_angles,
                    "joint_velocities": telemetry.joint_velocities,
                    "source": telemetry.source
                });

                if socket.emit("telemetry", telemetry_data).is_err() {
                    break; // Client disconnected
                }
            }
        }

        // Send rover telemetry if available
        if let Ok(rover_tel_opt) = state.latest_rover_telemetry.lock() {
            if let Some(ref telemetry) = *rover_tel_opt {
                let telemetry_data = serde_json::json!({
                    "type": "rover_telemetry",
                    "position": telemetry.position,
                    "yaw": telemetry.yaw,
                    "velocity": telemetry.velocity,
                    "timestamp": telemetry.timestamp
                });

                if socket.emit("telemetry", telemetry_data).is_err() {
                    break; // Client disconnected
                }
            }
        }
    }
}

async fn command_processor_loop(
    node: Arc<Mutex<DoraNode>>,
    state: SharedState,
    arm_command_output: DataId,
    rover_command_output: DataId,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20 Hz processing

    println!("Starting command processor loop - REAL MODE (sending via dora)");

    loop {
        interval.tick().await;

        // Process arm command queue
        if let Ok(mut arm_queue) = state.arm_command_queue.lock() {
            while let Some(cmd_with_metadata) = arm_queue.pop_front() {
                println!("Processing queued arm command: {:?}", cmd_with_metadata.command);

                // Actually send via dora node
                match serde_json::to_vec(&cmd_with_metadata) {
                    Ok(serialized) => {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                        if let Ok(mut node_guard) = node.lock() {
                            match node_guard.send_output(arm_command_output.clone(), Default::default(), arrow_data) {
                                Ok(_) => {
                                    println!("ARM command sent to dora dataflow successfully");
                                    if let Ok(mut stats) = state.stats.lock() {
                                        stats.commands_sent += 1;
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to send arm command via dora: {}", e);
                                }
                            }
                        } else {
                            println!("Failed to acquire node lock for arm command");
                        }
                    }
                    Err(e) => {
                        println!("Failed to serialize arm command: {}", e);
                    }
                }
            }
        }

        // Process rover command queue
        if let Ok(mut rover_queue) = state.rover_command_queue.lock() {
            while let Some(cmd_with_metadata) = rover_queue.pop_front() {
                println!("Processing queued rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                         cmd_with_metadata.command.throttle,
                         cmd_with_metadata.command.brake,
                         cmd_with_metadata.command.steering_angle);

                // Actually send via dora node
                match serde_json::to_vec(&cmd_with_metadata) {
                    Ok(serialized) => {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                        if let Ok(mut node_guard) = node.lock() {
                            match node_guard.send_output(rover_command_output.clone(), Default::default(), arrow_data) {
                                Ok(_) => {
                                    println!("ROVER command sent to dora dataflow successfully");
                                    if let Ok(mut stats) = state.stats.lock() {
                                        stats.commands_sent += 1;
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to send rover command via dora: {}", e);
                                }
                            }
                        } else {
                            println!("Failed to acquire node lock for rover command");
                        }
                    }
                    Err(e) => {
                        println!("Failed to serialize rover command: {}", e);
                    }
                }
            }
        }
    }
}

fn convert_web_arm_command(web_cmd: WebArmCommand) -> Result<ArmCommand> {
    match web_cmd.command_type.as_str() {
        "cartesian_move" => Ok(ArmCommand::CartesianMove {
            x: web_cmd.x.unwrap_or(0.0),
            y: web_cmd.y.unwrap_or(0.0),
            z: web_cmd.z.unwrap_or(0.0),
            roll: web_cmd.roll.unwrap_or(0.0),
            pitch: web_cmd.pitch.unwrap_or(0.0),
            yaw: web_cmd.yaw.unwrap_or(0.0),
            max_velocity: web_cmd.max_velocity,
        }),
        "joint_position" => {
            if let Some(angles) = web_cmd.joint_angles {
                Ok(ArmCommand::JointPosition {
                    joint_angles: angles,
                    max_velocity: web_cmd.max_velocity,
                })
            } else {
                Err(eyre::eyre!("joint_angles required for joint_position command"))
            }
        },
        "relative_move" => {
            if let Some(deltas) = web_cmd.delta_joints {
                Ok(ArmCommand::RelativeMove {
                    delta_joints: deltas,
                })
            } else {
                Err(eyre::eyre!("delta_joints required for relative_move command"))
            }
        },
        "stop" => Ok(ArmCommand::Stop),
        "home" => Ok(ArmCommand::Home),
        "emergency_stop" => Ok(ArmCommand::EmergencyStop),
        _ => Err(eyre::eyre!("Unknown arm command type: {}", web_cmd.command_type))
    }
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