import React, { useState } from "react";
import { Activity, X, Pause, Play, ChevronUp, Cpu, HardDrive, Clock, Zap } from "lucide-react";
import { SystemMetrics } from "../types/robo.ts";
import { Socket } from "socket.io-client";

interface FloatingMetricsProps {
  metrics: SystemMetrics | null;
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

  const toggleMonitoring = () => {
    if (socket) {
      const newState = !isMonitoringEnabled;
      socket.emit("performance_control", { enabled: newState });
      setIsMonitoringEnabled(newState);
    }
  };

  const getFpsColor = (fps: number) => {
    if (fps > 20) return "text-emerald-400";
    if (fps > 10) return "text-amber-400";
    return "text-rose-400";
  };

  const getCpuColor = (cpu: number) => {
    if (cpu < 50) return "text-emerald-400";
    if (cpu < 80) return "text-amber-400";
    return "text-rose-400";
  };

  const getMetricValue = (nodeId: string, type: TabType): number => {
    if (!metrics?.node_metrics[nodeId]) return 0;
    const node = metrics.node_metrics[nodeId];
    switch (type) {
      case "fps": return node.fps;
      case "cpu": return node.cpu_usage_percent;
      case "memory": return node.memory_usage_mb;
      case "latency": return node.avg_processing_time_ms;
      default: return 0;
    }
  };

