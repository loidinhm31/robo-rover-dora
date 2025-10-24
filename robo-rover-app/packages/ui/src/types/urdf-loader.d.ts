// Type declarations for urdf-loader
// Project: https://github.com/gkjohnson/urdf-loaders
// Definitions by: Generated for Tauri Next.js Monorepo

declare module 'urdf-loader' {
  import * as THREE from 'three';

  export interface URDFJoint {
    name: string;
    type: string;
    parent: string;
    child: string;
    axis: THREE.Vector3;
    limit: {
      lower: number;
      upper: number;
      effort: number;
      velocity: number;
    };
    setJointValue(value: number): void;
    getJointValue(): number;
  }

  export interface URDFLink {
    name: string;
    visual?: THREE.Object3D;
    collision?: THREE.Object3D;
  }

  export interface URDFRobot extends THREE.Group {
    links: { [key: string]: URDFLink };
    joints: { [key: string]: URDFJoint };
    robotName: string;
    setJointValue(jointName: string, value: number): void;
    setJointValues(values: { [key: string]: number }): void;
  }

  export default class URDFLoader {
    constructor();

    load(
      url: string,
      onLoad: (robot: URDFRobot) => void,
      onProgress?: (event: ProgressEvent) => void,
      onError?: (error: ErrorEvent) => void
    ): void;

    parse(urdfContent: string): URDFRobot;
  }
}