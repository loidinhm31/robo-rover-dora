export interface LogEntry {
  timestamp: string;
  message: string;
  type: "info" | "success" | "error" | "warning";
}

// LeKiwi 6DOF Arm Joint Positions
export interface JointPositions {
  shoulder_pan: number; // Joint 1: Base rotation (-3.14 to 3.14 rad)
  shoulder_lift: number; // Joint 2: Shoulder pitch (-1.57 to 1.57 rad)
  elbow_flex: number; // Joint 3: Elbow pitch (-2.09 to 2.09 rad)
  wrist_flex: number; // Joint 4: Wrist pitch (-3.14 to 3.14 rad)
  wrist_roll: number; // Joint 5: Wrist roll (-1.57 to 1.57 rad)
  gripper: number; // Joint 6: Gripper (-3.14 to 3.14 rad)
  wheel1?: number;
  wheel2?: number;
  wheel3?: number;
}

// Web Arm Command structure matching Rust WebArmCommand
export interface WebArmCommand {
  command_type: "joint_position" | "cartesian" | "home" | "stop";
  joint_positions?: JointPositions;
  max_velocity?: number;
}

// Web Rover Command structure matching Rust WebRoverCommand
export interface WebRoverCommand {
  command_type: "velocity" | "joint_positions" | "stop";
  // For velocity commands (omnidirectional mecanum wheels)
  v_x?: number; // Linear velocity in x (m/s)
  v_y?: number; // Linear velocity in y (m/s)
  omega_z?: number; // Angular velocity (rad/s)
  // For direct joint control
  wheel1?: number;
  wheel2?: number;
  wheel3?: number;
}

// Telemetry structures (kept from original)
export interface ArmTelemetry {
  end_effector_pose: number[];
  joint_angles?: number[];
  joint_velocities?: number[];
  is_moving: boolean;
  source: string;
  timestamp: number;
}

export interface RoverTelemetry {
  position: [number, number];
  yaw: number;
  pitch: number;
  roll: number;
  velocity: number;
  timestamp: number;
  near_sample: boolean;
  picking_up: boolean;
  nav_angles?: number[];
  nav_dists?: number[];
}

// Connection state
export interface ConnectionState {
  isConnected: boolean;
  clientId: string | null;
  commandsSent: number;
  commandsReceived: number;
}

// Joint limit constants
export const JOINT_LIMITS = {
  shoulder_pan: { min: -3.14, max: 3.14 },
  shoulder_lift: { min: -1.57, max: 1.57 },
  elbow_flex: { min: -2.09, max: 2.09 },
  wrist_flex: { min: -3.14, max: 3.14 },
  wrist_roll: { min: -1.57, max: 1.57 },
  gripper: { min: -3.14, max: 3.14 },
};

// Helper function to create home position
export function createHomePosition(): JointPositions {
  return {
    shoulder_pan: 0.0,
    shoulder_lift: 0.0,
    elbow_flex: 0.0,
    wrist_flex: 0.0,
    wrist_roll: 0.0,
    gripper: 0.0,
  };
}

// Helper function to validate joint positions
export function validateJointPositions(positions: JointPositions): string | null {
  const checks: Array<[keyof JointPositions, { min: number; max: number }]> = [
    ["shoulder_pan", JOINT_LIMITS.shoulder_pan],
    ["shoulder_lift", JOINT_LIMITS.shoulder_lift],
    ["elbow_flex", JOINT_LIMITS.elbow_flex],
    ["wrist_flex", JOINT_LIMITS.wrist_flex],
    ["wrist_roll", JOINT_LIMITS.wrist_roll],
    ["gripper", JOINT_LIMITS.gripper],
  ];

  for (const [joint, limits] of checks) {
    const value = positions[joint];
    if (value! < limits.min || value! > limits.max) {
      return `${joint} out of range: ${value!.toFixed(3)} (expected ${limits.min.toFixed(2)} to ${limits.max.toFixed(2)})`;
    }
  }

  return null;
}

export interface VideoFrame {
  timestamp: number;
  frame_id: number;
  width: number;
  height: number;
  format: string; // "JPEG", "PNG", "WEBP"
  quality: number; // 1-100 for JPEG
  data: string; // base64 encoded image data
  overlay_data?: OverlayData;
}

export interface OverlayData {
  rover_position?: [number, number];
  rover_velocity?: number;
  arm_position?: number[];
  battery_level?: number;
  signal_strength?: number;
  timestamp_text: string;
}

export interface VideoStats {
  timestamp: number;
  frames_processed: number;
  frames_dropped: number;
  avg_frame_size_kb: number;
  avg_processing_time_ms: number;
  current_fps: number;
  bandwidth_kbps: number;
}

export interface VideoControl {
  command: VideoCommand;
  quality?: VideoQuality;
  max_fps?: number;
}

export type VideoCommand = "start" | "stop" | "pause" | "resume" | "change_quality";

export enum VideoQuality {
  Low = "low", // 320x240, JPEG quality 60
  Medium = "medium", // 640x480, JPEG quality 75
  High = "high", // 640x480, JPEG quality 90
  UltraHigh = "ultra_high", // 1280x720, JPEG quality 95
}

export interface CameraStatus {
  is_active: boolean;
  fps: number;
  dropped_frames: number;
  capture_errors: number;
  last_frame_timestamp: number;
}

// Socket.IO event types for type-safe communication
export interface ServerToClientEvents {
  video_frame: (frame: VideoFrame) => void;
  video_stats: (stats: VideoStats) => void;
  video_status: (status: { streaming: boolean; fps?: number }) => void;
  arm_telemetry: (telemetry: ArmTelemetry) => void;
  rover_telemetry: (telemetry: RoverTelemetry) => void;
}

export interface ClientToServerEvents {
  arm_command: (command: WebArmCommand) => void;
  rover_command: (command: WebRoverCommand) => void;
  video_control: (control: VideoControl) => void;
}




