export interface ArmTelemetry {
  type: "arm_telemetry";
  end_effector_pose: number[];
  joint_angles?: number[];
  joint_velocities?: number[];
  is_moving: boolean;
  source?: string;
  timestamp: number;
}

export interface RoverTelemetry {
  type: "rover_telemetry";
  position: [number, number];
  yaw: number;
  velocity: number;
  timestamp: number;
}

export interface LogEntry {
  timestamp: string;
  message: string;
  type: "info" | "success" | "error" | "warning";
}

export interface ConnectionState {
  isConnected: boolean;
  clientId: string | null;
  commandsSent: number;
  commandsReceived: number;
}

export interface KeyboardState {
  [key: string]: boolean;
}
