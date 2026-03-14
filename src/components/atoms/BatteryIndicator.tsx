import React from "react";
import { Battery, BatteryCharging, BatteryLow, BatteryWarning } from "lucide-react";

interface BatteryIndicatorProps {
  level?: number;
  voltage?: number;
  size?: "sm" | "md" | "lg";
  showPercentage?: boolean;
  showVoltage?: boolean;
  className?: string;
}

export const BatteryIndicator: React.FC<BatteryIndicatorProps> = ({
  level,
  voltage,
  size = "md",
  showPercentage = true,
  showVoltage = false,
  className = "",
}) => {
  // If no battery info available
  if (level === undefined && voltage === undefined) {
    return null;
  }

  const batteryLevel = level ?? 0;

  // Determine battery status and styling - syntax colors
  const getBatteryStatus = () => {
    if (batteryLevel > 80) {
      return {
        Icon: Battery,
        color: "text-syntax-green",
        bgColor: "bg-syntax-green/10",
        fillColor: "bg-syntax-green",
      };
    } else if (batteryLevel > 50) {
      return {
        Icon: BatteryCharging,
        color: "text-syntax-blue",
        bgColor: "bg-syntax-blue/10",
        fillColor: "bg-syntax-blue",
      };
    } else if (batteryLevel > 20) {
      return {
        Icon: BatteryLow,
        color: "text-syntax-yellow",
        bgColor: "bg-syntax-yellow/10",
        fillColor: "bg-syntax-yellow",
      };
    } else {
      return {
        Icon: BatteryWarning,
        color: "text-syntax-red",
        bgColor: "bg-syntax-red/10",
        fillColor: "bg-syntax-red",
      };
    }
  };

  const status = getBatteryStatus();
  const Icon = status.Icon;

  // Size classes
  const sizeClasses = {
    sm: {
      icon: "w-3.5 h-3.5",
      text: "text-xs",
      container: "gap-1 px-1.5 py-0.5",
      bar: "h-1",
    },
    md: {
      icon: "w-4 h-4",
      text: "text-sm",
      container: "gap-1.5 px-2 py-1",
      bar: "h-1.5",
    },
    lg: {
      icon: "w-5 h-5",
      text: "text-base",
      container: "gap-2 px-2.5 py-1.5",
      bar: "h-2",
    },
  };

  const sizes = sizeClasses[size];

  return (
    <div className={`flex items-center ${sizes.container} ${status.bgColor} border border-slate-700 rounded ${className}`}>
      <Icon className={`${sizes.icon} ${status.color}`} />

      {showPercentage && level !== undefined && (
        <span className={`${sizes.text} font-mono font-semibold ${status.color}`}>
          {batteryLevel.toFixed(0)}%
        </span>
      )}

      {showVoltage && voltage !== undefined && (
        <span className={`${sizes.text} text-slate-400 font-mono`}>
          {voltage.toFixed(2)}V
        </span>
      )}

      {/* Battery level bar */}
      {level !== undefined && (
        <div className={`w-12 ${sizes.bar} bg-slate-800 border border-slate-700 rounded-full overflow-hidden`}>
          <div
            className={`${sizes.bar} ${status.fillColor} transition-all duration-300`}
            style={{ width: `${Math.max(0, Math.min(100, batteryLevel))}%` }}
          />
        </div>
      )}
    </div>
  );
};

export default BatteryIndicator;
