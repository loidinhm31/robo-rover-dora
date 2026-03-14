/**
 * URDF Visualization Page
 *
 * Main page component for 3D robot visualization with controls.
 * Adapted from hexapod's InverseKinematicsPage.js pattern.
 *
 * Features:
 * - 3D URDF robot visualization
 * - Manual joint control sliders
 * - Pose presets for quick configuration
 * - Animation recording and playback
 * - Socket.IO telemetry integration
 * - Responsive layout (desktop/mobile)
 */

import React, { useState, useEffect, useCallback } from "react";
import type { Socket } from "socket.io-client";
import { URDFViewer } from "../organisms/URDFViewer";
import { PosePresetSelector } from "../molecules/PosePresetSelector";
import { AnimationControls } from "../molecules/AnimationControls";
import { SliderControl } from "../molecules/SliderControl";
import { CollapsibleSection } from "../molecules";
import {
  LeKiwiJointPositions,
  LEKIWI_POSE_PRESETS,
  LEKIWI_JOINT_LIMITS,
  TrajectorySequence,
  TrajectoryRecording,
  getJointDisplayName,
  getJointGroup,
} from "../../types/urdf";
import { TrajectoryAnimator } from "../../lib/trajectoryAnimator";
import type { ServoTelemetry, URDFCommand } from "../../types/robo";

// ============================================================================
// Component Interface
// ============================================================================

export interface URDFVisualizationPageProps {
  socket: Socket | null;
  isConnected: boolean;
}

// ============================================================================
// Component
// ============================================================================

