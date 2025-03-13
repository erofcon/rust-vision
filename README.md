# Rust vision

##### A Rust-based computer vision pipeline for processing RTSP streams and video files using GStreamer, ONNX Runtime, and Actix-web. 

### The project is under development ...

## Overview

Rust vision is a high-performance system for real-time video processing, object detection, and tracking. The project provides a modular pipeline architecture that:

1. Ingests video from RTSP streams or video files via GStreamer
2. Performs object detection using ONNX Runtime
3. Tracks detected objects across frames
4. Streams processed video back through an RTSP server
5. Offers a RESTful API for configuration and stream management

## Features

- **Video Processing**: Ingest and process RTSP streams or video files
- **Object Detection**: Utilize ONNX Runtime for efficient deep learning inference
- **Object Tracking**: Track detected objects across video frames
- **Stream Output**: Serve processed video through RTSP server
- **API Server**: Configure and manage streams through a RESTful API
- **Database Integration**: Store configuration and stream data persistently

## Architecture

```
┌──────────────┐    ┌──────────────┐    ┌────────────────┐    ┌────────────────┐
│              │    │              │    │                │    │                │
│  RTSP Stream │───►│  GStreamer   │───►│ Object Detection │───►│ Object Tracking │
│  Video Files │    │  Pipeline    │    │ (ONNX Runtime) │    │                │
│              │    │              │    │                │    │                │
└──────────────┘    └──────────────┘    └────────────────┘    └────────────────┘
                                                                      │
                                                                      ▼
                    ┌──────────────┐                         ┌────────────────┐
                    │              │                         │                │
                    │   Actix-web  │◄────────────────────────┤  RTSP Server   │
                    │   API Server │                         │                │
                    │              │                         │                │
                    └──────────────┘                         └────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │              │
                    │   Database   │
                    │              │
                    │              │
                    └──────────────┘
```


## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
