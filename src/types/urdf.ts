/**
 * URDF Visualization Types for LeKiwi Robot
 *
 * Type definitions for joint positions, presets, trajectories, and animations.
 * Adapted from hexapod repository patterns to work with URDF-based robots.
 */

// ============================================================================
// Joint Position Interfaces
// ============================================================================

/**
 * Joint positions for LeKiwi robot (9 controllable joints)
 *
 * Mobile base: 3 omni-directional wheels (continuous rotation)
 * Robotic arm: 6 DOF manipulator
 */
export interface LeKiwiJointPositions {
  // Mobile base wheels (continuous joints - no limits)
  "ST3215_Servo_Motor-v1-2_Revolute-60": number;  // wheel_1
  "ST3215_Servo_Motor-v1-1_Revolute-62": number;  // wheel_2
  "ST3215_Servo_Motor-v1_Revolute-64": number;    // wheel_3

  // Robotic arm joints (6 DOF)
  "STS3215_03a-v1_Revolute-45": number;           // shoulder_pan
  "STS3215_03a-v1-1_Revolute-49": number;         // shoulder_lift
  "STS3215_03a-v1-2_Revolute-51": number;         // elbow_flex
  "STS3215_03a-v1-3_Revolute-53": number;         // wrist_flex
  "STS3215_03a_Wrist_Roll-v1_Revolute-55": number; // wrist_roll
  "STS3215_03a-v1-4_Revolute-57": number;         // gripper
}

/**
 * Friendly joint name aliases for easier programming
 */
export const JOINT_NAME_ALIASES = {
  wheel_1: "ST3215_Servo_Motor-v1-2_Revolute-60",
  wheel_2: "ST3215_Servo_Motor-v1-1_Revolute-62",
  wheel_3: "ST3215_Servo_Motor-v1_Revolute-64",
  shoulder_pan: "STS3215_03a-v1_Revolute-45",
  shoulder_lift: "STS3215_03a-v1-1_Revolute-49",
  elbow_flex: "STS3215_03a-v1-2_Revolute-51",
  wrist_flex: "STS3215_03a-v1-3_Revolute-53",
  wrist_roll: "STS3215_03a_Wrist_Roll-v1_Revolute-55",
  gripper: "STS3215_03a-v1-4_Revolute-57",
} as const;

export type JointAlias = keyof typeof JOINT_NAME_ALIASES;

// ============================================================================
// Joint Limits
// ============================================================================

/**
 * Joint limits for each controllable joint
 *
 * Wheels have no limits (continuous rotation)
 * Arm joints have physical limits based on servo specs and mechanical constraints
 */
export interface JointLimit {
  min: number;
  max: number;
  velocity?: number;
}

export const LEKIWI_JOINT_LIMITS: Record<keyof LeKiwiJointPositions, JointLimit> = {
  // Wheels: continuous rotation (no limits)
  "ST3215_Servo_Motor-v1-2_Revolute-60": { min: -Infinity, max: Infinity },
  "ST3215_Servo_Motor-v1-1_Revolute-62": { min: -Infinity, max: Infinity },
  "ST3215_Servo_Motor-v1_Revolute-64": { min: -Infinity, max: Infinity },

  // Arm joints: limits in radians
  // Based on STS3215 servo specs and mechanical constraints
  "STS3215_03a-v1_Revolute-45": { min: -3.14, max: 3.14, velocity: 2.0 },          // shoulder_pan: ±180°
  "STS3215_03a-v1-1_Revolute-49": { min: -1.57, max: 1.57, velocity: 1.5 },        // shoulder_lift: ±90°
  "STS3215_03a-v1-2_Revolute-51": { min: -2.09, max: 2.09, velocity: 1.5 },        // elbow_flex: ±120°
  "STS3215_03a-v1-3_Revolute-53": { min: -1.57, max: 1.57, velocity: 1.5 },        // wrist_flex: ±90°
  "STS3215_03a_Wrist_Roll-v1_Revolute-55": { min: -3.14, max: 3.14, velocity: 2.0 }, // wrist_roll: ±180°
  "STS3215_03a-v1-4_Revolute-57": { min: -0.79, max: 0.79, velocity: 1.0 },        // gripper: ±45°
};

