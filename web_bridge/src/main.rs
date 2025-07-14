use dora_node_api::{
    arrow,
    arrow::{
        array::{
            types::GenericBinaryType, Array, AsArray, BinaryArray, BooleanArray, Float64Array,
            StringArray, UInt64Array,
        },
        datatypes::{DataType, Field, Schema},
        ipc::reader::StreamReader,
        ipc::writer::StreamWriter,
        record_batch::RecordBatch,
    },
    dora_core::config::DataId,
    DoraNode, Event,
};
use eyre::Result;
use robo_rover_lib::{
    ArmCommand, ArmCommandWithMetadata, ArmTelemetry, CommandMetadata, CommandPriority,
    InputSource, RoverCommand, RoverTelemetry,
};
use std::collections::VecDeque;
use std::error::Error;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid;

use axum::http::Method;
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
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
    // Arrow schemas
    arm_telemetry_schema: Arc<Schema>,
    rover_telemetry_schema: Arc<Schema>,
    arm_command_schema: Arc<Schema>,
    rover_command_schema: Arc<Schema>,
}

#[derive(Debug, Clone)]
struct WebBridgeStats {
    commands_received: u64,
    commands_sent: u64,
    clients_connected: u64,
    uptime_start: SystemTime,
    arrow_messages_sent: u64,
    arrow_messages_received: u64,
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
                arrow_messages_sent: 0,
                arrow_messages_received: 0,
            })),
            arm_command_queue: Arc::new(Mutex::new(VecDeque::new())),
            rover_command_queue: Arc::new(Mutex::new(VecDeque::new())),
            arm_telemetry_schema: Arc::new(create_arm_telemetry_schema()),
            rover_telemetry_schema: Arc::new(create_rover_telemetry_schema()),
            arm_command_schema: Arc::new(create_arm_command_schema()),
            rover_command_schema: Arc::new(create_rover_command_schema()),
        }
    }
}

// Arrow Schema Definitions
fn create_arm_telemetry_schema() -> Schema {
    Schema::new(vec![
        Field::new("end_effector_x", DataType::Float64, false),
        Field::new("end_effector_y", DataType::Float64, false),
        Field::new("end_effector_z", DataType::Float64, false),
        Field::new("end_effector_roll", DataType::Float64, false),
        Field::new("end_effector_pitch", DataType::Float64, false),
        Field::new("end_effector_yaw", DataType::Float64, false),
        Field::new("is_moving", DataType::Boolean, false),
        Field::new("timestamp", DataType::UInt64, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("joint_angles", DataType::Utf8, true), // JSON string for optional array
        Field::new("joint_velocities", DataType::Utf8, true), // JSON string for optional array
    ])
}

fn create_rover_telemetry_schema() -> Schema {
    Schema::new(vec![
        Field::new("position_x", DataType::Float64, false),
        Field::new("position_y", DataType::Float64, false),
        Field::new("yaw", DataType::Float64, false),
        Field::new("pitch", DataType::Float64, false),
        Field::new("roll", DataType::Float64, false),
        Field::new("velocity", DataType::Float64, false),
        Field::new("timestamp", DataType::UInt64, false),
        Field::new("near_sample", DataType::Boolean, false),
        Field::new("picking_up", DataType::Boolean, false),
        Field::new("nav_angles", DataType::Utf8, true), // JSON string for optional array
        Field::new("nav_dists", DataType::Utf8, true),  // JSON string for optional array
    ])
}

fn create_arm_command_schema() -> Schema {
    Schema::new(vec![
        Field::new("command_type", DataType::Utf8, false),
        Field::new("x", DataType::Float64, true),
        Field::new("y", DataType::Float64, true),
        Field::new("z", DataType::Float64, true),
        Field::new("roll", DataType::Float64, true),
        Field::new("pitch", DataType::Float64, true),
        Field::new("yaw", DataType::Float64, true),
        Field::new("max_velocity", DataType::Float64, true),
        Field::new("joint_angles", DataType::Utf8, true), // JSON string for optional array
        Field::new("delta_joints", DataType::Utf8, true), // JSON string for optional array
        Field::new("command_id", DataType::Utf8, false),
        Field::new("timestamp", DataType::UInt64, false),
    ])
}

fn create_rover_command_schema() -> Schema {
    Schema::new(vec![
        Field::new("throttle", DataType::Float64, false),
        Field::new("brake", DataType::Float64, false),
        Field::new("steering_angle", DataType::Float64, false),
        Field::new("timestamp", DataType::UInt64, false),
        Field::new("command_id", DataType::Utf8, false),
    ])
}

// Arrow conversion functions
fn arm_telemetry_to_arrow(telemetry: &ArmTelemetry, schema: &Schema) -> Result<String> {
    let end_effector_x = Float64Array::from(vec![telemetry.end_effector_pose[0]]);
    let end_effector_y = Float64Array::from(vec![telemetry.end_effector_pose[1]]);
    let end_effector_z = Float64Array::from(vec![telemetry.end_effector_pose[2]]);
    let end_effector_roll = Float64Array::from(vec![telemetry.end_effector_pose[3]]);
    let end_effector_pitch = Float64Array::from(vec![telemetry.end_effector_pose[4]]);
    let end_effector_yaw = Float64Array::from(vec![telemetry.end_effector_pose[5]]);
    let is_moving = BooleanArray::from(vec![telemetry.is_moving]);
    let timestamp = UInt64Array::from(vec![telemetry.timestamp]);
    let source = StringArray::from(vec![telemetry.source.as_str()]);

    let joint_angles_json = if let Some(ref angles) = telemetry.joint_angles {
        serde_json::to_string(angles)?
    } else {
        "null".to_string()
    };

    let joint_velocities_json = if let Some(ref velocities) = telemetry.joint_velocities {
        serde_json::to_string(velocities)?
    } else {
        "null".to_string()
    };

    let joint_angles = StringArray::from(vec![Some(joint_angles_json.as_str())]);
    let joint_velocities = StringArray::from(vec![Some(joint_velocities_json.as_str())]);

    let record_batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(end_effector_x),
            Arc::new(end_effector_y),
            Arc::new(end_effector_z),
            Arc::new(end_effector_roll),
            Arc::new(end_effector_pitch),
            Arc::new(end_effector_yaw),
            Arc::new(is_moving),
            Arc::new(timestamp),
            Arc::new(source),
            Arc::new(joint_angles),
            Arc::new(joint_velocities),
        ],
    )?;

    arrow_batch_to_base64(&record_batch)
}

