import React, {useCallback, useEffect, useState} from "react";
import {SpeechTranscription} from "../types/robo.ts";
import {ChevronDown, ChevronUp, Mic, MicOff, Volume2} from "lucide-react";

interface TranscriptionDisplayProps {
  transcription: SpeechTranscription | null;
  isAudioActive: boolean;
  maxHistory?: number;
  onStartAudio?: () => void;
  onStopAudio?: () => void;
}

interface TranscriptionHistoryItem extends SpeechTranscription {
  id: string;
}

export const TranscriptionDisplay: React.FC<TranscriptionDisplayProps> = ({
  transcription,
  isAudioActive,
  maxHistory = 5,
  onStartAudio,
  onStopAudio,
}) => {
  const [history, setHistory] = useState<TranscriptionHistoryItem[]>([]);
  const [isAnimating, setIsAnimating] = useState(false);
  const [isVisible, setIsVisible] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 80 }); // Initial y=80 (top-20 = 5rem = 80px)
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [hasMoved, setHasMoved] = useState(false);

  useEffect(() => {
    if (transcription) {
      // Add new transcription to history with unique ID
      const newItem: TranscriptionHistoryItem = {
        ...transcription,
        id: `${transcription.timestamp}-${Math.random()}`,
      };

      setHistory((prev) => {
        const updated = [newItem, ...prev];
        return updated.slice(0, maxHistory);
      });

      // Trigger animation
      setIsAnimating(true);
      const timer = setTimeout(() => setIsAnimating(false), 300);
      return () => clearTimeout(timer);
    }
  }, [transcription, maxHistory]);

  const getConfidenceColor = (confidence: number) => {
    if (confidence >= 0.8) return "text-green-400";
    if (confidence >= 0.6) return "text-yellow-400";
    return "text-orange-400";
  };

  const getConfidenceBadge = (confidence: number) => {
    if (confidence >= 0.8) return "bg-green-500/20 text-green-400";
    if (confidence >= 0.6) return "bg-yellow-500/20 text-yellow-400";
    return "bg-orange-500/20 text-orange-400";
  };

  // Handle drag start
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // Allow dragging from drag-handle areas
    const target = e.target as HTMLElement;
    if (target.closest('.drag-handle')) {
      setIsDragging(true);
      setHasMoved(false);
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      setDragOffset({
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
      });
      setDragStart({
        x: e.clientX,
        y: e.clientY,
      });
    }
  }, []);

  // Handle dragging
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (isDragging) {
        // Check if mouse has moved more than 5 pixels (drag threshold)
        const dx = Math.abs(e.clientX - dragStart.x);
        const dy = Math.abs(e.clientY - dragStart.y);
        if (dx > 5 || dy > 5) {
          setHasMoved(true);
        }

        setPosition({
          x: e.clientX - dragOffset.x,
          y: e.clientY - dragOffset.y,
        });
      }
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, dragOffset, dragStart]);

  // Calculate centered position when x is 0
  const centerX = position.x === 0 ? 'left-1/2 -translate-x-1/2' : '';
  const positionStyle = position.x !== 0 ? {
    left: `${position.x}px`,
    top: `${position.y}px`,
    transform: 'none',
  } : {
    top: `${position.y}px`,
  };

  // Collapsed mini indicator
  if (!isVisible) {
    return (
      <div
        className={`fixed z-40 ${centerX}`}
        style={positionStyle}
        onMouseDown={handleMouseDown}
      >
        <button
          onClick={() => {
            // Only expand if we didn't drag
            if (!hasMoved) {
              setIsVisible(true);
            }
          }}
          className="group flex items-center gap-2 px-3 py-1.5 bg-slate-900/95 backdrop-blur-md border border-slate-700/50 rounded-full shadow-lg hover:shadow-xl transition-all hover:scale-105 drag-handle cursor-move"
        >
          {isAudioActive ? (
            <>
              <Mic className="w-3.5 h-3.5 text-green-400 animate-pulse" />
              <span className="text-[10px] font-bold text-green-400 uppercase tracking-wide">Live</span>
            </>
          ) : (
            <>
              <MicOff className="w-3.5 h-3.5 text-slate-500" />
              <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wide">Off</span>
            </>
          )}
          <ChevronDown className="w-3 h-3 text-slate-400 group-hover:text-slate-300" />
        </button>
      </div>
    );
  }

  return (
    <div
      className={`fixed w-96 max-h-[40vh] bg-slate-900/95 backdrop-blur-md rounded-xl border border-slate-700/50 shadow-2xl overflow-hidden flex flex-col z-40 ${centerX} ${isDragging ? 'cursor-grabbing' : ''}`}
      style={positionStyle}
      onMouseDown={handleMouseDown}
    >
      <div className="drag-handle flex items-center justify-between px-2.5 py-1.5 border-b border-slate-700/50 bg-gradient-to-r from-slate-800/90 to-slate-900/90 cursor-move">
        <div className="flex items-center gap-1.5">
          {isAudioActive ? (
            <>
              <Mic className="w-3.5 h-3.5 text-green-400 animate-pulse" />
              <span className="text-[10px] font-bold text-green-400 uppercase tracking-wide">Live</span>
            </>
          ) : (
            <>
              <MicOff className="w-3.5 h-3.5 text-slate-500" />
              <span className="text-[10px] font-semibold text-slate-400 uppercase tracking-wide">Off</span>
            </>
          )}
        </div>

        <div className="flex items-center gap-1.5">
          {/* Audio control button */}
          {onStartAudio && onStopAudio && (
            <button
              onClick={isAudioActive ? onStopAudio : onStartAudio}
              className={`px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wide transition-all ${
                isAudioActive
                  ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
                  : "bg-green-500/20 text-green-400 hover:bg-green-500/30"
              }`}
            >
              {isAudioActive ? "Stop" : "Start"}
            </button>
          )}

          {/* Hide button */}
          <button
            onClick={() => setIsVisible(false)}
            className="p-1 rounded-md text-slate-400 hover:text-slate-300 hover:bg-slate-700/50 transition-all"
            title="Hide transcription"
          >
            <ChevronUp className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Scrollable Content */}
      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* Current Transcription */}
        {transcription && (
          <div
            className={`px-2.5 py-2 border-b border-slate-700/30 transition-all duration-200 ${
              isAnimating ? "bg-blue-500/10" : "bg-slate-800/30"
            }`}
          >
            <p className="text-xs font-medium text-slate-100 leading-tight mb-1">
              {transcription.text}
            </p>
            <div className="flex items-center gap-1.5 text-[10px]">
              <span
                className={`px-1 py-0.5 rounded text-[10px] font-bold ${getConfidenceBadge(transcription.confidence)}`}
              >
                {(transcription.confidence * 100).toFixed(0)}%
              </span>
              <span className="text-slate-500">
                {new Date(transcription.timestamp).toLocaleTimeString([], {
                  hour: '2-digit',
                  minute: '2-digit'
                })}
              </span>
              {transcription.duration_ms && (
                <span className="text-slate-500 flex items-center gap-0.5">
                  <Volume2 className="w-2.5 h-2.5" />
                  {(transcription.duration_ms / 1000).toFixed(1)}s
                </span>
              )}
            </div>
          </div>
        )}

        {/* Transcription History */}
        {history.length > 1 && (
          <div className="divide-y divide-slate-700/20">
            {history.slice(1, 4).map((item, index) => (
              <div
                key={item.id}
                className="px-2.5 py-1.5 hover:bg-slate-800/20 transition-colors"
                style={{ opacity: Math.max(0.5, 1 - index * 0.15) }}
              >
                <p className="text-[11px] text-slate-300 leading-tight mb-0.5 line-clamp-2">
                  {item.text}
                </p>
                <div className="flex items-center gap-1.5">
                  <span
                    className={`text-[10px] font-bold ${getConfidenceColor(item.confidence)}`}
                  >
                    {(item.confidence * 100).toFixed(0)}%
                  </span>
                  <span className="text-[10px] text-slate-600">
                    {new Date(item.timestamp).toLocaleTimeString([], {
                      hour: '2-digit',
                      minute: '2-digit'
                    })}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Empty State */}
        {!transcription && history.length === 0 && (
          <div className="px-2.5 py-6 text-center">
            <Mic className="w-8 h-8 text-slate-600 mx-auto mb-1.5 opacity-50" />
            <p className="text-[10px] text-slate-500 font-medium">
              {isAudioActive
                ? "Listening..."
                : "Start to transcribe"}
            </p>
          </div>
        )}
      </div>
    </div>
  );
};

export default TranscriptionDisplay;
