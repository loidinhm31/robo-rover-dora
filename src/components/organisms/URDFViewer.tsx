/**
 * URDF Viewer Component
 *
 * 3D visualization component for URDF-based robots using React Three Fiber.
 * Adapted from hexapod's HexapodPlot.js but uses Three.js instead of Plotly.
 *
 * Features:
 * - Dynamic URDF loading with progress tracking
 * - Real-time joint position updates
 * - Interactive camera controls (OrbitControls)
 * - Glassmorphic rendering with shadows
 * - Grid and environment for spatial reference
 */

import React, { useRef, useEffect, useState, Suspense } from "react";
import { Canvas } from "@react-three/fiber";
import { OrbitControls, Environment, Grid, PerspectiveCamera } from "@react-three/drei";
import * as THREE from "three";
import { URDFLoaderService } from "../../lib/urdfLoader";
import type { LeKiwiJointPositions } from "../../types/urdf";
import { LoadingSpinner } from "../atoms/LoadingSpinner";

// ============================================================================
// URDF Model Component (Inner)
// ============================================================================

interface URDFModelProps {
  urdfPath: string;
  jointPositions: LeKiwiJointPositions;
  onLoadComplete?: () => void;
  onLoadError?: (error: Error) => void;
  onLoadProgress?: (progress: number) => void;
}

const URDFModel: React.FC<URDFModelProps> = ({
  urdfPath,
  jointPositions,
  onLoadComplete,
  onLoadError,
  onLoadProgress,
}) => {
  const robotRef = useRef<THREE.Object3D | null>(null);
  const loaderRef = useRef<URDFLoaderService | null>(null);
  const [robot, setRobot] = useState<any | null>(null);

  // Load URDF on mount
  useEffect(() => {
    const loader = new URDFLoaderService();
    loaderRef.current = loader;

    loader
      .loadURDF(urdfPath, onLoadProgress)
      .then((loadedRobot) => {
        setRobot(loadedRobot);
        robotRef.current = loadedRobot;
        onLoadComplete?.();
        console.log("URDF loaded and ready for rendering");
      })
      .catch((error) => {
        console.error("URDF load error:", error);
        onLoadError?.(error);
      });

    // Cleanup
    return () => {
      loader.dispose();
    };
  }, [urdfPath, onLoadComplete, onLoadError, onLoadProgress]);

  // Update joint positions when they change
  useEffect(() => {
    if (robot && loaderRef.current) {
      loaderRef.current.setJointPositions(robot, jointPositions);
    }
  }, [robot, jointPositions]);

  // Render robot if loaded
  if (!robot) return null;

  return <primitive object={robot} ref={robotRef} />;
};

// ============================================================================
// URDF Viewer Component (Outer)
// ============================================================================

export interface URDFViewerProps {
  urdfPath: string;
  jointPositions: LeKiwiJointPositions;
  onLoadComplete?: () => void;
  onLoadError?: (error: Error) => void;
  showGrid?: boolean;
  className?: string;
}

export const URDFViewer: React.FC<URDFViewerProps> = ({
  urdfPath,
  jointPositions,
  onLoadComplete,
  onLoadError,
  showGrid = true,
  className = "",
}) => {
  const [loadProgress, setLoadProgress] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const handleLoadComplete = () => {
    setIsLoading(false);
    setLoadProgress(100);
    onLoadComplete?.();
  };

  const handleLoadError = (err: Error) => {
    setIsLoading(false);
    setError(err.message);
    onLoadError?.(err);
  };

  const handleLoadProgress = (progress: number) => {
    setLoadProgress(progress);
  };

  return (
    <div className={`relative w-full h-full ${className}`}>
      {/* Loading Overlay */}
      {isLoading && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/20 backdrop-blur-sm z-10 rounded-2xl">
          <LoadingSpinner />
          <p className="mt-4 text-white font-medium">Loading URDF Model...</p>
          <p className="mt-2 text-white/70 text-sm">{Math.round(loadProgress)}%</p>
          <div className="w-64 h-2 bg-white/10 rounded-full mt-2 overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-cyan-400 to-purple-400 transition-all duration-300"
              style={{ width: `${loadProgress}%` }}
            />
          </div>
        </div>
      )}

      {/* Error Overlay */}
      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-red-500/10 backdrop-blur-sm z-10 rounded-2xl">
          <div className="text-red-400 text-6xl mb-4">⚠️</div>
          <p className="text-white font-semibold text-lg">Failed to Load URDF</p>
          <p className="text-white/70 text-sm mt-2 max-w-md text-center px-4">{error}</p>
        </div>
      )}

      {/* 3D Canvas */}
      <Canvas
        shadows
        camera={{ position: [2, 2, 2], fov: 50 }}
        style={{ background: "transparent" }}
        gl={{ antialias: true, alpha: true }}
      >
        {/* Lighting Setup */}
        <Lighting />

        {/* Environment (HDR lighting) */}
        <Environment preset="city" />

        {/* Grid (optional) */}
        {showGrid && (
          <Grid
            args={[10, 10]}
            cellSize={0.5}
            cellThickness={0.5}
            cellColor="#6366f1"
            sectionSize={2}
            sectionThickness={1}
            sectionColor="#8b5cf6"
            fadeDistance={30}
            fadeStrength={1}
            followCamera={false}
            infiniteGrid={true}
          />
        )}

        {/* Robot Model */}
        <Suspense fallback={null}>
          <URDFModel
            urdfPath={urdfPath}
            jointPositions={jointPositions}
            onLoadComplete={handleLoadComplete}
            onLoadError={handleLoadError}
            onLoadProgress={handleLoadProgress}
          />
        </Suspense>

        {/* Camera Controls */}
        <OrbitControls
          enableDamping
          dampingFactor={0.05}
          minDistance={0.5}
          maxDistance={10}
          target={[0, 0.5, 0]}
          enablePan={true}
          enableZoom={true}
          enableRotate={true}
          // Touch controls for mobile
          touches={{
            ONE: THREE.TOUCH.ROTATE,
            TWO: THREE.TOUCH.DOLLY_PAN,
          }}
        />
      </Canvas>
    </div>
  );
};

// ============================================================================
// Lighting Component
// ============================================================================

const Lighting: React.FC = () => {
  return (
    <>
      {/* Ambient light for overall illumination */}
      <ambientLight intensity={0.5} />

      {/* Main directional light (sun) with shadows */}
      <directionalLight
        position={[5, 5, 5]}
        intensity={1}
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={50}
        shadow-camera-left={-10}
        shadow-camera-right={10}
        shadow-camera-top={10}
        shadow-camera-bottom={-10}
      />

      {/* Fill light from opposite side */}
      <directionalLight position={[-5, 3, -5]} intensity={0.3} />

      {/* Point light for highlights */}
      <pointLight position={[0, 3, 0]} intensity={0.4} color="#00bcd4" />
    </>
  );
};

// ============================================================================
// Export
// ============================================================================

export default URDFViewer;