fn rover_telemetry_to_arrow(telemetry: &RoverTelemetry, schema: &Schema) -> Result<String> {
    let position_x = Float64Array::from(vec![telemetry.position.0]);
    let position_y = Float64Array::from(vec![telemetry.position.1]);
    let yaw = Float64Array::from(vec![telemetry.yaw]);
    let pitch = Float64Array::from(vec![telemetry.pitch]);
    let roll = Float64Array::from(vec![telemetry.roll]);
    let velocity = Float64Array::from(vec![telemetry.velocity]);
    let timestamp = UInt64Array::from(vec![telemetry.timestamp]);
    let near_sample = BooleanArray::from(vec![telemetry.near_sample]);
    let picking_up = BooleanArray::from(vec![telemetry.picking_up]);

    let nav_angles_json = if let Some(ref angles) = telemetry.nav_angles {
        serde_json::to_string(angles)?
    } else {
        "null".to_string()
    };

    let nav_dists_json = if let Some(ref dists) = telemetry.nav_dists {
        serde_json::to_string(dists)?
    } else {
        "null".to_string()
    };

    let nav_angles = StringArray::from(vec![Some(nav_angles_json.as_str())]);
    let nav_dists = StringArray::from(vec![Some(nav_dists_json.as_str())]);

    let record_batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(position_x),
            Arc::new(position_y),
            Arc::new(yaw),
            Arc::new(pitch),
            Arc::new(roll),
            Arc::new(velocity),
            Arc::new(timestamp),
            Arc::new(near_sample),
            Arc::new(picking_up),
            Arc::new(nav_angles),
            Arc::new(nav_dists),
        ],
    )?;

    arrow_batch_to_base64(&record_batch)
}

fn arrow_batch_to_base64(batch: &RecordBatch) -> Result<String> {
    let mut buffer = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut buffer, &batch.schema())?;
        writer.write(batch)?;
        writer.finish()?;
    }
    Ok(base64::encode(&buffer))
}