export const URDFVisualizationPage: React.FC<URDFVisualizationPageProps> = ({
  socket,
  isConnected,
}) => {
  // State management
  const [jointPositions, setJointPositions] = useState<LeKiwiJointPositions>(
    LEKIWI_POSE_PRESETS.home.positions
  );
  const [currentPreset, setCurrentPreset] = useState<string | null>("home");
  const [isPlaying, setIsPlaying] = useState(false);
  const [recording, setRecording] = useState<TrajectoryRecording>({
    isRecording: false,
    startTime: null,
    keyframes: [],
  });
  const [trajectory, setTrajectory] = useState<TrajectorySequence | null>(null);
  const [animator] = useState(() => new TrajectoryAnimator(30));
  const [currentTime, setCurrentTime] = useState(0);

  // Socket.IO telemetry listener
  useEffect(() => {
    if (!socket || !isConnected) return;

    const handleServoTelemetry = (telemetry: ServoTelemetry) => {
      // Only update from telemetry if not playing or recording
      if (!isPlaying && !recording.isRecording) {
        setJointPositions(telemetry.joint_positions);
      }
    };

    socket.on("servo_telemetry", handleServoTelemetry);

    return () => {
      socket.off("servo_telemetry", handleServoTelemetry);
    };
  }, [socket, isConnected, isPlaying, recording.isRecording]);

  // Manual joint control
  const handleJointChange = useCallback(
    (joint: keyof LeKiwiJointPositions, value: number) => {
      const newPositions = { ...jointPositions, [joint]: value };
      setJointPositions(newPositions);
      setCurrentPreset(null); // Clear preset when manually adjusting

      // Send to backend via Socket.IO
      if (socket && isConnected) {
        const command: URDFCommand = {
          command_type: "set_pose",
          positions: newPositions,
        };
        socket.emit("urdf_command", command);
      }
    },
    [jointPositions, socket, isConnected]
  );

  // Preset selection
  const handleSelectPreset = useCallback(
    (presetName: string) => {
      const preset = LEKIWI_POSE_PRESETS[presetName];
      setJointPositions(preset.positions);
      setCurrentPreset(presetName);

      if (socket && isConnected) {
        const command: URDFCommand = {
          command_type: "set_pose",
          positions: preset.positions,
        };
        socket.emit("urdf_command", command);
      }
    },
    [socket, isConnected]
  );

  // Animation playback
  const handlePlay = useCallback(() => {
    if (!trajectory) return;
    animator.loadSequence(trajectory);
    animator.play();
    setIsPlaying(true);
  }, [trajectory, animator]);

  const handlePause = useCallback(() => {
    animator.pause();
    setIsPlaying(false);
  }, [animator]);

  const handleReset = useCallback(() => {
    animator.reset();
    setIsPlaying(false);
    setCurrentTime(0);
    if (trajectory) {
      setJointPositions(trajectory.keyframes[0].positions);
    }
  }, [animator, trajectory]);

  // Recording
  const handleToggleRecording = useCallback(() => {
    if (recording.isRecording) {
      // Stop recording and create trajectory
      const newTrajectory: TrajectorySequence = {
        name: `Recorded ${new Date().toLocaleTimeString()}`,
        description: "User-recorded trajectory",
        keyframes: recording.keyframes,
        loop: false,
        totalDuration:
          recording.keyframes[recording.keyframes.length - 1]?.timestamp || 0,
      };
      setTrajectory(newTrajectory);
      setRecording({ isRecording: false, startTime: null, keyframes: [] });
      console.log("Recording stopped:", newTrajectory);
    } else {
      // Start recording
      setRecording({
        isRecording: true,
        startTime: Date.now(),
        keyframes: [
          {
            timestamp: 0,
            positions: jointPositions,
          },
        ],
      });
      console.log("Recording started");
    }
  }, [recording, jointPositions]);

  // Animation frame update
  useEffect(() => {
    if (!isPlaying) return;

    let animationFrame: number;
    const updateAnimation = () => {
      const now = Date.now();
      const pose = animator.getCurrentPose(now);
      const currentPlaybackTime = animator.getCurrentTime();

      setCurrentTime(currentPlaybackTime);

      if (pose) {
        setJointPositions(pose);

        // Optionally send to backend during playback
        // if (socket && isConnected) {
        //   socket.emit("urdf_command", { command_type: "set_pose", positions: pose });
        // }
      }

      animationFrame = requestAnimationFrame(updateAnimation);
    };

    animationFrame = requestAnimationFrame(updateAnimation);
    return () => cancelAnimationFrame(animationFrame);
  }, [isPlaying, animator, socket, isConnected]);

  // Recording frame capture
  useEffect(() => {
    if (!recording.isRecording || !recording.startTime) return;

    const captureInterval = setInterval(() => {
      const timestamp = Date.now() - recording.startTime!;
      setRecording((prev) => ({
        ...prev,
        keyframes: [
          ...prev.keyframes,
          { timestamp, positions: jointPositions },
        ],
      }));
    }, 100); // Capture every 100ms

    return () => clearInterval(captureInterval);
  }, [recording.isRecording, recording.startTime, jointPositions]);

  // Group joints by subsystem
  const armJoints = Object.keys(jointPositions).filter(
    (joint) => getJointGroup(joint as keyof LeKiwiJointPositions) === "arm"
  ) as Array<keyof LeKiwiJointPositions>;

  const wheelJoints = Object.keys(jointPositions).filter(
    (joint) => getJointGroup(joint as keyof LeKiwiJointPositions) === "wheel"
  ) as Array<keyof LeKiwiJointPositions>;

  return (
    <div className="flex flex-col lg:flex-row gap-4 p-4 h-screen overflow-hidden">
      {/* Left Panel: 3D Viewer */}
      <div className="flex-1 glass-card rounded-3xl p-4 min-h-[400px] md:min-h-[500px] lg:min-h-0">
        <URDFViewer
          urdfPath="/model/LeKiwi.urdf"
          jointPositions={jointPositions}
          showGrid={true}
          className="w-full h-full"
        />
      </div>

      {/* Right Panel: Controls */}
      <div className="w-full lg:w-96 space-y-4 overflow-y-auto max-h-[calc(100vh-2rem)] lg:max-h-none">
        {/* Connection Status */}
        <div
          className={`px-4 py-2 rounded-xl text-sm font-medium text-center ${
            isConnected
              ? "bg-green-500/20 text-green-300 border border-green-500/30"
              : "bg-red-500/20 text-red-300 border border-red-500/30"
          }`}
        >
          {isConnected ? "● Connected" : "○ Disconnected"}
        </div>

        {/* Pose Presets */}
        <PosePresetSelector
          presets={LEKIWI_POSE_PRESETS}
          currentPreset={currentPreset}
          onSelectPreset={handleSelectPreset}
          disabled={isPlaying || recording.isRecording}
        />

        {/* Animation Controls */}
        <AnimationControls
          isPlaying={isPlaying}
          isRecording={recording.isRecording}
          currentTime={currentTime}
          duration={trajectory?.totalDuration || 0}
          onPlay={handlePlay}
          onPause={handlePause}
          onReset={handleReset}
          onToggleRecording={handleToggleRecording}
          disabled={!isConnected}
        />

        {/* Arm Joint Controls */}
        <CollapsibleSection title="Arm Joints (6 DOF)" defaultOpen={true}>
          <div className="grid grid-cols-1 gap-3">
            {armJoints.map((joint) => (
              <SliderControl
                key={joint}
                label={getJointDisplayName(joint)}
                value={jointPositions[joint]}
                min={LEKIWI_JOINT_LIMITS[joint].min}
                max={LEKIWI_JOINT_LIMITS[joint].max}
                unit="rad"
                onChange={(value) => handleJointChange(joint, value)}
                disabled={isPlaying || recording.isRecording || !isConnected}
                decimals={2}
              />
            ))}
          </div>
        </CollapsibleSection>

        {/* Wheel Joint Controls (Optional) */}
        <CollapsibleSection title="Wheel Joints" defaultOpen={false}>
          <div className="grid grid-cols-1 gap-3">
            {wheelJoints.map((joint) => (
              <SliderControl
                key={joint}
                label={getJointDisplayName(joint)}
                value={jointPositions[joint]}
                min={-6.28}
                max={6.28}
                unit="rad"
                onChange={(value) => handleJointChange(joint, value)}
                disabled={isPlaying || recording.isRecording || !isConnected}
                decimals={2}
              />
            ))}
          </div>
        </CollapsibleSection>
      </div>
    </div>
  );
};

// ============================================================================
// Export
// ============================================================================

export default URDFVisualizationPage;
