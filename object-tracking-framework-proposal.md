# Object Detection and Tracking Framework for Robo-Rover-Dora

## Project Overview
Framework proposal for developing object detection and tracking capabilities for the robo-rover-dora project, built on the dora-rs framework using Apache Arrow for event-driven data transfer between nodes.

---

## Proposed Framework Architecture

### 1. Vision Pipeline (New Nodes)

#### A. Object Detection Node (`object-detector`)

**Inputs:**
- `frame`: RGB8 frames from `gst-camera/frame`

**Outputs:**
- `detections`: Object detection results containing:
  - Bounding boxes (x, y, width, height)
  - Class labels (person, dog, cat, etc.)
  - Confidence scores
  - Detection timestamp

**Key Responsibilities:**
- Run inference on incoming frames (~10-30 FPS)
- Use lightweight model (YOLOv12n)
- Filter detections by confidence threshold
- Normalize coordinates to frame dimensions

---

#### B. Object Tracking Node (`object-tracker`)

**Inputs:**
- `detections`: From `object-detector/detections`
- `tracking_command`: From `web-bridge/tracking_command`

**Outputs:**
- `tracked_object`: Current tracked object state containing:
  - Object ID (persistent across frames)
  - Position (center x, y in image coordinates)
  - Bounding box dimensions
  - Velocity estimation (pixel/frame)
  - Tracking confidence
  - Object class

**Key Responsibilities:**
- Maintain tracking state using algorithms (DeepSORT, SORT, or ByteTrack)
- Handle occlusion and re-identification
- Accept target selection commands from web UI
- Smooth position/velocity estimates with Kalman filter
- Auto-release tracking if object lost for N frames

---

#### C. Visual Servoing Node (`visual-servo-controller`)

**Inputs:**
- `tracked_object`: From `object-tracker/tracked_object`
- `rover_telemetry`: From `sim-interface/rover_telemetry`
- `camera_params`: Camera intrinsics and mounting configuration

**Outputs:**
- `servo_command`: Rover velocity commands to `rover-controller`

**Key Responsibilities:**
- Convert image coordinates to world coordinates
- Estimate object distance using:
  - Bounding box height (assuming known object size)
  - Depth from stereo/monocular estimation (future enhancement)
- Implement control algorithm:
  - **Lateral control**: Keep object centered (PID on x-offset)
  - **Longitudinal control**: Maintain minimum distance (PID on estimated depth)
  - **Smooth velocity commands**: Rate limiting and acceleration control
- Safety: Stop if object too close or lost

---

### 2. Web UI Enhancements (Existing `web-bridge` Node)

#### Extended Inputs:
- `detections`: For overlay visualization
- `tracked_object`: For highlighting selected target

#### Extended Outputs:
- `tracking_command`: New command type for:
  - `SelectTarget { detection_id, class_label }`
  - `ReleaseTarget`
  - `SetMinDistance { distance_meters }`

#### UI Features:
- Real-time bounding box overlay on video stream
- Clickable detections to select tracking target
- Visual indicator for currently tracked object
- Distance display and minimum distance slider
- Tracking status (Active, Lost, Searching)
- Auto-follow toggle button

---

### 3. Enhanced Rover Controller Integration

#### Command Priority System (Already exists in codebase):
```
Emergency (4) > Autonomous Tracking (3) > Manual Control (2) > Default (1)
```

#### Command Arbitration:
- **Manual override**: Web UI commands always have higher priority
- **Autonomous mode**: `visual-servo-controller` commands active when tracking enabled
- **Safety layer**: Emergency stop if collision imminent

---

### 4. Data Flow Architecture

```
┌─────────────────┐
│  gst-camera     │ (30 FPS)
│  (kornia)       │
└────────┬────────┘
         │ frame (RGB8)
         ├──────────────────────┐
         │                      │
         v                      v
┌────────────────┐    ┌─────────────────┐
│ object-        │    │  web-bridge     │
│ detector       │    │  (video stream) │
└────────┬───────┘    └─────────────────┘
         │ detections           │ tracking_command
         v                      v
┌────────────────┐    ┌─────────────────┐
│ object-        │◄───┤  web UI         │
│ tracker        │    │  (selection)    │
└────────┬───────┘    └─────────────────┘
         │ tracked_object
         v
┌────────────────┐
│ visual-servo-  │
│ controller     │
└────────┬───────┘
         │ servo_command
         v
┌────────────────┐    ┌─────────────────┐
│ rover-         │◄───┤ web-bridge      │
│ controller     │    │ (manual cmds)   │
└────────┬───────┘    └─────────────────┘
         │ processed_rover_command
         v
┌────────────────┐
│ sim-interface  │
└────────┬───────┘
         │ rover_telemetry (feedback)
         └─────────────────────┘
```

