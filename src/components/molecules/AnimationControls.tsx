/**
 * Animation Controls Component
 *
 * Playback and recording controls for robot trajectories.
 * Adapted from hexapod repository's WalkingGaitsPage controls.
 *
 * Features:
 * - Play/Pause/Reset buttons
 * - Recording toggle
 * - Progress bar with time display
 * - Glassmorphic purple/cyan gradient styling
 */

import React from "react";
import { Play, Pause, RotateCcw, Circle } from "lucide-react";

// ============================================================================
// Component Interface
// ============================================================================

export interface AnimationControlsProps {
  isPlaying: boolean;
  isRecording: boolean;
  currentTime: number;        // milliseconds
  duration: number;           // milliseconds
  onPlay: () => void;
  onPause: () => void;
  onReset: () => void;
  onToggleRecording: () => void;
  disabled?: boolean;
  className?: string;
}

// ============================================================================
// Component
// ============================================================================

export const AnimationControls: React.FC<AnimationControlsProps> = ({
  isPlaying,
  isRecording,
  currentTime,
  duration,
  onPlay,
  onPause,
  onReset,
  onToggleRecording,
  disabled = false,
  className = "",
}) => {
  // Calculate progress percentage
  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  // Format time as seconds with 1 decimal place
  const formatTime = (ms: number) => (ms / 1000).toFixed(1);

  return (
    <div className={`glass-card-light rounded-2xl p-4 space-y-3 ${className}`}>
      {/* Header */}
      <h3 className="text-purple-300 font-semibold text-sm uppercase tracking-wide">
        Animation
      </h3>

      {/* Playback Controls */}
      <div className="flex gap-2">
        {/* Play/Pause Button */}
        <button
          onClick={isPlaying ? onPause : onPlay}
          disabled={disabled || !duration}
          className="flex-1 px-4 py-2 bg-purple-500/30 hover:bg-purple-500/50
                     text-white rounded-xl flex items-center justify-center gap-2
                     transition-all disabled:opacity-50 disabled:cursor-not-allowed
                     active:scale-95"
        >
          {isPlaying ? <Pause size={18} /> : <Play size={18} />}
          <span className="font-medium">{isPlaying ? "Pause" : "Play"}</span>
        </button>

        {/* Reset Button */}
        <button
          onClick={onReset}
          disabled={disabled}
          className="px-4 py-2 bg-white/10 hover:bg-white/20
                     text-white rounded-xl transition-all
                     disabled:opacity-50 disabled:cursor-not-allowed
                     active:scale-95"
          title="Reset to beginning"
        >
          <RotateCcw size={18} />
        </button>
      </div>

      {/* Recording Toggle */}
      <button
        onClick={onToggleRecording}
        disabled={disabled}
        className={`
          w-full px-4 py-2 rounded-xl flex items-center justify-center gap-2
          transition-all disabled:opacity-50 disabled:cursor-not-allowed
          active:scale-95
          ${
            isRecording
              ? "bg-red-500/30 border-2 border-red-400 text-red-100 hover:bg-red-500/40"
              : "bg-white/10 hover:bg-white/20 text-white/70 hover:text-white"
          }
        `}
      >
        <Circle
          size={18}
          className={isRecording ? "fill-red-400 animate-pulse" : ""}
        />
        <span className="font-medium">
          {isRecording ? "Stop Recording" : "Start Recording"}
        </span>
      </button>

      {/* Progress Bar */}
      <div className="space-y-1">
        <div className="h-2 bg-white/10 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-purple-400 to-cyan-400 transition-all duration-200"
            style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
          />
        </div>

        {/* Time Display */}
        <div className="flex justify-between text-xs text-white/50 font-mono">
          <span>{formatTime(currentTime)}s</span>
          <span>{formatTime(duration)}s</span>
        </div>
      </div>

      {/* Status Indicator */}
      {isRecording && (
        <div className="text-xs text-red-300 text-center animate-pulse">
          ● Recording trajectory...
        </div>
      )}
      {isPlaying && !isRecording && (
        <div className="text-xs text-purple-300 text-center">
          ▶ Playing animation...
        </div>
      )}
    </div>
  );
};

// ============================================================================
// Export
// ============================================================================

export default AnimationControls;
