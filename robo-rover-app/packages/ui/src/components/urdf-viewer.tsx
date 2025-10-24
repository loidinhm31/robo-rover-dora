import React, { useEffect, useRef, useState } from "react";
import { Canvas } from "@react-three/fiber";
import { Environment, Grid, OrbitControls } from "@react-three/drei";
import * as THREE from "three";
import URDFLoader from "urdf-loader";
import { JointPositions } from "@repo/ui/types/robo-rover";

interface URDFViewerProps {
  urdfPath: string;
  jointPositions: JointPositions;
  width?: string;
  height?: string;
}

const URDFRobot: React.FC<{
  urdfPath: string;
  jointPositions: JointPositions;
}> = ({ urdfPath, jointPositions }) => {
  const robotRef = useRef<THREE.Group | null>(null);
  const [robot, setRobot] = useState<any>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    const loader = new URDFLoader();

    loader.load(
      urdfPath,
      (loadedRobot: any) => {
        console.log("✅ LeKiwi URDF loaded successfully");
        console.log("📋 Available joints:", Object.keys(loadedRobot.joints));
        setRobot(loadedRobot);
        setLoadError(null);
      },
      undefined,
      (error: any) => {
        console.error("❌ Error loading URDF:", error);
        setLoadError(error.message || "Failed to load URDF");
      },
    );
  }, [urdfPath]);

  useEffect(() => {
    if (!robot) return;

    // ACTUAL LeKiwi joint mapping from LeKiwi.urdf

    // ARM joint mapping (keep existing logic)
    const armJointMapping = {
      shoulder_pan: "STS3215_03a-v1_Revolute-45", // Base rotation
      shoulder_lift: "STS3215_03a-v1-1_Revolute-49", // Shoulder pitch
      elbow_flex: "STS3215_03a-v1-2_Revolute-51", // Elbow pitch
      wrist_flex: "STS3215_03a-v1-3_Revolute-53", // Wrist pitch
      wrist_roll: "STS3215_03a_Wrist_Roll-v1_Revolute-55", // Wrist roll
      gripper: "STS3215_03a-v1-4_Revolute-57", // Gripper
    };

    // WHEEL joint mapping (NEW - for 3 mecanum wheels)
    const wheelJointMapping = {
      wheel1: "ST3215_Servo_Motor-v1-2_Revolute-60", // Bottom wheel
      wheel2: "ST3215_Servo_Motor-v1-1_Revolute-62", // Right wheel
      wheel3: "ST3215_Servo_Motor-v1_Revolute-64", // Left wheel
    };

    // Update arm joints (existing logic)
    Object.entries(armJointMapping).forEach(([key, urdfJointName]) => {
      const value = jointPositions[key as keyof typeof armJointMapping];
      if (value !== undefined && robot.joints[urdfJointName]) {
        robot.joints[urdfJointName].setJointValue(value);
      }
    });

    // Update wheel joints (NEW)
    Object.entries(wheelJointMapping).forEach(([key, urdfJointName]) => {
      const value = jointPositions[key as keyof typeof wheelJointMapping];
      if (value !== undefined && robot.joints[urdfJointName]) {
        robot.joints[urdfJointName].setJointValue(value);
        console.log(`🎡 Updated ${key} (${urdfJointName}): ${value.toFixed(3)} rad`);
      }
    });
  }, [robot, jointPositions]);

  useEffect(() => {
    if (robot && robotRef.current) {
      robotRef.current.add(robot);

      // Step 1: Center robot in view (before rotation)
      const box = new THREE.Box3().setFromObject(robot);
      const center = box.getCenter(new THREE.Vector3());
      robot.position.sub(center);

      // Step 2: Apply rotation so wheels are on the ground
      // Negative rotation to flip correctly
      robot.rotation.x = -Math.PI / 2;

      // Step 3: CRITICAL - Update world matrix after rotation
      // This ensures the bounding box calculation uses the rotated positions
      robot.updateMatrixWorld(true);

      // Step 4: Calculate bounding box AFTER rotation and matrix update
      const rotatedBox = new THREE.Box3().setFromObject(robot);
      console.log("Rotated bounding box:", rotatedBox);

      // Step 5: Adjust position so robot sits on the ground plane
      if (rotatedBox.min.y !== null && isFinite(rotatedBox.min.y)) {
        const minY = rotatedBox.min.y;
        robot.position.y -= minY; // Lift robot so lowest point is at y=0
        console.log("Adjusted robot position.y by:", -minY);
      } else {
        console.warn(
          "⚠️  Could not calculate bounding box after rotation, using default height",
        );
        robot.position.y = 0.05; // Fallback: small lift above ground
      }

      return () => {
        robotRef.current?.remove(robot);
      };
    }
  }, [robot]);

  // Show error state
  if (loadError) {
    return (
      <group>
        <mesh position={[0, 0.5, 0]}>
          <boxGeometry args={[0.1, 1, 0.1]} />
          <meshStandardMaterial color="red" />
        </mesh>
      </group>
    );
  }

  return <group ref={robotRef} />;
};

export const URDFViewer: React.FC<URDFViewerProps> = ({
                                                        urdfPath,
                                                        jointPositions,
                                                        width = "100%",
                                                        height = "600px",
                                                      }) => {
  return (
    <div
      style={{
        width,
        height,
        background: "linear-gradient(to bottom, #0f172a, #1e293b)",
        borderRadius: "1rem",
        overflow: "hidden",
      }}
    >
      <Canvas
        camera={{
          position: [0.5, 0.5, 0.5],
          fov: 50,
        }}
        shadows
      >
        {/* Lighting */}
        <ambientLight intensity={0.6} />
        <directionalLight
          position={[5, 5, 5]}
          intensity={0.8}
          castShadow
          shadow-mapSize-width={2048}
          shadow-mapSize-height={2048}
        />
        <pointLight position={[-5, 5, -5]} intensity={0.4} />

        {/* Ground Grid */}
        <Grid
          args={[2, 2]}
          cellSize={0.1}
          cellThickness={0.5}
          cellColor="#6366f1"
          sectionSize={0.5}
          sectionThickness={1}
          sectionColor="#8b5cf6"
          fadeDistance={3}
          fadeStrength={1}
          followCamera={false}
          infiniteGrid={true}
        />

        {/* Robot */}
        <URDFRobot urdfPath={urdfPath} jointPositions={jointPositions} />

        {/* Environment & Controls */}
        <Environment preset="city" />
        <OrbitControls
          makeDefault
          minPolarAngle={0}
          maxPolarAngle={Math.PI / 2}
          enableDamping
          dampingFactor={0.05}
        />
      </Canvas>
    </div>
  );
};