  const getMetricColor = (value: number, type: TabType): string => {
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
    if (!metrics) return 100;
    const values = Object.values(metrics.node_metrics).map(n => getMetricValue(n.node_id, type));
    const max = Math.max(...values, 1);

    // Add some headroom based on type
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

  // Floating Button (collapsed state)
  if (!isExpanded) {
    return (
      <div className="fixed bottom-6 right-6 z-50">
        <button
          onClick={() => setIsExpanded(true)}
          className="group relative flex items-center gap-2.5 px-4 py-3 bg-gradient-to-br from-blue-600 to-purple-600 text-white rounded-full shadow-2xl hover:shadow-blue-500/50 transition-all duration-300 hover:scale-105"
        >
          <Activity className="w-5 h-5 animate-pulse" />
          {metrics && (
            <div className="flex items-center gap-3 font-mono text-sm font-bold">
              <span className={getFpsColor(metrics.dataflow_fps)}>
                {metrics.dataflow_fps.toFixed(1)}fps
              </span>
              <span className={getCpuColor(metrics.total_cpu_percent)}>
                {metrics.total_cpu_percent.toFixed(0)}%
              </span>
              <span className="text-emerald-400">
                {((metrics.total_memory_mb / metrics.total_system_memory_mb) * 100).toFixed(0)}%
              </span>
            </div>
          )}
          <ChevronUp className="w-4 h-4 opacity-60" />
        </button>
      </div>
    );
  }

  // Expanded Dashboard (compact design with charts and tabs)
  return (
    <div className="fixed bottom-6 right-6 z-50 w-[380px]">
      <div className="glass-card rounded-2xl shadow-2xl border border-white/10 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-2.5 bg-gradient-to-r from-blue-600/20 to-purple-600/20 border-b border-white/10">
          <div className="flex items-center gap-2">
            <Zap className="w-4 h-4 text-cyan-300" />
            <span className="text-sm font-semibold text-cyan-100">
              Performance
            </span>
            {!isMonitoringEnabled && (
              <span className="text-xs px-1.5 py-0.5 rounded-full bg-amber-500/20 text-amber-300 font-medium">
                Paused
              </span>
            )}
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={toggleMonitoring}
              className={`p-1.5 rounded-lg transition-colors ${
                isMonitoringEnabled
                  ? "bg-amber-500/20 text-amber-300 hover:bg-amber-500/30"
                  : "bg-emerald-500/20 text-emerald-300 hover:bg-emerald-500/30"
              }`}
              title={isMonitoringEnabled ? "Pause monitoring" : "Resume monitoring"}
            >
              {isMonitoringEnabled ? (
                <Pause className="w-3.5 h-3.5" />
              ) : (
                <Play className="w-3.5 h-3.5" />
              )}
            </button>
            <button
              onClick={() => setIsExpanded(false)}
              className="p-1.5 rounded-lg bg-rose-500/20 text-rose-300 hover:bg-rose-500/30 transition-colors"
              title="Hide dashboard"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Metrics Content */}
        {metrics ? (
          <div className="p-3 space-y-3">
            {/* System Overview - Compact */}
            <div className="grid grid-cols-4 gap-2">
              <div className="glass-card-light rounded-lg p-2 text-center">
                <div className="text-[10px] text-cyan-200 mb-0.5 font-medium">FPS</div>
                <div className={`text-lg font-mono font-bold ${getFpsColor(metrics.dataflow_fps)}`}>
                  {metrics.dataflow_fps.toFixed(1)}
                </div>
              </div>
              <div className="glass-card-light rounded-lg p-2 text-center">
                <div className="text-[10px] text-cyan-200 mb-0.5 font-medium">CPU</div>
                <div className={`text-lg font-mono font-bold ${getCpuColor(metrics.total_cpu_percent)}`}>
                  {metrics.total_cpu_percent.toFixed(0)}%
                </div>
              </div>
              <div className="glass-card-light rounded-lg p-2 text-center">
                <div className="text-[10px] text-cyan-200 mb-0.5 font-medium">MEM ({(metrics.total_system_memory_mb / 1024).toFixed(1)}GB)</div>
                <div className="text-lg font-mono font-bold text-emerald-300">
                  {((metrics.total_memory_mb / metrics.total_system_memory_mb) * 100).toFixed(0)}%
                  <span className="text-xs text-cyan-400/60 ml-0.5">
                    ({(metrics.total_memory_mb / 1024).toFixed(1)}GB)
                  </span>
                </div>
              </div>
              <div className="glass-card-light rounded-lg p-2 text-center">
                <div className="text-[10px] text-cyan-200 mb-0.5 font-medium">LAT</div>
                <div className="text-lg font-mono font-bold text-amber-300">
                  {metrics.end_to_end_latency_ms.toFixed(0)}
                  <span className="text-xs text-cyan-400/60 ml-0.5">ms</span>
                </div>
              </div>
            </div>

            {/* Tabs */}
            <div className="flex gap-1 p-1 glass-card-light rounded-lg">
              {(["fps", "cpu", "memory", "latency"] as TabType[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={`flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-xs font-semibold transition-all ${
                    activeTab === tab
                      ? "bg-gradient-to-r from-blue-500 to-purple-500 text-white shadow-lg"
                      : "text-cyan-300/70 hover:text-cyan-200 hover:bg-white/5"
                  }`}
                >
                  {getTabIcon(tab)}
                  <span>{getTabLabel(tab)}</span>
                </button>
              ))}
            </div>

            {/* Chart */}
            <div className="glass-card-light rounded-lg p-3">
              <div className="space-y-2 max-h-48 overflow-y-auto [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
                {Object.values(metrics.node_metrics)
                  .sort((a, b) => getMetricValue(b.node_id, activeTab) - getMetricValue(a.node_id, activeTab))
                  .map((node) => {
                    const value = getMetricValue(node.node_id, activeTab);
                    const maxValue = getMaxValue(activeTab);
                    const percentage = (value / maxValue) * 100;

                    return (
                      <div
                        key={node.node_id}
                        className="group relative"
                      >
                        <div className="flex items-center justify-between text-xs mb-1">
                          <span className="text-cyan-100 font-medium truncate flex-1">
                            {node.node_id}
                          </span>
                          <span className={`font-mono font-bold ml-2 ${
                            activeTab === "fps" ? getFpsColor(value) :
                            activeTab === "cpu" ? getCpuColor(value) :
                            "text-cyan-200"
                          }`}>
                            {formatValue(value, activeTab)}
                          </span>
                        </div>
                        <div className="h-2 bg-black/30 rounded-full overflow-hidden">
                          <div
                            className={`h-full ${getMetricColor(value, activeTab)} transition-all duration-500 rounded-full`}
                            style={{ width: `${Math.min(percentage, 100)}%` }}
                          />
                        </div>

                        {/* Tooltip on hover */}
                        <div className="absolute left-0 top-full mt-1 hidden group-hover:block z-10 bg-gray-900/95 text-cyan-100 text-xs px-2 py-1.5 rounded-lg shadow-xl border border-white/10 whitespace-nowrap">
                          <div className="font-semibold text-cyan-200 mb-1">{node.node_id}</div>
                          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[11px]">
                            <span className="text-cyan-400/70">FPS:</span>
                            <span className={`font-mono ${getFpsColor(node.fps)}`}>{node.fps.toFixed(1)}</span>
                            <span className="text-cyan-400/70">CPU:</span>
                            <span className={`font-mono ${getCpuColor(node.cpu_usage_percent)}`}>{node.cpu_usage_percent.toFixed(0)}%</span>
                            <span className="text-cyan-400/70">MEM:</span>
                            <span className="font-mono text-cyan-200">{node.memory_usage_mb.toFixed(0)}MB</span>
                            <span className="text-cyan-400/70">LAT:</span>
                            <span className="font-mono text-cyan-200">{node.avg_processing_time_ms.toFixed(1)}ms</span>
                          </div>
                        </div>
                      </div>
                    );
                  })}
              </div>
            </div>

            {/* Footer Info */}
            <div className="flex items-center justify-between text-[10px] text-cyan-300/60 font-medium">
              <span>
                {Object.keys(metrics.node_metrics).length} nodes
              </span>
              <span>
                {new Date(metrics.timestamp).toLocaleTimeString()}
              </span>
            </div>
          </div>
        ) : (
          <div className="p-6 text-center">
            <Activity className="w-8 h-8 mx-auto mb-2 text-cyan-400/50 animate-pulse" />
            <p className="text-sm text-cyan-200/60">Waiting for metrics...</p>
          </div>
        )}
      </div>
    </div>
  );
};