// ============================================================================
// Pose Presets
// ============================================================================

/**
 * Predefined pose preset definition
 * Pattern adapted from hexapod's DEFAULT_POSE
 */
export interface PosePreset {
  name: string;
  description: string;
  positions: LeKiwiJointPositions;
}

/**
 * Predefined pose presets for common configurations
 * Adapted from hexapod repository's DEFAULT_POSE pattern
 */
export const LEKIWI_POSE_PRESETS: Record<string, PosePreset> = {
  home: {
    name: "Home Position",
    description: "All joints at zero (neutral position)",
    positions: {
      "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
      "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
      "ST3215_Servo_Motor-v1_Revolute-64": 0,
      "STS3215_03a-v1_Revolute-45": 0,
      "STS3215_03a-v1-1_Revolute-49": 0,
      "STS3215_03a-v1-2_Revolute-51": 0,
      "STS3215_03a-v1-3_Revolute-53": 0,
      "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0,
      "STS3215_03a-v1-4_Revolute-57": 0,
    },
  },

  ready: {
    name: "Ready Position",
    description: "Arm raised and ready to manipulate",
    positions: {
      "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
      "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
      "ST3215_Servo_Motor-v1_Revolute-64": 0,
      "STS3215_03a-v1_Revolute-45": 0,           // shoulder_pan: forward
      "STS3215_03a-v1-1_Revolute-49": -0.785,    // shoulder_lift: raised 45°
      "STS3215_03a-v1-2_Revolute-51": 1.57,      // elbow_flex: bent 90°
      "STS3215_03a-v1-3_Revolute-53": -0.785,    // wrist_flex: down 45°
      "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0, // wrist_roll: neutral
      "STS3215_03a-v1-4_Revolute-57": 0,         // gripper: open
    },
  },

  stow: {
    name: "Stowed Position",
    description: "Compact configuration for transport",
    positions: {
      "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
      "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
      "ST3215_Servo_Motor-v1_Revolute-64": 0,
      "STS3215_03a-v1_Revolute-45": 0,
      "STS3215_03a-v1-1_Revolute-49": 1.2,       // shoulder_lift: raised ~69°
      "STS3215_03a-v1-2_Revolute-51": 2.0,       // elbow_flex: fully bent
      "STS3215_03a-v1-3_Revolute-53": 1.4,       // wrist_flex: folded
      "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0,
      "STS3215_03a-v1-4_Revolute-57": 0,         // gripper: closed
    },
  },

  reach: {
    name: "Reach Forward",
    description: "Extended forward position for reaching",
    positions: {
      "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
      "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
      "ST3215_Servo_Motor-v1_Revolute-64": 0,
      "STS3215_03a-v1_Revolute-45": 0,           // shoulder_pan: forward
      "STS3215_03a-v1-1_Revolute-49": -0.3,      // shoulder_lift: slightly down
      "STS3215_03a-v1-2_Revolute-51": 0.5,       // elbow_flex: slightly bent
      "STS3215_03a-v1-3_Revolute-53": -0.2,      // wrist_flex: slightly down
      "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0,
      "STS3215_03a-v1-4_Revolute-57": 0,
    },
  },

  grab: {
    name: "Grab Position",
    description: "Low position for picking up objects",
    positions: {
      "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
      "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
      "ST3215_Servo_Motor-v1_Revolute-64": 0,
      "STS3215_03a-v1_Revolute-45": 0,
      "STS3215_03a-v1-1_Revolute-49": 0.5,       // shoulder_lift: forward
      "STS3215_03a-v1-2_Revolute-51": 1.2,       // elbow_flex: bent
      "STS3215_03a-v1-3_Revolute-53": 0.8,       // wrist_flex: down
      "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0,
      "STS3215_03a-v1-4_Revolute-57": 0.5,       // gripper: partially open
    },
  },
};

