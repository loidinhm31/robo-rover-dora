// Command types for controlling the rover and arm

export interface JointPositions {
  shoulder_pan: number;
  shoulder_lift: number;
  elbow_flex: number;
  wrist_flex: number;
  wrist_roll: number;
  gripper: number;
  wheel1?: number;
  wheel2?: number;
  wheel3?: number;
}

export interface WebArmCommand {
  command_type: "joint_position" | "cartesian" | "home" | "stop";
  joint_positions?: JointPositions;
  max_velocity?: number;
}

export interface WebRoverCommand {
  command_type: "velocity" | "joint_positions" | "stop";
  v_x?: number;
  v_y?: number;
  omega_z?: number;
  wheel1?: number;
  wheel2?: number;
  wheel3?: number;
}

export interface WebTrackingCommand {
  command_type: "enable" | "disable" | "enable_detection" | "disable_detection" | "select_target" | "clear_target";
  tracking_id?: number;
  detection_index?: number;
}