---

### 5. Configuration & Tuning Parameters

#### Detection Node:
- Model selection (YOLO variant, model size)
- Confidence threshold (default: 0.5)
- NMS threshold (default: 0.4)
- Classes to detect (configurable filter list)
- Inference device (CPU/GPU/NPU)

#### Tracking Node:
- Tracking algorithm parameters (IOU threshold, etc.)
- Max tracking age (frames before release)
- Position smoothing window size
- Re-identification confidence threshold

#### Visual Servoing:
- PID gains for lateral control (Kp, Ki, Kd)
- PID gains for longitudinal control
- Minimum safe distance (0.5-2.0m configurable)
- Maximum velocity limits
- Camera calibration data (focal length, principal point)
- Camera mounting parameters (height, tilt angle)

---

### 6. Message Types (Extend `robo_rover_lib`)

#### Detection Message:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,      // Top-left x coordinate
    pub y: f32,      // Top-left y coordinate
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub detection_id: u64,
    pub bbox: BoundingBox,
    pub class_id: u32,
    pub class_label: String,
    pub confidence: f32,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionFrame {
    pub detections: Vec<Detection>,
    pub frame_id: u64,
    pub timestamp: u64,
}
```

#### Tracked Object State:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedObject {
    pub object_id: u64,
    pub position: (f32, f32),        // Image coordinates (center)
    pub bbox: BoundingBox,
    pub velocity: (f32, f32),        // Pixels per frame
    pub estimated_distance: Option<f32>,  // Meters from rover
    pub class_label: String,
    pub tracking_confidence: f32,
    pub frames_tracked: u32,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingState {
    pub tracked_object: Option<TrackedObject>,
    pub tracking_active: bool,
    pub tracking_status: TrackingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackingStatus {
    NoTarget,
    Tracking,
    Lost,
    Searching,
}
```

