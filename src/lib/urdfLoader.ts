/**
 * URDF Loader Service for LeKiwi Robot
 *
 * Handles loading URDF files, configuring meshes, applying materials,
 * and updating joint positions for 3D visualization.
 *
 * Adapted from hexapod's VirtualHexapod.js but uses Three.js + urdf-loader
 * instead of Plotly for more powerful 3D robot visualization.
 */

import * as THREE from "three";
import URDFLoader from "urdf-loader";
import type { LeKiwiJointPositions } from "../types/urdf";

// ============================================================================
// URDF Loader Service Class
// ============================================================================

export class URDFLoaderService {
  private loader: URDFLoader;
  private manager: THREE.LoadingManager;
  private meshCache: Map<string, THREE.Mesh> = new Map();
  private robot: any | null = null;

  constructor() {
    // Create loading manager for progress tracking
    this.manager = new THREE.LoadingManager();

    // Create URDF loader
    this.loader = new URDFLoader(this.manager);

    // Configure loader for mesh paths
    // urdf-loader needs to know where to find meshes relative to URDF
    this.configureLoader();
  }

  // ==========================================================================
  // Configuration
  // ==========================================================================

  /**
   * Configure URDF loader with mesh path resolution
   */
  private configureLoader(): void {
    // Set base path for resolving mesh filenames
    // The URDF uses relative paths like "meshes/base_plate.stl"
    // We need to tell urdf-loader where to find them
    this.loader.packages = {
      "": "/model/", // Empty string maps to our public/model/ directory
    };

    // Set working path (base directory for URDF file)
    this.loader.workingPath = "/model/";

    // Enable loading from data URIs and cross-origin
    this.loader.loadMeshCb = (path: string, manager: THREE.LoadingManager, done: (mesh: THREE.Object3D) => void) => {
      // Check cache first
      if (this.meshCache.has(path)) {
        const cachedMesh = this.meshCache.get(path)!.clone();
        done(cachedMesh);
        return;
      }

      // Load STL mesh
      const stlLoader = new THREE.STLLoader(manager);
      stlLoader.load(
        path,
        (geometry) => {
          // Create mesh from geometry
          const material = this.createDefaultMaterial();
          const mesh = new THREE.Mesh(geometry, material);

          // Enable shadows
          mesh.castShadow = true;
          mesh.receiveShadow = true;

          // Cache for future use
          this.meshCache.set(path, mesh);

          done(mesh);
        },
        undefined,
        (error) => {
          console.error(`Failed to load mesh: ${path}`, error);
          // Return empty mesh on error
          done(new THREE.Mesh());
        }
      );
    };
  }

  // ==========================================================================
  // URDF Loading
  // ==========================================================================

  /**
   * Load URDF file and return robot model
   *
   * @param urdfPath - Path to URDF file (e.g., "/model/LeKiwi.urdf")
   * @param onProgress - Optional callback for loading progress (0-100)
   * @returns Promise resolving to THREE.Object3D robot model
   */
  async loadURDF(
    urdfPath: string,
    onProgress?: (progress: number) => void
  ): Promise<THREE.Object3D> {
    return new Promise((resolve, reject) => {
      let totalItems = 0;
      let loadedItems = 0;

      // Track loading progress
      this.manager.onStart = (url, itemsLoaded, itemsTotal) => {
        totalItems = itemsTotal;
        console.log(`Started loading: ${itemsTotal} files`);
      };

      this.manager.onProgress = (url, itemsLoaded, itemsTotal) => {
        loadedItems = itemsLoaded;
        const progress = (itemsLoaded / itemsTotal) * 100;
        onProgress?.(progress);
        console.log(`Loading progress: ${progress.toFixed(1)}% (${itemsLoaded}/${itemsTotal})`);
      };

      this.manager.onLoad = () => {
        console.log("All URDF assets loaded successfully");
        onProgress?.(100);
      };

      this.manager.onError = (url) => {
        const error = new Error(`Failed to load asset: ${url}`);
        console.error(error);
        reject(error);
      };

      // Load URDF
      this.loader.load(
        urdfPath,
        (robot: any) => {
          console.log("URDF loaded successfully:", robot);

          // Store robot reference
          this.robot = robot;

          // Process robot (apply materials, configure shadows)
          this.processRobot(robot);

          // Log joint names for debugging
          const jointNames = this.getJointNames(robot);
          console.log(`Robot has ${jointNames.length} joints:`, jointNames);

          resolve(robot);
        },
        undefined, // onProgress (handled by manager)
        (error: Error) => {
          console.error("Failed to load URDF:", error);
          reject(error);
        }
      );
    });
  }

  // ==========================================================================
  // Robot Processing
  // ==========================================================================

