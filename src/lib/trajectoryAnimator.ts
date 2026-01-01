/**
 * Trajectory Animator for Robot Motion Playback
 *
 * Handles animation playback with keyframe interpolation for smooth motion.
 * Adapted from hexapod repository's walkSequenceSolver.js to work with
 * generic trajectories instead of gait-specific sequences.
 *
 * Key Features:
 * - Frame-rate independent playback
 * - Linear interpolation between keyframes
 * - Loop support
 * - Play/pause/reset controls
 * - Static factory methods for creating trajectories
 */

import type {
  JointKeyframe,
  LeKiwiJointPositions,
  TrajectorySequence,
} from "../types/urdf";

// ============================================================================
// Trajectory Animator Class
// ============================================================================

export class TrajectoryAnimator {
  private sequence: TrajectorySequence | null = null;
  private startTime: number = 0;
  private isPlaying: boolean = false;
  private currentFrame: number = 0;
  private targetFPS: number = 30; // Target FPS for complex scenes
  private lastFrameTime: number = 0;

  constructor(targetFPS: number = 30) {
    this.targetFPS = targetFPS;
  }

  // ==========================================================================
  // Playback Control
  // ==========================================================================

  /**
   * Load a trajectory sequence for playback
   */
  loadSequence(sequence: TrajectorySequence): void {
    this.sequence = sequence;
    this.currentFrame = 0;
    console.log(`Loaded trajectory "${sequence.name}" with ${sequence.keyframes.length} keyframes`);
  }

  /**
   * Start playing the loaded sequence
   */
  play(): void {
    if (!this.sequence) {
      console.warn("No sequence loaded");
      return;
    }

    this.startTime = Date.now();
    this.lastFrameTime = this.startTime;
    this.isPlaying = true;
    console.log("Playback started");
  }

  /**
   * Pause playback
   */
  pause(): void {
    this.isPlaying = false;
    console.log("Playback paused");
  }

  /**
   * Reset to beginning
   */
  reset(): void {
    this.currentFrame = 0;
    this.isPlaying = false;
    this.startTime = 0;
    this.lastFrameTime = 0;
    console.log("Playback reset");
  }

  /**
   * Check if currently playing
   */
  getIsPlaying(): boolean {
    return this.isPlaying;
  }

  /**
   * Get current playback time (ms from start)
   */
  getCurrentTime(): number {
    if (!this.isPlaying) return 0;
    return Date.now() - this.startTime;
  }

  // ==========================================================================
  // Pose Computation
  // ==========================================================================

  /**
   * Get interpolated pose at current time
   * Adapted from hexapod's getPose() method
   *
   * @param currentTime - Current timestamp in milliseconds
   * @returns Interpolated joint positions, or null if playback stopped
   */
  getCurrentPose(currentTime: number): LeKiwiJointPositions | null {
    if (!this.sequence || !this.isPlaying) {
      return null;
    }

    // Frame rate limiting for performance
    const frameInterval = 1000 / this.targetFPS;
    if (currentTime - this.lastFrameTime < frameInterval) {
      return null; // Skip this frame
    }
    this.lastFrameTime = currentTime;

    const elapsed = currentTime - this.startTime;
    const { keyframes, totalDuration, loop } = this.sequence;

    // Calculate playback time (with loop support)
    let t = elapsed;
    if (loop) {
      t = elapsed % totalDuration;
    } else if (elapsed > totalDuration) {
      // Sequence finished
      this.pause();
      return keyframes[keyframes.length - 1].positions;
    }

    // Find surrounding keyframes
    const [prevFrame, nextFrame] = this.findKeyframes(t, keyframes);

    if (!nextFrame) {
      // At or past last keyframe
      return prevFrame.positions;
    }

    // Linear interpolation between keyframes
    const segmentDuration = nextFrame.timestamp - prevFrame.timestamp;
    const segmentProgress = (t - prevFrame.timestamp) / segmentDuration;

    return this.interpolatePositions(
      prevFrame.positions,
      nextFrame.positions,
      segmentProgress
    );
  }

  /**
   * Find keyframes surrounding the given time
   * Adapted from hexapod's buildSequence() logic
   */
  private findKeyframes(
    time: number,
    keyframes: JointKeyframe[]
  ): [JointKeyframe, JointKeyframe | null] {
    for (let i = 0; i < keyframes.length - 1; i++) {
      if (time >= keyframes[i].timestamp && time <= keyframes[i + 1].timestamp) {
        return [keyframes[i], keyframes[i + 1]];
      }
    }

    // Past last keyframe
    return [keyframes[keyframes.length - 1], null];
  }

  /**
   * Linear interpolation between two joint position sets
   * Adapted from hexapod's interpolation logic
   *
   * @param start - Starting joint positions
   * @param end - Ending joint positions
   * @param t - Interpolation factor (0-1)
   * @returns Interpolated positions
   */
  private interpolatePositions(
    start: LeKiwiJointPositions,
    end: LeKiwiJointPositions,
    t: number
  ): LeKiwiJointPositions {
    const result: any = {};

    // Clamp t to [0, 1]
    t = Math.max(0, Math.min(1, t));

    // Interpolate each joint
    Object.keys(start).forEach((joint) => {
      const jointKey = joint as keyof LeKiwiJointPositions;
      const startVal = start[jointKey];
      const endVal = end[jointKey];

      // Linear interpolation: lerp(a, b, t) = a + (b - a) * t
      result[joint] = startVal + (endVal - startVal) * t;
    });

    return result as LeKiwiJointPositions;
  }