#### Tracking Commands:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackingCommand {
    SelectTarget {
        detection_id: u64,
        class_label: String,
    },
    ReleaseTarget,
    SetMinDistance {
        distance: f32,  // Meters
    },
    SetMaxVelocity {
        velocity: f32,  // M/s
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingCommandWithMetadata {
    pub command: TrackingCommand,
    pub metadata: CommandMetadata,  // Reuse existing metadata struct
}
```

#### Visual Servoing Commands:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualServoCommand {
    pub linear_velocity: f32,   // Forward/backward (m/s)
    pub angular_velocity: f32,  // Rotation (rad/s)
    pub target_distance: f32,   // Current estimated distance
    pub control_mode: ServoMode,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServoMode {
    CenterAndApproach,  // Keep centered and maintain distance
    CenterOnly,         // Only keep centered
    ApproachOnly,       // Only maintain distance
}
```

---

### 7. Implementation Phases

#### Phase 1: Object Detection Node (Week 1-2)
- Set up Rust inference framework (ONNX Runtime or Candle)
- Download and integrate pre-trained model (YOLOv8n recommended)
- Create dora-rs node structure
- Connect to `gst-camera/frame` input
- Implement detection output serialization
- Basic testing with static images and live stream

**Deliverables:**
- `object-detector` node implementation
- Detection message types in `robo_rover_lib`
- Unit tests for detection pipeline

---

#### Phase 2: Web UI Visualization (Week 2-3)
- Extend web-bridge to receive detection data
- Implement bounding box overlay on video stream
- Add class labels and confidence scores to UI
- Create detection list/panel in UI
- Implement visual styling (colors per class)

**Deliverables:**
- Enhanced web UI with detection overlay
- Real-time detection visualization
- Performance metrics display (FPS, detection count)

---

#### Phase 3: Object Tracking (Week 3-4)
- Implement tracking algorithm (SORT/DeepSORT)
- Create `object-tracker` node
- Handle target selection from web UI
- Implement Kalman filter for position smoothing
- Add re-identification logic
- Tracking state management (active/lost/searching)

**Deliverables:**
- `object-tracker` node implementation
- Tracking message types
- Web UI target selection mechanism
- Tracking status indicators

---

#### Phase 4: Visual Servoing (Week 4-5)
- Implement distance estimation algorithm
- Create PID controllers for lateral and longitudinal control
- Develop `visual-servo-controller` node
- Add safety constraints (minimum distance, max velocity)
- Integrate with existing rover controller command priority system
- Implement smooth velocity transitions

**Deliverables:**
- `visual-servo-controller` node
- Tuned PID parameters
- Safety constraint implementation
- Distance estimation calibration

---

#### Phase 5: Integration & Tuning (Week 5-6)
- End-to-end system testing
- Parameter optimization (detection threshold, PID gains)
- Failure mode testing (object loss, occlusion)
- Performance optimization (latency reduction)
- Documentation and user guide
- Edge case handling

**Deliverables:**
- Fully integrated tracking system
- Configuration documentation
- User operation guide
- Performance benchmarks

---

### 8. Technical Considerations

#### Model Selection:
- **YOLOv8n**: Best balance of speed and accuracy
- **YOLO-World**: Open-vocabulary detection (can detect any object)
- **MobileNet-SSD**: Fastest but less accurate
- **YOLOv5s/v7-tiny**: Good alternatives

#### Inference Framework:
- **ONNX Runtime**: Cross-platform, mature
- **Candle**: Rust-native, better integration
- **TensorRT**: Best performance (NVIDIA GPUs only)

#### Distance Estimation Methods:
1. **Monocular Depth Estimation**: Single-camera depth prediction using DNN
2. **Known Object Size**: Use typical heights (person ~1.7m, dog ~0.5m)
3. **Bounding Box Height**: Inverse relationship with distance
4. **Ground Plane Assumption**: Camera geometry-based estimation

#### Control Strategy:
- **PID Control**: Simple, tunable, proven
- **Model Predictive Control (MPC)**: Better performance, more complex
- **Pure Pursuit**: Alternative for path following
- **Look-ahead distance**: Adaptive based on velocity

---

### 9. Testing Strategy

#### Unit Tests:
- Detection inference on test images
- Tracking algorithm with synthetic trajectories
- PID controller step responses
- Message serialization/deserialization

#### Integration Tests:
- Detection → Tracking pipeline
- Tracking → Visual Servoing pipeline
- Web UI command handling
- Priority system with manual override

#### System Tests:
- Follow stationary object
- Follow moving object (constant velocity)
- Follow moving object (changing direction)
- Object occlusion recovery
- Multiple objects (correct target selection)
- Distance maintenance accuracy

---

### 10. Performance Targets

#### Latency Budget:
- Camera capture to detection: < 50ms
- Detection to tracking: < 10ms
- Tracking to servo command: < 10ms
- Total perception-to-action: < 100ms (10 Hz control loop)

#### Accuracy Targets:
- Detection confidence: > 0.7 for selected objects
- Tracking stability: > 90% retention over 30 seconds
- Distance estimation error: < 20% at 1-3m range
- Centering error: < 50 pixels at 640x480 resolution

#### Resource Constraints:
- CPU usage: < 50% on target platform
- Memory: < 2GB for vision pipeline
- Network bandwidth: Maintain video stream quality

---

### 11. Safety & Edge Cases

#### Safety Measures:
- Minimum approach distance enforcement (configurable)
- Maximum velocity limits
- Emergency stop on tracking loss
- Manual override always available
- Collision avoidance (future: integrate with nav sensors)

#### Edge Case Handling:
- **Object leaves frame**: Enter searching mode, stop movement
- **Multiple similar objects**: Track by ID, show UI warning
- **Occlusion**: Maintain last known position, predict trajectory
- **Poor lighting**: Reduce confidence threshold, show warning
- **Motion blur**: Skip frames, request lower velocity
- **Network lag**: Buffering strategy, stale data detection

---

### 12. Future Enhancements

#### Short-term:
- Multi-object tracking (track group formation)
- Depth sensor integration (RealSense, Kinect)
- Path prediction for moving objects
- Obstacle avoidance while following

#### Long-term:
- Semantic understanding (detect "walking human" vs "sitting human")
- Gesture recognition for control
- Re-identification after long occlusion
- 3D trajectory planning
- Integration with SLAM for world-frame tracking

---

### 13. Development Environment Setup

#### Dependencies:
```toml
# Add to workspace Cargo.toml
[dependencies]
# ML inference
ort = "2.0"  # ONNX Runtime
# or
candle-core = "0.3"
candle-nn = "0.3"

# Image processing
image = "0.24"
imageproc = "0.23"

# Numerical computing
ndarray = "0.15"
nalgebra = "0.32"

# Tracking algorithms
linfa = "0.7"  # For Kalman filter
```

#### Model Download:
```bash
# YOLOv8n ONNX model
wget https://github.com/ultralytics/assets/releases/download/v0.0.0/yolov8n.onnx

# Or export from PyTorch
pip install ultralytics
yolo export model=yolov8n.pt format=onnx
```

---

### 14. Dataflow YAML Configuration

```yaml
nodes:
  # Existing nodes...
  
  # Object detection node
  - id: object-detector
    build: cargo build --release -p object_detector
    path: target/release/object_detector
    inputs:
      frame: gst-camera/frame
    outputs:
      - detections
    env:
      MODEL_PATH: "models/yolov8n.onnx"
      CONFIDENCE_THRESHOLD: "0.5"
      NMS_THRESHOLD: "0.4"
      TARGET_CLASSES: "person,dog,cat"
      
  # Object tracking node
  - id: object-tracker
    build: cargo build --release -p object_tracker
    path: target/release/object_tracker
    inputs:
      detections: object-detector/detections
      tracking_command: web-bridge/tracking_command
    outputs:
      - tracked_object
      - tracking_state
    env:
      MAX_TRACKING_AGE: "30"
      IOU_THRESHOLD: "0.3"
      
  # Visual servoing controller
  - id: visual-servo-controller
    build: cargo build --release -p visual_servo_controller
    path: target/release/visual_servo_controller
    inputs:
      tracked_object: object-tracker/tracked_object
      rover_telemetry: sim-interface/rover_telemetry
    outputs:
      - servo_command
    env:
      MIN_DISTANCE: "1.0"
      MAX_VELOCITY: "0.5"
      LATERAL_PID_KP: "0.005"
      LATERAL_PID_KI: "0.0"
      LATERAL_PID_KD: "0.001"
      LONGITUDINAL_PID_KP: "0.3"
      LONGITUDINAL_PID_KI: "0.0"
      LONGITUDINAL_PID_KD: "0.05"
      
  # Enhanced web-bridge
  - id: web-bridge
    # ... existing config ...
    inputs:
      # ... existing inputs ...
      detections: object-detector/detections
      tracked_object: object-tracker/tracked_object
      tracking_state: object-tracker/tracking_state
    outputs:
      # ... existing outputs ...
      - tracking_command
      
  # Enhanced rover-controller
  - id: rover-controller
    # ... existing config ...
    inputs:
      # ... existing inputs ...
      servo_command: visual-servo-controller/servo_command
```

---

### 15. Summary

This framework provides a modular, extensible architecture for object detection and tracking on the robo-rover-dora platform. The design:

- **Leverages existing infrastructure**: Builds on current dora-rs nodes and command system
- **Modular design**: Each node has clear responsibilities and can be tested independently
- **Safety-first**: Multiple layers of safety constraints and manual override
- **Scalable**: Easy to add new detection models, tracking algorithms, or control strategies
- **Real-time capable**: Designed for < 100ms latency perception-to-action loop
- **User-friendly**: Intuitive web UI for target selection and monitoring

The phased implementation approach allows for incremental development and testing, reducing integration risks and enabling early demos of partial functionality.

---

## Next Steps

1. Review and approve framework architecture
2. Set up development environment with ML dependencies
3. Begin Phase 1: Object Detection Node implementation
4. Establish testing infrastructure for vision pipeline
5. Create initial benchmarks for performance tracking

---

**Document Version:** 1.0  
**Date:** 2025-10-29  
**Author:** Framework Architecture Proposal for robo-rover-dora