fn base64_to_arrow_batch(base64_data: &str) -> Result<RecordBatch> {
    let buffer = base64::decode(base64_data)?;
    let cursor = Cursor::new(buffer);
    let mut reader = StreamReader::try_new(cursor, None)?;

    if let Some(batch) = reader.next() {
        Ok(batch?)
    } else {
        Err(eyre::eyre!("No record batch found"))
    }
}

fn arrow_to_arm_command(base64_data: &str) -> Result<ArmCommand> {
    let batch = base64_to_arrow_batch(base64_data)?;

    if batch.num_rows() != 1 {
        return Err(eyre::eyre!("Expected exactly one row in arm command batch"));
    }

    let schema = batch.schema();
    let mut command_type = String::new();
    let mut x = None;
    let mut y = None;
    let mut z = None;
    let mut roll = None;
    let mut pitch = None;
    let mut yaw = None;
    let mut max_velocity = None;
    let mut joint_angles = None;
    let mut delta_joints = None;

    for (i, field) in schema.fields().iter().enumerate() {
        let column = batch.column(i);

        match field.name().as_str() {
            "command_type" => {
                // Handle string or dictionary-encoded string
                if let Some(string_array) = column.as_any().downcast_ref::<StringArray>() {
                    command_type = string_array.value(0).to_string();
                } else if let Some(dict_array) = column
                    .as_any()
                    .downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>()
                {
                    if let Some(values) = dict_array.values().as_any().downcast_ref::<StringArray>()
                    {
                        let key = dict_array.keys().value(0) as usize;
                        command_type = values.value(key).to_string();
                    }
                }
            }
            "x" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        x = Some(array.value(0));
                    }
                }
            }
            "y" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        y = Some(array.value(0));
                    }
                }
            }
            "z" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        z = Some(array.value(0));
                    }
                }
            }
            "roll" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        roll = Some(array.value(0));
                    }
                }
            }
            "pitch" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        pitch = Some(array.value(0));
                    }
                }
            }
            "yaw" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        yaw = Some(array.value(0));
                    }
                }
            }
            "max_velocity" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    if !array.is_null(0) {
                        max_velocity = Some(array.value(0));
                    }
                }
            }
            "joint_angles" => {
                // Handle JSON string in string or dictionary column
                let json_str = if let Some(string_array) =
                    column.as_any().downcast_ref::<StringArray>()
                {
                    if !string_array.is_null(0) {
                        Some(string_array.value(0))
                    } else {
                        None
                    }
                } else if let Some(dict_array) = column
                    .as_any()
                    .downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>()
                {
                    if let Some(values) = dict_array.values().as_any().downcast_ref::<StringArray>()
                    {
                        let key = dict_array.keys().value(0) as usize;
                        Some(values.value(key))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(json_str) = json_str {
                    if json_str != "null" {
                        if let Ok(angles) = serde_json::from_str::<Vec<f64>>(json_str) {
                            joint_angles = Some(angles);
                        }
                    }
                }
            }
            "delta_joints" => {
                // Handle JSON string in string or dictionary column
                let json_str = if let Some(string_array) =
                    column.as_any().downcast_ref::<StringArray>()
                {
                    if !string_array.is_null(0) {
                        Some(string_array.value(0))
                    } else {
                        None
                    }
                } else if let Some(dict_array) = column
                    .as_any()
                    .downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>()
                {
                    if let Some(values) = dict_array.values().as_any().downcast_ref::<StringArray>()
                    {
                        let key = dict_array.keys().value(0) as usize;
                        Some(values.value(key))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(json_str) = json_str {
                    if json_str != "null" {
                        if let Ok(deltas) = serde_json::from_str::<Vec<f64>>(json_str) {
                            delta_joints = Some(deltas);
                        }
                    }
                }
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Convert command_type string to ArmCommand enum
    match command_type.as_str() {
        "cartesian_move" => Ok(ArmCommand::CartesianMove {
            x: x.unwrap_or(0.0),
            y: y.unwrap_or(0.0),
            z: z.unwrap_or(0.0),
            roll: roll.unwrap_or(0.0),
            pitch: pitch.unwrap_or(0.0),
            yaw: yaw.unwrap_or(0.0),
            max_velocity,
        }),
        "joint_position" => Ok(ArmCommand::JointPosition {
            joint_angles: joint_angles.unwrap_or_else(|| vec![0.0; 6]),
            max_velocity,
        }),
        "relative_move" => Ok(ArmCommand::RelativeMove {
            delta_joints: delta_joints.unwrap_or_else(|| vec![0.0; 6]),
        }),
        "stop" => Ok(ArmCommand::Stop),
        "home" => Ok(ArmCommand::Home),
        "emergency_stop" => Ok(ArmCommand::EmergencyStop),
        _ => Err(eyre::eyre!("Unknown command type: {}", command_type)),
    }
}

fn arrow_to_rover_command(base64_data: &str) -> Result<RoverCommand> {
    let batch = base64_to_arrow_batch(base64_data)?;

    if batch.num_rows() != 1 {
        return Err(eyre::eyre!(
            "Expected exactly one row in rover command batch"
        ));
    }

    // Get values by column name
    let schema = batch.schema();
    let mut throttle = 0.0;
    let mut brake = 0.0;
    let mut steering_angle = 0.0;
    let mut timestamp = 0u64;
    let mut command_id = String::new();

    for (i, field) in schema.fields().iter().enumerate() {
        let column = batch.column(i);

        match field.name().as_str() {
            "throttle" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    throttle = array.value(0);
                } else {
                    return Err(eyre::eyre!("Invalid throttle column type"));
                }
            }
            "brake" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    brake = array.value(0);
                } else {
                    return Err(eyre::eyre!("Invalid brake column type"));
                }
            }
            "steering_angle" => {
                if let Some(array) = column.as_any().downcast_ref::<Float64Array>() {
                    steering_angle = array.value(0);
                } else {
                    return Err(eyre::eyre!("Invalid steering_angle column type"));
                }
            }
            "timestamp" => {
                if let Some(array) = column.as_any().downcast_ref::<UInt64Array>() {
                    timestamp = array.value(0);
                } else {
                    return Err(eyre::eyre!("Invalid timestamp column type"));
                }
            }
            "command_id" => {
                // FIXED: Handle both regular strings and dictionary-encoded strings
                if let Some(string_array) = column.as_any().downcast_ref::<StringArray>() {
                    // Regular string array
                    command_id = string_array.value(0).to_string();
                } else if let Some(dict_array) = column
                    .as_any()
                    .downcast_ref::<arrow::array::DictionaryArray<arrow::datatypes::Int32Type>>()
                {
                    // Dictionary-encoded string array
                    if let Some(values) = dict_array.values().as_any().downcast_ref::<StringArray>()
                    {
                        let key = dict_array.keys().value(0) as usize;
                        command_id = values.value(key).to_string();
                    } else {
                        return Err(eyre::eyre!("Invalid dictionary values in command_id"));
                    }
                } else {
                    return Err(eyre::eyre!(
                        "Invalid command_id column type: {:?}",
                        column.data_type()
                    ));
                }
            }
            _ => {
                // Ignore unknown fields
                println!("Ignoring unknown field: {}", field.name());
            }
        }
    }

    Ok(RoverCommand {
        throttle,
        brake,
        steering_angle,
        timestamp,
        command_id,
    })
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoverCommandWithMetadata {
    command: RoverCommand,
    metadata: CommandMetadata,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ArrowMessage {
    message_type: String,
    schema_name: String,
    arrow_data: String, // base64 encoded Arrow data
    timestamp: u64,
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
    let socketio_handle =
        tokio::spawn(async move { start_web_socketio_server(shared_state_clone).await });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Wrap the node for sharing between tasks
    let node_arc = Arc::new(Mutex::new(node));
    let node_clone = node_arc.clone();
    let state_clone = shared_state.clone();
    let arm_output_clone = arm_command_output.clone();
    let rover_output_clone = rover_command_output.clone();

    // Start command processor task with actual node
    let command_processor_handle = tokio::spawn(async move {
        command_processor_loop(
            node_clone,
            state_clone,
            arm_output_clone,
            rover_output_clone,
        )
        .await;
    });

    println!("Web Bridge initialized with Apache Arrow support");
    println!("SocketIO server running on http://127.0.0.1:8080");
    println!("Using Arrow IPC format for data transfer");
    println!("Waiting for dora events and web client connections...");

    // Main event loop
    loop {
        let event_future = tokio::time::timeout(Duration::from_millis(50), async { events.recv() });

        if let Ok(Some(event)) = event_future.await {
            match event {
                Event::Input { id, data, .. } => {
                    let id_str = id.as_str();

                    match id_str {
                        "rover_telemetry" => {
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>()
                            {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    if let Ok(telemetry) =
                                        serde_json::from_slice::<RoverTelemetry>(bytes)
                                    {
                                        println!(
                                            "Received rover telemetry - converting to Arrow format"
                                        );
                                        if let Ok(mut rover_tel) =
                                            shared_state.latest_rover_telemetry.lock()
                                        {
                                            *rover_tel = Some(telemetry);
                                        }
                                    }
                                }
                            }
                        }

                        "arm_telemetry" => {
                            if let Some(bytes_array) = data.as_bytes_opt::<GenericBinaryType<i32>>()
                            {
                                if bytes_array.len() > 0 {
                                    let bytes = bytes_array.value(0);
                                    match serde_json::from_slice::<ArmTelemetry>(bytes) {
                                        Ok(telemetry) => {
                                            println!("Received arm telemetry - converting to Arrow format");
                                            println!("   End effector pose: [{:.3}, {:.3}, {:.3}, {:.3}, {:.3}, {:.3}]",
                                                     telemetry.end_effector_pose[0], telemetry.end_effector_pose[1], telemetry.end_effector_pose[2],
                                                     telemetry.end_effector_pose[3], telemetry.end_effector_pose[4], telemetry.end_effector_pose[5]);

                                            // Store the telemetry for broadcasting to web clients
                                            if let Ok(mut arm_tel) =
                                                shared_state.latest_arm_telemetry.lock()
                                            {
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
            "arrow_enabled": true,
            "supported_schemas": ["arm_telemetry", "rover_telemetry", "arm_command", "rover_command"],
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });

        if let Err(e) = socket.emit("status", welcome_data) {
            println!("Failed to send welcome message: {}", e);
        }

        // Handle arm commands from web clients
        socket.on("arrow_arm_command", {
            let state = state.clone();
            move |socket: SocketRef, Data::<ArrowMessage>(arrow_msg)| {
                println!("Received Arrow arm command from web: schema={}, type={}",
                         arrow_msg.schema_name, arrow_msg.message_type);

                if let Ok(mut stats) = state.stats.lock() {
                    stats.commands_received += 1;
                    stats.arrow_messages_received += 1;
                }

                // Validate schema
                if arrow_msg.schema_name != "arm_command" {
                    let error_response = serde_json::json!({
                        "type": "error",
                        "message": format!("Invalid schema: expected 'arm_command', got '{}'", arrow_msg.schema_name)
                    });
                    let _ = socket.emit("error", error_response);
                    return;
                }

                // Convert Arrow data to ArmCommand
                match arrow_to_arm_command(&arrow_msg.arrow_data) {
                    Ok(arm_command) => {
                        println!("Successfully converted Arrow data to arm command: {:?}", arm_command);

                        // Create command with metadata
                        let cmd_with_metadata = ArmCommandWithMetadata {
                            command: Some(arm_command),
                            metadata: create_metadata(),
                        };

                        // Queue the command for processing
                        if let Ok(mut queue) = state.arm_command_queue.lock() {
                            queue.push_back(cmd_with_metadata);
                            println!("Queued Arrow arm command for processing (queue size: {})", queue.len());
                        }
                    }
                    Err(e) => {
                        println!("Failed to convert Arrow arm command: {}", e);
                        let error_response = serde_json::json!({
                            "type": "error",
                            "message": format!("Invalid Arrow arm command: {}", e)
                        });
                        let _ = socket.emit("error", error_response);
                    }
                }
            }
        });

        // Handle rover commands from web clients
        socket.on("arrow_rover_command", {
            let state = state.clone();
            move |_socket: SocketRef, Data::<ArrowMessage>(arrow_msg)| {
                println!("Received Arrow rover command from web: schema={}, type={}",
                         arrow_msg.schema_name, arrow_msg.message_type);

                if let Ok(mut stats) = state.stats.lock() {
                    stats.commands_received += 1;
                    stats.arrow_messages_received += 1;
                }

                // Validate schema
                if arrow_msg.schema_name != "rover_command" {
                    println!("Invalid schema for rover command: {}", arrow_msg.schema_name);
                    return;
                }

                // Convert Arrow data to RoverCommand
                match arrow_to_rover_command(&arrow_msg.arrow_data) {
                    Ok(rover_command) => {
                        println!("Successfully converted Arrow data to rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                                 rover_command.throttle, rover_command.brake, rover_command.steering_angle);

                        // Create command with metadata
                        let cmd_with_metadata = RoverCommandWithMetadata {
                            command: rover_command,
                            metadata: create_metadata(),
                        };

                        // Queue the command for processing
                        if let Ok(mut queue) = state.rover_command_queue.lock() {
                            queue.push_back(cmd_with_metadata);
                            println!("Queued Arrow rover command for processing (queue size: {})", queue.len());
                        }
                    }
                    Err(e) => {
                        println!("Failed to convert Arrow rover command: {} {}", e, &state.rover_command_schema);
                    }
                }
            }
        });

        // Handle schema requests
        socket.on("get_schema", {
            let state = state.clone();
            move |socket: SocketRef, Data::<serde_json::Value>(request)| {
                if let Some(schema_name) = request.get("schema").and_then(|s| s.as_str()) {
                    let schema_response = match schema_name {
                        "arm_telemetry" => {
                            serde_json::json!({
                                "schema_name": "arm_telemetry",
                                "fields": state.arm_telemetry_schema.fields().iter().map(|f| {
                                    serde_json::json!({
                                        "name": f.name(),
                                        "data_type": format!("{:?}", f.data_type()),
                                        "nullable": f.is_nullable()
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }
                        "rover_telemetry" => {
                            serde_json::json!({
                                "schema_name": "rover_telemetry",
                                "fields": state.rover_telemetry_schema.fields().iter().map(|f| {
                                    serde_json::json!({
                                        "name": f.name(),
                                        "data_type": format!("{:?}", f.data_type()),
                                        "nullable": f.is_nullable()
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }
                        "arm_command" => {
                            serde_json::json!({
                                "schema_name": "arm_command",
                                "fields": state.arm_command_schema.fields().iter().map(|f| {
                                    serde_json::json!({
                                        "name": f.name(),
                                        "data_type": format!("{:?}", f.data_type()),
                                        "nullable": f.is_nullable()
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }
                        "rover_command" => {
                            serde_json::json!({
                                "schema_name": "rover_command",
                                "fields": state.rover_command_schema.fields().iter().map(|f| {
                                    serde_json::json!({
                                        "name": f.name(),
                                        "data_type": format!("{:?}", f.data_type()),
                                        "nullable": f.is_nullable()
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }
                        _ => {
                            serde_json::json!({
                                "error": format!("Unknown schema: {}", schema_name)
                            })
                        }
                    };

                    let _ = socket.emit("schema_response", schema_response);
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
                        "arrow_messages_sent": stats.arrow_messages_sent,
                        "arrow_messages_received": stats.arrow_messages_received,
                        "uptime_seconds": SystemTime::now()
                            .duration_since(stats.uptime_start)
                            .unwrap_or_default()
                            .as_secs(),
                        "arrow_enabled": true
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

    let app = axum::Router::new().layer(ServiceBuilder::new().layer(cors).layer(layer));

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("Web SocketIO server listening on 127.0.0.1:8080");
    println!("Test with: curl -X GET http://127.0.0.1:8080/socket.io/");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn telemetry_broadcaster_loop(socket: SocketRef, state: SharedState) {
    let mut interval = tokio::time::interval(Duration::from_millis(200)); // 5 Hz
    let mut telemetry_count = 0u64;

    loop {
        interval.tick().await;
        telemetry_count += 1;

        // Send arm telemetry if available
        if let Ok(arm_tel_opt) = state.latest_arm_telemetry.lock() {
            if let Some(ref telemetry) = *arm_tel_opt {
                match arm_telemetry_to_arrow(telemetry, &state.arm_telemetry_schema) {
                    Ok(arrow_data) => {
                        let arrow_message = ArrowMessage {
                            message_type: "telemetry".to_string(),
                            schema_name: "arm_telemetry".to_string(),
                            arrow_data,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        };

                        if socket.emit("arrow_telemetry", arrow_message).is_err() {
                            break; // Client disconnected
                        }

                        if let Ok(mut stats) = state.stats.lock() {
                            stats.arrow_messages_sent += 1;
                        }

                        if telemetry_count <= 5 || telemetry_count % 25 == 0 {
                            println!(
                                "Sent arm telemetry #{} as Arrow to client {}",
                                telemetry_count, socket.id
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed to convert arm telemetry to Arrow: {}", e);
                    }
                }
            }
        }

        // Send rover telemetry if available
        if let Ok(rover_tel_opt) = state.latest_rover_telemetry.lock() {
            if let Some(ref telemetry) = *rover_tel_opt {
                match rover_telemetry_to_arrow(telemetry, &state.rover_telemetry_schema) {
                    Ok(arrow_data) => {
                        let arrow_message = ArrowMessage {
                            message_type: "telemetry".to_string(),
                            schema_name: "rover_telemetry".to_string(),
                            arrow_data,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        };

                        if socket.emit("arrow_telemetry", arrow_message).is_err() {
                            break; // Client disconnected
                        }

                        if let Ok(mut stats) = state.stats.lock() {
                            stats.arrow_messages_sent += 1;
                        }

                        if telemetry_count <= 5 || telemetry_count % 25 == 0 {
                            println!(
                                "Sent rover telemetry #{} as Arrow to client {}",
                                telemetry_count, socket.id
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed to convert rover telemetry to Arrow: {}", e);
                    }
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

    println!("Starting Arrow-enabled command processor loop");

    loop {
        interval.tick().await;

        // Process arm command queue
        if let Ok(mut arm_queue) = state.arm_command_queue.lock() {
            while let Some(cmd_with_metadata) = arm_queue.pop_front() {
                println!(
                    "Processing queued Arrow arm command: {:?}",
                    cmd_with_metadata.command
                );

                // Send via dora node (still using JSON internally for dora compatibility)
                match serde_json::to_vec(&cmd_with_metadata) {
                    Ok(serialized) => {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                        if let Ok(mut node_guard) = node.lock() {
                            match node_guard.send_output(
                                arm_command_output.clone(),
                                Default::default(),
                                arrow_data,
                            ) {
                                Ok(_) => {
                                    println!(
                                        "Arrow ARM command sent to dora dataflow successfully"
                                    );
                                    if let Ok(mut stats) = state.stats.lock() {
                                        stats.commands_sent += 1;
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to send Arrow arm command via dora: {}", e);
                                }
                            }
                        } else {
                            println!("Failed to acquire node lock for Arrow arm command");
                        }
                    }
                    Err(e) => {
                        println!("Failed to serialize Arrow arm command: {}", e);
                    }
                }
            }
        }

        // Process rover command queue
        if let Ok(mut rover_queue) = state.rover_command_queue.lock() {
            while let Some(cmd_with_metadata) = rover_queue.pop_front() {
                println!("Processing queued Arrow rover command: throttle={:.2}, brake={:.2}, steer={:.2}",
                         cmd_with_metadata.command.throttle,
                         cmd_with_metadata.command.brake,
                         cmd_with_metadata.command.steering_angle);

                // Send via dora node
                match serde_json::to_vec(&cmd_with_metadata) {
                    Ok(serialized) => {
                        let arrow_data = BinaryArray::from_vec(vec![serialized.as_slice()]);

                        if let Ok(mut node_guard) = node.lock() {
                            match node_guard.send_output(
                                rover_command_output.clone(),
                                Default::default(),
                                arrow_data,
                            ) {
                                Ok(_) => {
                                    println!(
                                        "Arrow ROVER command sent to dora dataflow successfully"
                                    );
                                    if let Ok(mut stats) = state.stats.lock() {
                                        stats.commands_sent += 1;
                                    }
                                }
                                Err(e) => {
                                    println!("Failed to send Arrow rover command via dora: {}", e);
                                }
                            }
                        } else {
                            println!("Failed to acquire node lock for Arrow rover command");
                        }
                    }
                    Err(e) => {
                        println!("Failed to serialize Arrow rover command: {}", e);
                    }
                }
            }
        }
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
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_default(subscriber)
}
