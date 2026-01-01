/**
 * Pose Preset Selector Component
 *
 * Grid of buttons for selecting predefined robot poses.
 * Adapted from hexapod repository's pose selection pattern.
 *
 * Features:
 * - 2-column responsive grid layout
 * - Active state highlighting
 * - Glassmorphic design matching app theme
 * - Disabled state support
 */

import React from "react";
import type { PosePreset } from "../../types/urdf";

// ============================================================================
// Component Interface
// ============================================================================

export interface PosePresetSelectorProps {
  presets: Record<string, PosePreset>;
  currentPreset: string | null;
  onSelectPreset: (presetName: string) => void;
  disabled?: boolean;
  className?: string;
}

// ============================================================================
// Component
// ============================================================================

export const PosePresetSelector: React.FC<PosePresetSelectorProps> = ({
  presets,
  currentPreset,
  onSelectPreset,
  disabled = false,
  className = "",
}) => {
  return (
    <div className={`glass-card-light rounded-2xl p-4 space-y-3 ${className}`}>
      {/* Header */}
      <h3 className="text-cyan-300 font-semibold text-sm uppercase tracking-wide">
        Pose Presets
      </h3>

      {/* Preset Grid */}
      <div className="grid grid-cols-2 gap-2">
        {Object.entries(presets).map(([key, preset]) => (
          <PresetButton
            key={key}
            presetKey={key}
            preset={preset}
            isActive={currentPreset === key}
            onSelect={onSelectPreset}
            disabled={disabled}
          />
        ))}
      </div>
    </div>
  );
};

// ============================================================================
// Preset Button Component
// ============================================================================

interface PresetButtonProps {
  presetKey: string;
  preset: PosePreset;
  isActive: boolean;
  onSelect: (key: string) => void;
  disabled: boolean;
}

const PresetButton: React.FC<PresetButtonProps> = ({
  presetKey,
  preset,
  isActive,
  onSelect,
  disabled,
}) => {
  return (
    <button
      onClick={() => onSelect(presetKey)}
      disabled={disabled}
      className={`
        px-3 py-2 rounded-xl font-medium text-sm transition-all
        ${
          isActive
            ? "bg-cyan-500/30 text-cyan-100 border-2 border-cyan-400 shadow-lg shadow-cyan-500/20"
            : "bg-white/10 text-white/70 border border-white/20 hover:bg-white/20 hover:text-white"
        }
        disabled:opacity-50 disabled:cursor-not-allowed
        active:scale-95
      `}
    >
      <div className="text-left">
        <div className="font-semibold">{preset.name}</div>
        <div className="text-xs opacity-70 mt-0.5">{preset.description}</div>
      </div>
    </button>
  );
};

// ============================================================================
// Export
// ============================================================================

export default PosePresetSelector;