// ============================================================================
// Animation & Trajectory Types
// ============================================================================

/**
 * Single keyframe in a trajectory
 * Pattern adapted from hexapod's walkSequenceSolver
 */
export interface JointKeyframe {
  timestamp: number;          // milliseconds from start
  positions: LeKiwiJointPositions;
  duration?: number;          // transition time to this keyframe (ms)
  interpolation?: "linear" | "cubic"; // interpolation method (default: linear)
}

/**
 * Complete trajectory sequence with metadata
 */
export interface TrajectorySequence {
  name: string;
  description?: string;
  keyframes: JointKeyframe[];
  loop?: boolean;
  totalDuration: number;      // milliseconds
}

/**
 * Recording state for trajectory capture
 */
export interface TrajectoryRecording {
  isRecording: boolean;
  startTime: number | null;
  keyframes: JointKeyframe[];
}

// ============================================================================
// URDF Loader State
// ============================================================================

/**
 * URDF loading state tracking
 */
export interface URDFLoadState {
  status: "idle" | "loading" | "loaded" | "error";
  progress: number;           // 0-100
  error: string | null;
  robot: any | null;          // THREE.Object3D from urdf-loader
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Get user-friendly joint display name
 */
export function getJointDisplayName(jointName: keyof LeKiwiJointPositions): string {
  const aliasEntry = Object.entries(JOINT_NAME_ALIASES).find(
    ([_, fullName]) => fullName === jointName
  );

  if (aliasEntry) {
    const alias = aliasEntry[0];
    // Convert snake_case to Title Case
    return alias
      .split("_")
      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  }

  return jointName;
}

/**
 * Get joint group (wheel or arm)
 */
export function getJointGroup(jointName: keyof LeKiwiJointPositions): "wheel" | "arm" {
  return jointName.startsWith("ST3215") ? "wheel" : "arm";
}

/**
 * Convert joint alias to full URDF joint name
 */
export function aliasToJointName(alias: JointAlias): keyof LeKiwiJointPositions {
  return JOINT_NAME_ALIASES[alias];
}

/**
 * Create empty joint positions (all zeros)
 */
export function createEmptyJointPositions(): LeKiwiJointPositions {
  return {
    "ST3215_Servo_Motor-v1-2_Revolute-60": 0,
    "ST3215_Servo_Motor-v1-1_Revolute-62": 0,
    "ST3215_Servo_Motor-v1_Revolute-64": 0,
    "STS3215_03a-v1_Revolute-45": 0,
    "STS3215_03a-v1-1_Revolute-49": 0,
    "STS3215_03a-v1-2_Revolute-51": 0,
    "STS3215_03a-v1-3_Revolute-53": 0,
    "STS3215_03a_Wrist_Roll-v1_Revolute-55": 0,
    "STS3215_03a-v1-4_Revolute-57": 0,
  };
}

/**
 * Clamp joint value to its limits
 */
export function clampJointValue(
  jointName: keyof LeKiwiJointPositions,
  value: number
): number {
  const limits = LEKIWI_JOINT_LIMITS[jointName];
  return Math.max(limits.min, Math.min(limits.max, value));
}

/**
 * Validate all joint positions are within limits
 */
export function validateJointPositions(positions: Partial<LeKiwiJointPositions>): {
  valid: boolean;
  errors: string[];
} {
  const errors: string[] = [];

  Object.entries(positions).forEach(([joint, value]) => {
    const jointName = joint as keyof LeKiwiJointPositions;
    const limits = LEKIWI_JOINT_LIMITS[jointName];

    if (value < limits.min || value > limits.max) {
      errors.push(
        `${getJointDisplayName(jointName)}: ${value.toFixed(3)} is outside limits [${limits.min.toFixed(3)}, ${limits.max.toFixed(3)}]`
      );
    }
  });

  return {
    valid: errors.length === 0,
    errors,
  };
}
