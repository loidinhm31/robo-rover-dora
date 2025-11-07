import React from "react";
import { Radio, Server, Wifi, WifiOff } from "lucide-react";
import { FleetStatus, SystemMetrics } from "../../types";
import { BatteryIndicator } from "../atoms";

interface FleetSelectorProps {
  fleetStatus: FleetStatus | null;
  metricsMap?: Map<string, SystemMetrics>;
  onSelectRover: (entityId: string) => void;
  className?: string;
}

export const FleetSelector: React.FC<FleetSelectorProps> = ({
  fleetStatus,
  metricsMap,
  onSelectRover,
  className = "",
}) => {
  if (!fleetStatus) {
    return (
      <div className={`p-4 bg-gray-800/50 rounded-lg ${className}`}>
        <div className="flex items-center gap-2 text-gray-400">
          <Server size={16} />
          <span className="text-sm">Loading fleet status...</span>
        </div>
      </div>
    );
  }

  const handleSelect = (entityId: string) => {
    if (entityId !== fleetStatus.selected_entity) {
      onSelectRover(entityId);
    }
  };

  // Get metrics for a rover
  const getRoverMetrics = (entityId: string): SystemMetrics | undefined => {
    return metricsMap?.get(entityId);
  };

  // Check if rover has recent metrics (within last 10 seconds)
  const isRoverOnline = (entityId: string): boolean => {
    const metrics = getRoverMetrics(entityId);
    if (!metrics) return false;
    const now = Date.now();
    const metricsAge = now - metrics.timestamp;
    return metricsAge < 10000; // 10 seconds
  };

  return (
    <div className={`p-4 bg-gray-800/50 rounded-lg ${className}`}>
      <div className="flex items-center gap-2 mb-3">
        <Server size={18} className="text-blue-400" />
        <h3 className="text-sm font-semibold text-gray-200">Fleet Control</h3>
        <span className="ml-auto text-xs text-gray-500">
          {fleetStatus.fleet_roster.length} {fleetStatus.fleet_roster.length === 1 ? "rover" : "rovers"}
        </span>
      </div>

      <div className="space-y-2">
        {fleetStatus.fleet_roster.map((entityId) => {
          const isSelected = entityId === fleetStatus.selected_entity;
          const metrics = getRoverMetrics(entityId);
          const isOnline = isRoverOnline(entityId);

          return (
            <button
              key={entityId}
              onClick={() => handleSelect(entityId)}
              className={`w-full p-3 rounded-lg transition-all ${
                isSelected
                  ? "bg-blue-600 hover:bg-blue-700 text-white"
                  : "bg-gray-700/50 hover:bg-gray-700 text-gray-300"
              }`}
            >
              {/* Top row: Name and status */}
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <Radio
                    size={16}
                    className={isSelected ? "text-white" : "text-gray-400"}
                  />
                  <span className="text-sm font-medium">{entityId}</span>
                </div>

                <div className="flex items-center gap-2">
                  {/* Online status */}
                  {isOnline ? (
                    <Wifi size={14} className="text-emerald-400" />
                  ) : (
                    <WifiOff size={14} className="text-gray-500" />
                  )}

                  {/* Active badge */}
                  {isSelected && (
                    <span className="text-xs bg-white/20 px-2 py-0.5 rounded">
                      Active
                    </span>
                  )}
                </div>
              </div>

              {/* Bottom row: Metrics summary */}
              {metrics && isOnline && (
                <div className="flex items-center justify-between gap-3 text-xs">
                  {/* Battery */}
                  {metrics.battery_level !== undefined && (
                    <BatteryIndicator
                      level={metrics.battery_level}
                      voltage={metrics.battery_voltage}
                      size="sm"
                      showPercentage
                    />
                  )}

                  {/* CPU */}
                  <div className="flex items-center gap-1">
                    <span className={isSelected ? "text-white/70" : "text-gray-400"}>
                      CPU:
                    </span>
                    <span className={`font-mono font-semibold ${
                      metrics.total_cpu_percent > 80
                        ? "text-rose-400"
                        : metrics.total_cpu_percent > 50
                        ? "text-amber-400"
                        : "text-emerald-400"
                    }`}>
                      {metrics.total_cpu_percent.toFixed(0)}%
                    </span>
                  </div>

                  {/* Memory */}
                  <div className="flex items-center gap-1">
                    <span className={isSelected ? "text-white/70" : "text-gray-400"}>
                      MEM:
                    </span>
                    <span className="font-mono font-semibold text-cyan-400">
                      {((metrics.total_memory_mb / metrics.total_system_memory_mb) * 100).toFixed(0)}%
                    </span>
                  </div>

                  {/* FPS */}
                  <div className="flex items-center gap-1">
                    <span className={isSelected ? "text-white/70" : "text-gray-400"}>
                      FPS:
                    </span>
                    <span className={`font-mono font-semibold ${
                      metrics.dataflow_fps > 20
                        ? "text-emerald-400"
                        : metrics.dataflow_fps > 10
                        ? "text-amber-400"
                        : "text-rose-400"
                    }`}>
                      {metrics.dataflow_fps.toFixed(0)}
                    </span>
                  </div>
                </div>
              )}

              {/* Offline message */}
              {!isOnline && (
                <div className="text-xs text-gray-500 mt-1">
                  No recent telemetry
                </div>
              )}
            </button>
          );
        })}
      </div>

      {fleetStatus.fleet_roster.length === 0 && (
        <div className="text-center py-4">
          <p className="text-sm text-gray-400">No rovers available</p>
        </div>
      )}

      <div className="mt-3 pt-3 border-t border-gray-700">
        <p className="text-xs text-gray-500">
          Selected: <span className="text-gray-300">{fleetStatus.selected_entity}</span>
        </p>
      </div>
    </div>
  );
};
