import React, { useState } from "react";
import { Activity, X, Pause, Play, ChevronUp, Cpu, HardDrive, Clock, Zap } from "lucide-react";
import { SystemMetrics } from "../types";
import { Socket } from "socket.io-client";
import { StatPanel } from "./molecules";
import { getMetricColor } from "../hooks";
import { BatteryIndicator } from "./atoms";

interface FloatingMetricsProps {
  metrics: Map<string, SystemMetrics>;
  socket: Socket | null;
}

type TabType = "fps" | "cpu" | "memory" | "latency";

export const FloatingMetrics: React.FC<FloatingMetricsProps> = ({
  metrics,
  socket,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isMonitoringEnabled, setIsMonitoringEnabled] = useState(true);
  const [activeTab, setActiveTab] = useState<TabType>("fps");
  const [selectedRover, setSelectedRover] = useState<string | null>(null);

  const toggleMonitoring = () => {
    if (socket) {
      const newState = !isMonitoringEnabled;
      socket.emit("performance_control", { enabled: newState });
      setIsMonitoringEnabled(newState);
    }
  };

  // Get the metrics for the currently selected rover
  const currentMetrics = selectedRover && metrics.has(selectedRover)
    ? metrics.get(selectedRover)!
    : metrics.size > 0
    ? Array.from(metrics.values())[0]
    : null;

  // Auto-select first rover if none selected
  React.useEffect(() => {
    if (!selectedRover && metrics.size > 0) {
      setSelectedRover(Array.from(metrics.keys())[0]);
    }
  }, [metrics, selectedRover]);

  const getMetricValue = (nodeId: string, type: TabType): number => {
    if (!currentMetrics?.node_metrics[nodeId]) return 0;
    const node = currentMetrics.node_metrics[nodeId];
    switch (type) {
      case "fps": return node.fps;
      case "cpu": return node.cpu_usage_percent;
      case "memory": return node.memory_usage_mb;
      case "latency": return node.avg_processing_time_ms;
      default: return 0;
    }
  };

  const getMetricBarColor = (value: number, type: TabType): string => {
    switch (type) {
      case "fps":
        if (value > 20) return "bg-emerald-500";
        if (value > 10) return "bg-amber-500";
        return "bg-rose-500";
      case "cpu":
        if (value < 50) return "bg-emerald-500";
        if (value < 80) return "bg-amber-500";
        return "bg-rose-500";
      case "memory":
        if (value < 100) return "bg-emerald-500";
        if (value < 500) return "bg-amber-500";
        return "bg-rose-500";
      case "latency":
        if (value < 50) return "bg-emerald-500";
        if (value < 100) return "bg-amber-500";
        return "bg-rose-500";
      default:
        return "bg-gray-500";
    }
  };

  const getMaxValue = (type: TabType): number => {
    if (!currentMetrics) return 100;
    const values = Object.values(currentMetrics.node_metrics).map(n => getMetricValue(n.node_id, type));
    const max = Math.max(...values, 1);

    switch (type) {
      case "fps": return Math.max(30, max * 1.2);
      case "cpu": return Math.max(100, max * 1.2);
      case "memory": return Math.max(100, max * 1.2);
      case "latency": return Math.max(100, max * 1.2);
      default: return 100;
    }
  };

  const getTabIcon = (type: TabType) => {
    switch (type) {
      case "fps": return <Activity className="w-3.5 h-3.5" />;
      case "cpu": return <Cpu className="w-3.5 h-3.5" />;
      case "memory": return <HardDrive className="w-3.5 h-3.5" />;
      case "latency": return <Clock className="w-3.5 h-3.5" />;
    }
  };

  const getTabLabel = (type: TabType) => {
    switch (type) {
      case "fps": return "FPS";
      case "cpu": return "CPU";
      case "memory": return "MEM";
      case "latency": return "LAT";
    }
  };

  const formatValue = (value: number, type: TabType): string => {
    switch (type) {
      case "fps": return value.toFixed(1);
      case "cpu": return value.toFixed(0) + "%";
      case "memory": return value.toFixed(0) + "MB";
      case "latency": return value.toFixed(1) + "ms";
      default: return value.toFixed(1);
    }
  };

  // Use the hook for FPS and CPU colors
  const fpsColor = getMetricColor("fps", currentMetrics?.dataflow_fps || 0);
  const cpuColor = getMetricColor("cpu", currentMetrics?.total_cpu_percent || 0, true);

  // Collapsed Button - Show summary for all rovers
  if (!isExpanded) {
    return (
      <div className="fixed bottom-6 right-6 z-50">
        <button
          onClick={() => setIsExpanded(true)}
          className="group relative flex items-center gap-2.5 px-4 py-3 bg-gradient-to-br from-blue-600 to-purple-600 text-white rounded-full shadow-2xl hover:shadow-blue-500/50 transition-all duration-300 hover:scale-105"
        >
          <Activity className="w-5 h-5 animate-pulse" />
          {currentMetrics && (
            <div className="flex items-center gap-3 font-mono text-sm font-bold">
              <span className={fpsColor}>
                {currentMetrics.dataflow_fps.toFixed(1)}fps
              </span>
              <span className={cpuColor}>
                {currentMetrics.total_cpu_percent.toFixed(0)}%
              </span>
              <span className="text-emerald-400">
                {((currentMetrics.total_memory_mb / currentMetrics.total_system_memory_mb) * 100).toFixed(0)}%
              </span>
              {currentMetrics.battery_level !== undefined && (
                <span className="text-amber-400">
                  🔋{currentMetrics.battery_level.toFixed(0)}%
                </span>
              )}
            </div>
          )}
          {metrics.size > 1 && (
            <span className="text-xs bg-white/20 px-2 py-0.5 rounded-full">
              {metrics.size} robots
            </span>
          )}
          <ChevronUp className="w-4 h-4 opacity-60" />
        </button>
      </div>
    );
  }

  // Expanded Dashboard
  const systemStats = currentMetrics ? [
    { label: "Dataflow FPS", value: currentMetrics.dataflow_fps, decimals: 1, color: fpsColor },
    { label: "Total CPU", value: currentMetrics.total_cpu_percent, decimals: 0, unit: "%", color: cpuColor },
    { label: "Memory", value: currentMetrics.total_memory_mb, decimals: 0, unit: "MB", color: "text-cyan-400" },
    { label: "Latency", value: currentMetrics.end_to_end_latency_ms, decimals: 1, unit: "ms", color: "text-blue-400" },
  ] : [];

  const tabs: TabType[] = ["fps", "cpu", "memory", "latency"];
  const maxValue = getMaxValue(activeTab);

  const roverList = Array.from(metrics.keys());

  return (
    <div className="fixed bottom-6 right-6 z-50 w-[420px]">
      <div className="glass-card rounded-2xl shadow-2xl border border-white/10 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-3 border-b border-white/10 bg-gradient-to-r from-blue-600/20 to-purple-600/20">
          <div className="flex items-center gap-2">
            <Zap className="w-5 h-5 text-blue-400 animate-pulse" />
            <h3 className="text-sm font-bold text-white">FLEET PERFORMANCE</h3>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={toggleMonitoring}
              className="p-1.5 rounded-lg hover:bg-white/10 transition-colors"
              title={isMonitoringEnabled ? "Pause monitoring" : "Resume monitoring"}
            >
              {isMonitoringEnabled ? (
                <Pause className="w-4 h-4 text-white" />
              ) : (
                <Play className="w-4 h-4 text-white" />
              )}
            </button>
            <button
              onClick={() => setIsExpanded(false)}
              className="p-1.5 rounded-lg hover:bg-white/10 transition-colors"
            >
              <X className="w-4 h-4 text-white" />
            </button>
          </div>
        </div>

        {/* Rover Selector - Only show if multiple rovers */}
        {roverList.length > 1 && (
          <div className="p-2 border-b border-white/10 bg-black/20">
            <div className="flex gap-2 overflow-x-auto">
              {roverList.map((roverId) => {
                const roverMetrics = metrics.get(roverId)!;
                return (
                  <button
                    key={roverId}
                    onClick={() => setSelectedRover(roverId)}
                    className={`flex-shrink-0 px-3 py-2 rounded-lg text-xs font-semibold transition-all ${
                      selectedRover === roverId
                        ? "bg-blue-500/30 text-white"
                        : "text-white/60 hover:text-white/80 hover:bg-white/5"
                    }`}
                  >
                    <div className="flex flex-col items-start gap-1">
                      <span>{roverId}</span>
                      {roverMetrics.battery_level !== undefined && (
                        <BatteryIndicator
                          level={roverMetrics.battery_level}
                          voltage={roverMetrics.battery_voltage}
                          size="sm"
                          showPercentage={false}
                          className="self-start"
                        />
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {currentMetrics && (
          <>
            {/* Rover Info */}
            <div className="p-3 border-b border-white/10 bg-black/10">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs text-white/60 font-semibold">
                  {currentMetrics.entity_id || "Unknown Rover"}
                </span>
                {currentMetrics.battery_level !== undefined && (
                  <BatteryIndicator
                    level={currentMetrics.battery_level}
                    voltage={currentMetrics.battery_voltage}
                    size="sm"
                    showPercentage
                    showVoltage
                  />
                )}
              </div>

              {/* System Stats */}
              <StatPanel stats={systemStats} columns={2} />
            </div>

            {/* Tabs */}
            <div className="flex gap-1 p-2 bg-black/20">
              {tabs.map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={`flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg text-xs font-semibold transition-all ${
                    activeTab === tab
                      ? "bg-blue-500/30 text-white"
                      : "text-white/60 hover:text-white/80 hover:bg-white/5"
                  }`}
                >
                  {getTabIcon(tab)}
                  {getTabLabel(tab)}
                </button>
              ))}
            </div>

            {/* Node Metrics */}
            <div className="p-3 space-y-2 max-h-64 overflow-y-auto custom-scrollbar">
              {Object.entries(currentMetrics.node_metrics).map(([nodeId]) => {
                const value = getMetricValue(nodeId, activeTab);
                const percentage = (value / maxValue) * 100;

                return (
                  <div key={nodeId} className="space-y-1">
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-white/80 font-medium truncate">{nodeId}</span>
                      <span className="text-white font-mono font-bold">
                        {formatValue(value, activeTab)}
                      </span>
                    </div>
                    <div className="w-full h-1.5 bg-white/10 rounded-full overflow-hidden">
                      <div
                        className={`h-full ${getMetricBarColor(value, activeTab)} transition-all duration-300`}
                        style={{ width: `${Math.min(100, percentage)}%` }}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        )}

        {metrics.size === 0 && (
          <div className="p-6 text-center text-white/40 text-sm">
            <Activity className="w-8 h-8 mx-auto mb-2 opacity-50" />
            No metrics available
          </div>
        )}
      </div>
    </div>
  );
};

export default FloatingMetrics;
