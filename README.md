# Rust vision

Rust vision is a high-performance system for real-time video processing, object detection, and tracking.

The system uses Gstreamer and ORT packages for video capture and analytics.

> [!warning]
> This project is under active development. Most of the features have not been implemented yet.

### Roadmap to v 0.1.0
Version 0.1.0 will include basic functionality for detecting and tracking people in video files. Starting video processing will be possible only by uploading a file using a special API. After uploading, the file will be placed in the task queue, where each task will receive a unique identifier to track the processing status. 

The task completion process can be monitored in real time via an RTMP stream, which will allow you to monitor the operation of detection and tracking algorithms. For integration with the API, documentation will be provided, including methods for: 
- Video file downloads (supported formats: MP4, AVI, MOV) 
- Request the task status 
- Obtaining metadata and processing results (coordinates of objects, timestamps) 

The system will provide basic scalability: processing of several tasks will be carried out sequentially, with prioritization in turn.

- [x] **Detecting people using Yolo models**
  > It is necessary to remove other objects on the screen besides people.
- [X] **Build a pipeline to capture video from a file**
- [X] **Combine pipeline and human detection**
- [X] **Create an image output on an RTMP server**
  > It is necessary to remove the output branch on the screen
- [ ] **Create tasks on Rabitmq to run a video processing task**
- [ ] **Create an Actix Web API for processing tasks**
- [ ] **Add a system load monitoring system**
- [ ] **Add people tracking**
- [ ] **Add config file**
- [ ] **Add logging**



### The approximate structure of the project


```
rust-vision/
│
├── Cargo.toml                # Main workspace
│
├── crates/                   # Catalog with libs (crates)
│   │
│   ├── common/               # Common code used in all components
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs        
│   │       ├── config.rs
│   │       ├── models/     
│   │       │   ├── mod.rs
│   │       │   ├── video.rs
│   │       │   ├── task.rs
│   │       │   └── detection.rs
│   │       └── utils/
│   │           ├── mod.rs
│   │           ├── error.rs
│   │           └── logging.rs
│   │
│   ├── queue/                # Library for working with RabbitMQ
│   │   ├── Cargo.toml        #
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs
│   │       ├── producer.rs
│   │       └── consumer.rs
│   │
│   ├── video/                # Video Processing Library
│   │   ├── Cargo.toml        #
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pipeline.rs
│   │       ├── detector.rs
│   │       └── utils.rs
│   │
│   ├── inference/                # Video inference Library
│   │   ├── Cargo.toml        #
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── tracking/                # People tracking Library
│   │   ├── Cargo.toml        #
│   │   └── src/
│   │       └── lib.rs
│   │
│   ├── resources/            # Library for monitoring resources
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── monitor.rs
│   │       └── allocator.rs
│   │
│   ├── storage/              # Library for storing results
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── local.rs
│   │       └── s3.rs
│   │
│   ├── worker/               # Library with worker logic
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs
│   │       ├── pool.rs
│   │       └── task_processor.rs
│   │
│   └── api/                  # Library with API
│       ├── Cargo.toml        
│       └── src/
│           ├── lib.rs
│           ├── routes.rs
│           └── handlers.rs
│
├── apps/                     # Executable applications 
│   │
│   ├── api-server/           # API-server
│   │   ├── Cargo.toml      
│   │   └── src/
│   │       └── main.rs       # API Server Entry Point
│   │
│   └── worker/               # The worker executable file
│       ├── Cargo.toml        
│       └── src/
│           └── main.rs      # Entry point worker
│
├── config/                   # Config files
│   ├── api/
│   │   ├── default.toml
│   │   ├── development.toml
│   │   └── production.toml
│   └── worker/
│       ├── default.toml
│       ├── development.toml
│       └── production.toml
│
├── docs/                     # Docs
│   ├── architecture.md
│   ├── api.md
│   └── deployment.md
│
├── scripts/                  # Auxiliary scripts
│   ├── setup.sh
│   ├── run_api.sh
│   ├── run_worker.sh
│   └── docker-compose.yml
│
└── tests/                    # Integration tests of the entire system
    ├── api_tests.rs
    ├── worker_tests.rs
    └── integration_tests.rs
```


