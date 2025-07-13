"use client";

import dynamic from "next/dynamic";
import { Suspense } from "react";

// Dynamically import the controller to avoid SSR issues with Socket.IO
const RoboRoverController = dynamic(
  () => import("@repo/ui/views/robo-rover-control"),
  {
    ssr: false,
    loading: () => (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
          <p className="text-gray-600">Loading Robo Rover Controller...</p>
        </div>
      </div>
    ),
  },
);

export default function RoboRoverPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen bg-gray-50 flex items-center justify-center">
          <div className="text-center">
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto mb-4"></div>
            <p className="text-gray-600">Initializing...</p>
          </div>
        </div>
      }
    >
      <RoboRoverController />
    </Suspense>
  );
}
