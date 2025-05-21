# Rust vision

Rust vision is a high-performance system for real-time video processing, object detection, and tracking.

The system uses Gstreamer and ORT packages for video capture and analytics.

> [!warning]
> This project is under active development. Most of the features have not been implemented yet.

[API Documentation](./docs/api.md)

### The structure of the project

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
│   ├── streaming/                # Video Processing Library
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
├── app/                     # Executable applications 
│   │
│   └── bin/           # The executable file
│      ├── Cargo.toml      
│      └── src/
│          └── main.rs       # Entry worker
│   
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

 