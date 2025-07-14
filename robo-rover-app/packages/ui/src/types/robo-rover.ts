export interface LogEntry {
  timestamp: string;
  message: string;
  type: "info" | "success" | "error" | "warning";
}

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

export interface ArmCommand {
  type:
    | "cartesian_move"
    | "joint_position"
    | "relative_move"
    | "stop"
    | "home"
    | "emergency_stop";
  x?: number;
  y?: number;
  z?: number;
  roll?: number;
  pitch?: number;
  yaw?: number;
  max_velocity?: number;
  joint_angles?: number[];
  delta_joints?: number[];
}

export interface RoverCommand {
  throttle: number;
  brake: number;
  steering_angle: number;
}

export interface ArrowMessage {
  message_type: string;
  schema_name: string;
  arrow_data: string; // base64 encoded Arrow data
  timestamp: number;
}

export interface ConnectionState {
  isConnected: boolean;
  clientId: string | null;
  commandsSent: number;
  commandsReceived: number;
  arrowEnabled: boolean;
  schemasLoaded: boolean;
}

export interface KeyboardState {
  [key: string]: boolean;
}
