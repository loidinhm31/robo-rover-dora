use dora_node_api::{
    self,
    arrow::array::{Array, BinaryArray},
    dora_core::config::DataId,
    DoraNode, Event, Parameter,
};
use kornia_io::gstreamer::{RTSPCameraConfig, V4L2CameraConfig};
use robo_rover_lib::{CameraAction, CameraControl};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting capture node");

    // parse env variables
    let source_type =
        std::env::var("SOURCE_TYPE").map_err(|e| format!("SOURCE_TYPE error: {e}"))?;
    let source_uri = std::env::var("SOURCE_URI").map_err(|e| format!("SOURCE_URI error: {e}"))?;

    let output = DataId::from("frame".to_owned());
    let (mut node, mut events) = DoraNode::init_from_env()?;

    // Match on camera type and handle each separately
    match source_type.as_str() {
        "webcam" => {
            let image_cols = std::env::var("IMAGE_COLS")
                .map_err(|e| format!("IMAGE_COLS error: {e}"))?
                .parse::<usize>()?;
            let image_rows = std::env::var("IMAGE_ROWS")
                .map_err(|e| format!("IMAGE_ROWS error: {e}"))?
                .parse::<usize>()?;
            let source_fps = std::env::var("SOURCE_FPS")
                .map_err(|e| format!("SOURCE_FPS error: {e}"))?
                .parse::<u32>()?;

            let mut camera_opt = Some(
                V4L2CameraConfig::new()
                    .with_size([image_cols, image_rows].into())
                    .with_fps(source_fps)
                    .with_device(&source_uri)
                    .build()?
            );

            camera_opt.as_mut().unwrap().start()?;
            println!("Camera started successfully");

            while let Some(event) = events.recv() {
                match event {
                    Event::Input {
                        id,
                        metadata,
                        data,
                    } => match id.as_str() {
                        "tick" => {
                            if let Some(ref mut camera) = camera_opt {
                                let Some(frame) = camera.grab_rgb8()? else {
                                    continue;
                                };

                                let mut params = metadata.parameters;
                                params.insert("encoding".to_owned(), Parameter::String("RGB8".to_string()));
                                params.insert(
                                    "height".to_owned(),
                                    Parameter::Integer(frame.size().height as i64),
                                );
                                params.insert(
                                    "width".to_owned(),
                                    Parameter::Integer(frame.size().width as i64),
                                );

                                node.send_output_bytes(
                                    output.clone(),
                                    params,
                                    frame.numel(),
                                    frame.as_slice(),
                                )?;
                            }
                        }
                        "camera_control" => {
                            if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                                if binary_array.len() > 0 {
                                    let control_bytes = binary_array.value(0);
                                    if let Ok(camera_control) =
                                        serde_json::from_slice::<CameraControl>(control_bytes)
                                    {
                                        println!("Camera control received: {:?}", camera_control.command);
                                        match camera_control.command {
                                            CameraAction::Start => {
                                                if camera_opt.is_none() {
                                                    println!("Starting camera...");
                                                    let new_camera = V4L2CameraConfig::new()
                                                        .with_size([image_cols, image_rows].into())
                                                        .with_fps(source_fps)
                                                        .with_device(&source_uri)
                                                        .build()?;
                                                    new_camera.start()?;
                                                    camera_opt = Some(new_camera);
                                                    println!("Camera started");
                                                }
                                            }
                                            CameraAction::Stop => {
                                                if let Some(camera) = camera_opt.take() {
                                                    println!("Stopping camera...");
                                                    camera.close()?;
                                                    println!("Camera stopped");
                                                }
                                            }
                                        }
                                    } else {
                                        eprintln!("Failed to parse camera control command");
                                    }
                                }
                            }
                        }
                        other => eprintln!("Ignoring unexpected input: {other}"),
                    },
                    Event::Stop(_) => {
                        println!("Stop event received, closing camera");
                        if let Some(camera) = camera_opt.take() {
                            camera.close()?;
                        }
                        break;
                    }
                    other => eprintln!("Received unexpected event: {other:?}"),
                }
            }
        }
        "rtsp" => {
            let mut camera_opt = Some(RTSPCameraConfig::new().with_url(&source_uri).build()?);

            camera_opt.as_mut().unwrap().start()?;
            println!("RTSP Camera started successfully");

            while let Some(event) = events.recv() {
                match event {
                    Event::Input {
                        id,
                        metadata,
                        data,
                    } => match id.as_str() {
                        "tick" => {
                            if let Some(ref mut camera) = camera_opt {
                                let Some(frame) = camera.grab_rgb8()? else {
                                    continue;
                                };

                                let mut params = metadata.parameters;
                                params.insert("encoding".to_owned(), Parameter::String("RGB8".to_string()));
                                params.insert(
                                    "height".to_owned(),
                                    Parameter::Integer(frame.size().height as i64),
                                );
                                params.insert(
                                    "width".to_owned(),
                                    Parameter::Integer(frame.size().width as i64),
                                );

                                node.send_output_bytes(
                                    output.clone(),
                                    params,
                                    frame.numel(),
                                    frame.as_slice(),
                                )?;
                            }
                        }
                        "camera_control" => {
                            if let Some(binary_array) = data.as_any().downcast_ref::<BinaryArray>() {
                                if binary_array.len() > 0 {
                                    let control_bytes = binary_array.value(0);
                                    if let Ok(camera_control) =
                                        serde_json::from_slice::<CameraControl>(control_bytes)
                                    {
                                        println!("Camera control received: {:?}", camera_control.command);
                                        match camera_control.command {
                                            CameraAction::Start => {
                                                if camera_opt.is_none() {
                                                    println!("Starting RTSP camera...");
                                                    let new_camera = RTSPCameraConfig::new().with_url(&source_uri).build()?;
                                                    new_camera.start()?;
                                                    camera_opt = Some(new_camera);
                                                    println!("RTSP camera started");
                                                }
                                            }
                                            CameraAction::Stop => {
                                                if let Some(camera) = camera_opt.take() {
                                                    println!("Stopping RTSP camera...");
                                                    camera.close()?;
                                                    println!("RTSP camera stopped");
                                                }
                                            }
                                        }
                                    } else {
                                        eprintln!("Failed to parse camera control command");
                                    }
                                }
                            }
                        }
                        other => eprintln!("Ignoring unexpected input: {other}"),
                    },
                    Event::Stop(_) => {
                        println!("Stop event received, closing RTSP camera");
                        if let Some(camera) = camera_opt.take() {
                            camera.close()?;
                        }
                        break;
                    }
                    other => eprintln!("Received unexpected event: {other:?}"),
                }
            }
        }
        _ => return Err(format!("Invalid source type: {source_type}").into()),
    }

    Ok(())
}
