# Robo Rover Dora

A hybrid robotic rover control system using Dora dataflow framework.

## Prerequisites

### System Dependencies

Install GStreamer (required for video capture):
```shell
# Arch/Manjaro
sudo pacman -S gstreamer gst-plugins-base

# Ubuntu/Debian
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

Install Dora CLI:
```shell
cargo install dora-cli
```

### ONNX Runtime Setup

The object detection node requires ONNX Runtime. The library has been pre-downloaded and configured in the repository.

**If you cloned this repository**, the ONNX Runtime library should already be in `onnxruntime-linux-x64-1.16.3/` and configured in `web-dataflow.yml`.

**If you encounter this error**:
```
thread 'main' panicked at libonnxruntime.so: cannot open shared object file: No such file or directory
```

Download ONNX Runtime to the project directory:

```shell
# Download ONNX Runtime (version 1.16.3)
wget https://github.com/microsoft/onnxruntime/releases/download/v1.16.3/onnxruntime-linux-x64-1.16.3.tgz

# Extract in the project root
tar -xzf onnxruntime-linux-x64-1.16.3.tgz
```

The `web-dataflow.yml` is already configured to use this library via the `ORT_DYLIB_PATH` environment variable.

**Alternative**: Install system-wide (requires sudo):
```shell
# Copy library to system path
sudo cp onnxruntime-linux-x64-1.16.3/lib/libonnxruntime.so* /usr/local/lib/

# Update library cache
sudo ldconfig

# Remove ORT_DYLIB_PATH from web-dataflow.yml if using system install
```

## Quick Start

### 1. Build the Project

Before running any dataflow, build the Rust nodes:

For development (debug builds):
```shell
cargo build
```

For production (optimized release builds):
```shell
cargo build --release
```

### 2. Start Dora

Start the Dora daemon:
```shell
dora up
```

### 3. Run a Dataflow

**Development dataflow** (keyboard control, debug builds):
```shell
dora start dev-dataflow.yml --name robo-rover-dev --attach
```

**Web dataflow** (web UI control, release builds):
```shell
dora start web-dataflow.yml --name robo-rover-web --attach
```

### 4. Stop and Cleanup

Stop all running dataflows and destroy the daemon:
```shell
dora destroy
```

### 5. Visualize Dataflow

View the dataflow graph in your browser:
```shell
# For dev dataflow
dora graph dev-dataflow --open

# For web dataflow
dora graph web-dataflow --open
```

## Additional Documentation

For detailed information about the architecture, dataflow nodes, and development patterns, see [CLAUDE.md](CLAUDE.md).