  // ==========================================================================
  // Static Factory Methods
  // ==========================================================================

  /**
   * Create a simple trajectory from start to end pose
   * Adapted from hexapod's buildSequence() pattern
   *
   * @param name - Trajectory name
   * @param startPose - Starting joint positions
   * @param endPose - Ending joint positions
   * @param duration - Total duration in milliseconds
   * @param steps - Number of intermediate keyframes (default: 10)
   * @returns Complete trajectory sequence
   */
  static createTrajectory(
    name: string,
    startPose: LeKiwiJointPositions,
    endPose: LeKiwiJointPositions,
    duration: number,
    steps: number = 10
  ): TrajectorySequence {
    const keyframes: JointKeyframe[] = [];
    const stepDuration = duration / steps;

    for (let i = 0; i <= steps; i++) {
      const t = i / steps; // 0 to 1
      const positions: any = {};

      // Interpolate each joint
      Object.keys(startPose).forEach((joint) => {
        const jointKey = joint as keyof LeKiwiJointPositions;
        const start = startPose[jointKey];
        const end = endPose[jointKey];
        positions[joint] = start + (end - start) * t;
      });

      keyframes.push({
        timestamp: i * stepDuration,
        positions: positions as LeKiwiJointPositions,
        duration: stepDuration,
        interpolation: "linear",
      });
    }

    return {
      name,
      description: `${steps}-step trajectory from start to end`,
      keyframes,
      loop: false,
      totalDuration: duration,
    };
  }

  /**
   * Create a looping trajectory (e.g., for repetitive motions)
   *
   * @param name - Trajectory name
   * @param poses - Array of poses to cycle through
   * @param cycleDuration - Duration for one complete cycle (ms)
   * @returns Looping trajectory sequence
   */
  static createLoopingTrajectory(
    name: string,
    poses: LeKiwiJointPositions[],
    cycleDuration: number
  ): TrajectorySequence {
    if (poses.length < 2) {
      throw new Error("Looping trajectory requires at least 2 poses");
    }

    const keyframes: JointKeyframe[] = [];
    const stepDuration = cycleDuration / poses.length;

    poses.forEach((pose, index) => {
      keyframes.push({
        timestamp: index * stepDuration,
        positions: pose,
        duration: stepDuration,
        interpolation: "linear",
      });
    });

    // Add first pose again at the end for smooth loop
    keyframes.push({
      timestamp: cycleDuration,
      positions: poses[0],
      duration: stepDuration,
      interpolation: "linear",
    });

    return {
      name,
      description: `Looping trajectory with ${poses.length} poses`,
      keyframes,
      loop: true,
      totalDuration: cycleDuration,
    };
  }

  /**
   * Create trajectory from pose preset transitions
   * Useful for chaining multiple preset poses
   *
   * @param name - Trajectory name
   * @param poseSequence - Array of poses with durations
   * @returns Complete trajectory sequence
   */
  static createSequenceFromPoses(
    name: string,
    poseSequence: Array<{ pose: LeKiwiJointPositions; duration: number }>
  ): TrajectorySequence {
    const keyframes: JointKeyframe[] = [];
    let currentTime = 0;

    poseSequence.forEach(({ pose, duration }) => {
      keyframes.push({
        timestamp: currentTime,
        positions: pose,
        duration,
        interpolation: "linear",
      });
      currentTime += duration;
    });

    return {
      name,
      description: `Sequence of ${poseSequence.length} poses`,
      keyframes,
      loop: false,
      totalDuration: currentTime,
    };
  }

  // ==========================================================================
  // Trajectory Analysis
  // ==========================================================================

  /**
   * Get trajectory statistics
   */
  getSequenceStats(): {
    totalKeyframes: number;
    totalDuration: number;
    avgFrameDuration: number;
    isLooping: boolean;
  } | null {
    if (!this.sequence) return null;

    const avgDuration =
      this.sequence.totalDuration / (this.sequence.keyframes.length - 1);

    return {
      totalKeyframes: this.sequence.keyframes.length,
      totalDuration: this.sequence.totalDuration,
      avgFrameDuration: avgDuration,
      isLooping: this.sequence.loop || false,
    };
  }

  /**
   * Export sequence to JSON
   */
  exportSequence(): string | null {
    if (!this.sequence) return null;
    return JSON.stringify(this.sequence, null, 2);
  }

  /**
   * Import sequence from JSON
   */
  importSequence(json: string): boolean {
    try {
      const sequence = JSON.parse(json) as TrajectorySequence;
      this.loadSequence(sequence);
      return true;
    } catch (error) {
      console.error("Failed to import sequence:", error);
      return false;
    }
  }

  // ==========================================================================
  // Performance Tuning
  // ==========================================================================

  /**
   * Set target FPS for playback
   * Lower FPS improves performance for complex scenes
   */
  setTargetFPS(fps: number): void {
    this.targetFPS = Math.max(1, Math.min(60, fps));
    console.log(`Target FPS set to ${this.targetFPS}`);
  }

  /**
   * Get current target FPS
   */
  getTargetFPS(): number {
    return this.targetFPS;
  }
}