  /**
   * Process robot after loading
   * - Apply glassmorphic materials
   * - Configure shadows
   * - Set up rendering properties
   */
  private processRobot(robot: any): void {
    robot.traverse((child: any) => {
      if (child.isMesh) {
        // Apply glassmorphic-compatible material
        // Uses cyan color scheme from robo-control-app design system
        child.material = this.createGlassmorphicMaterial();

        // Enable shadows
        child.castShadow = true;
        child.receiveShadow = true;

        // Enable frustum culling for performance
        child.frustumCulled = true;
      }
    });

    // Configure robot-level properties
    robot.castShadow = true;
    robot.receiveShadow = true;
  }

  /**
   * Create default material for meshes
   */
  private createDefaultMaterial(): THREE.Material {
    return new THREE.MeshStandardMaterial({
      color: 0x00bcd4, // Cyan from design system
      metalness: 0.3,
      roughness: 0.4,
      transparent: false,
      opacity: 1.0,
    });
  }

  /**
   * Create glassmorphic material matching robo-control-app design
   */
  private createGlassmorphicMaterial(): THREE.Material {
    return new THREE.MeshStandardMaterial({
      color: 0x00bcd4, // Primary cyan (#00bcd4)
      metalness: 0.5,
      roughness: 0.2,
      transparent: true,
      opacity: 0.95,
      side: THREE.DoubleSide,
      // Add slight emissive glow for glassmorphic effect
      emissive: 0x0099aa,
      emissiveIntensity: 0.1,
    });
  }

  // ==========================================================================
  // Joint Control
  // ==========================================================================

  /**
   * Set joint positions on the robot
   *
   * @param robot - Robot object from URDF loader
   * @param positions - Partial joint positions (only specified joints will be updated)
   */
  setJointPositions(robot: any, positions: Partial<LeKiwiJointPositions>): void {
    if (!robot || !robot.joints) {
      console.warn("Robot or robot.joints is null");
      return;
    }

    Object.entries(positions).forEach(([jointName, angle]) => {
      if (robot.joints[jointName]) {
        try {
          // urdf-loader's setJointValue method updates joint angle
          robot.setJointValue(jointName, angle);
        } catch (error) {
          console.error(`Failed to set joint ${jointName} to ${angle}:`, error);
        }
      } else {
        console.warn(`Joint "${jointName}" not found in robot model`);
      }
    });
  }

  /**
   * Get all joint names from robot
   *
   * @param robot - Robot object from URDF loader
   * @returns Array of joint names
   */
  getJointNames(robot: any): string[] {
    if (!robot || !robot.joints) {
      return [];
    }

    return Object.keys(robot.joints);
  }

  /**
   * Get current joint positions
   *
   * @param robot - Robot object from URDF loader
   * @returns Current joint angles as LeKiwiJointPositions (or partial)
   */
  getCurrentJointPositions(robot: any): Partial<LeKiwiJointPositions> {
    if (!robot || !robot.joints) {
      return {};
    }

    const positions: any = {};

    Object.keys(robot.joints).forEach((jointName) => {
      try {
        // Get joint value from URDF robot
        const joint = robot.joints[jointName];
        if (joint && typeof joint.angle !== "undefined") {
          positions[jointName] = joint.angle;
        }
      } catch (error) {
        console.error(`Failed to get joint position for ${jointName}:`, error);
      }
    });

    return positions as Partial<LeKiwiJointPositions>;
  }

  // ==========================================================================
  // Mesh Cache Management
  // ==========================================================================

  /**
   * Clear mesh cache to free memory
   */
  clearMeshCache(): void {
    this.meshCache.clear();
    console.log("Mesh cache cleared");
  }

  /**
   * Get cache statistics
   */
  getCacheStats(): { size: number; meshes: string[] } {
    return {
      size: this.meshCache.size,
      meshes: Array.from(this.meshCache.keys()),
    };
  }

  // ==========================================================================
  // Utility Methods
  // ==========================================================================

  /**
   * Dispose of robot and free resources
   */
  dispose(): void {
    if (this.robot) {
      this.robot.traverse((child: any) => {
        if (child.geometry) child.geometry.dispose();
        if (child.material) {
          if (Array.isArray(child.material)) {
            child.material.forEach((m: THREE.Material) => m.dispose());
          } else {
            child.material.dispose();
          }
        }
      });
    }

    this.clearMeshCache();
    this.robot = null;
  }
}

// ============================================================================
// STLLoader Extension for Three.js
// ============================================================================

// Extend THREE with STLLoader (included in three/examples)
// @ts-ignore
if (typeof THREE.STLLoader === "undefined") {
  // Import STLLoader from three/examples
  const STLLoaderModule = await import("three/examples/jsm/loaders/STLLoader.js");
  // @ts-ignore
  THREE.STLLoader = STLLoaderModule.STLLoader;
}
