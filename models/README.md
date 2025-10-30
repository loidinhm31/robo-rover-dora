# YOLOv12 Models for Object Detection

This directory contains YOLO models used by the `object_detector` node for real-time object detection.

## Quick Start

### Option 1: Download Pre-trained PyTorch Model and Export to ONNX (Recommended)

1. **Download YOLOv12n PyTorch model:**
```bash
cd models
curl -L -o yolo12n.pt https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12n.pt
```

2. **Install ultralytics (in a virtual environment):**
```bash
python3 -m venv venv
source venv/bin/activate
pip install ultralytics
```

3. **Export to ONNX format:**
```bash
python export_to_onnx.py
```

This will create `yolo12n.onnx` in the current directory.

### Option 2: Using Python Directly

```python
from ultralytics import YOLO

# Load YOLOv12n model (will auto-download if not present)
model = YOLO('yolo12n.pt')

# Export to ONNX format with opset 14 for ONNX Runtime 1.16 compatibility
model.export(format='onnx', simplify=True, opset=14)
```

**Important**: The `opset=14` parameter is required for compatibility with ONNX Runtime 1.16.0. Without it, the model will use a newer ONNX IR version that isn't supported.

## Available YOLOv12 Models

All models are available from the Ultralytics assets repository:

| Model | Size | mAP | Speed (ms) | Download Link |
|-------|------|-----|------------|---------------|
| YOLOv12n | 6 MB | 39.8 | 1.4 | [yolo12n.pt](https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12n.pt) |
| YOLOv12s | 12 MB | 47.0 | 2.2 | [yolo12s.pt](https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12s.pt) |
| YOLOv12m | 28 MB | 51.6 | 4.5 | [yolo12m.pt](https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12m.pt) |
| YOLOv12l | 45 MB | 53.3 | 6.8 | [yolo12l.pt](https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12l.pt) |
| YOLOv12x | 62 MB | 54.3 | 9.2 | [yolo12x.pt](https://github.com/ultralytics/assets/releases/download/v8.3.0/yolo12x.pt) |

**Note:** Speed measured on NVIDIA T4 GPU with TensorRT FP16 precision.

## Model Information

- **YOLOv12** is an attention-centric object detection framework
- Supports 80 COCO dataset classes (person, car, dog, cat, etc.)
- Input size: 640×640 pixels
- Output format: `[batch, num_features, num_detections]` where `num_features = 4 (bbox) + 80 (classes)`

## ONNX Runtime Requirements

The `object_detector` node requires ONNX Runtime 1.22.x or later.

### Install ONNX Runtime

**Linux:**
```bash
# Download ONNX Runtime 1.22.0
wget https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-linux-x64-1.22.0.tgz

# Extract
tar -xzf onnxruntime-linux-x64-1.22.0.tgz

# Set library path
export LD_LIBRARY_PATH=/path/to/onnxruntime-linux-x64-1.22.0/lib:$LD_LIBRARY_PATH
```

**Or install via pip:**
```bash
pip install onnxruntime
```

## Usage in Dora Dataflow

Add to your dataflow YAML (e.g., `web-dataflow.yml`):

```yaml
- id: object-detector
  build: cargo build --release -p object_detector
  path: target/release/object_detector
  inputs:
    frame: gst-camera/frame
  outputs:
    - detections
  env:
    MODEL_PATH: "models/yolo12n.onnx"
    CONFIDENCE_THRESHOLD: "0.5"
    NMS_THRESHOLD: "0.4"
    TARGET_CLASSES: "person,dog,cat"  # Optional: filter specific classes
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MODEL_PATH` | `models/yolo12n.onnx` | Path to ONNX model file |
| `CONFIDENCE_THRESHOLD` | `0.5` | Minimum confidence score (0.0-1.0) |
| `NMS_THRESHOLD` | `0.4` | Non-maximum suppression threshold (0.0-1.0) |
| `TARGET_CLASSES` | (empty) | Comma-separated class names to detect (e.g., "person,car,dog") |

## COCO Classes

The YOLOv12 models detect 80 object classes from the COCO dataset:

```
person, bicycle, car, motorcycle, airplane, bus, train, truck, boat,
traffic light, fire hydrant, stop sign, parking meter, bench, bird, cat,
dog, horse, sheep, cow, elephant, bear, zebra, giraffe, backpack,
umbrella, handbag, tie, suitcase, frisbee, skis, snowboard, sports ball,
kite, baseball bat, baseball glove, skateboard, surfboard, tennis racket,
bottle, wine glass, cup, fork, knife, spoon, bowl, banana, apple,
sandwich, orange, broccoli, carrot, hot dog, pizza, donut, cake, chair,
couch, potted plant, bed, dining table, toilet, tv, laptop, mouse,
remote, keyboard, cell phone, microwave, oven, toaster, sink,
refrigerator, book, clock, vase, scissors, teddy bear, hair drier, toothbrush
```

## Troubleshooting

### Model not found
- Ensure the model file is in the correct path specified by `MODEL_PATH`
- Check file permissions

### ONNX Runtime version mismatch
```
ort 2.0.0-rc.10 is not compatible with the ONNX Runtime binary found
```
**Solution:** Install ONNX Runtime 1.22.x or later (see ONNX Runtime Requirements above)

### Out of memory
- Use a smaller model (yolo12n instead of yolo12x)
- Reduce input resolution (requires model re-export)
- Enable CPU-only mode if GPU memory is limited

## References

- [Ultralytics YOLOv12 Documentation](https://docs.ultralytics.com/models/yolo12/)
- [ONNX Export Guide](https://docs.ultralytics.com/integrations/onnx/)
- [YOLOv12 GitHub Repository](https://github.com/sunsmarterjie/yolov12)
- [ONNX Runtime Releases](https://github.com/microsoft/onnxruntime/releases)
