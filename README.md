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

Hi!

I'm trying to launch a video from the extension.dav is like this:

````
gst-launch-1.0 filesrc location=2.dav ! decodebin ! autovideosink
````

I apologize, I indicated the error incorrectly. It displays such an error:

````
Use Windows high-resolution clock, precision: 1 ms
Setting pipeline to PAUSED ...
Pipeline is PREROLLING ...
Got context from element 'autovideosink0': gst.d3d11.device.handle=context, device=(GstD3D11Device)"\(GstD3D11Device\)\ d3d11device3", adapter=(uint)0, adapter-luid=(gint64)56838, device-id=(uint)5686, vendor-id=(uint)4098, hardware=(boolean)true, description=(string)"AMD\ Radeon\(TM\)\ Graphics";
ERROR: from element /GstPipeline:pipeline0/GstDecodeBin:decodebin0/GstMpeg4VParse:mpeg4vparse0: No valid frames found before end of stream
Additional debug info:
../libs/gst/base/gstbaseparse.c(3676): gst_base_parse_loop (): /GstPipeline:pipeline0/GstDecodeBin:decodebin0/GstMpeg4VParse:mpeg4vparse0
ERROR: pipeline doesn't want to preroll.
Setting pipeline to NULL ...
Freeing pipeline ...
````

I found information on the web about the format .doc is a proprietary container used mainly by dashcam recorders and
video surveillance systems from Dahua and some other manufacturers. This format differs from standard multimedia
containers (e.g. AVI, MP4), and supports .dav is missing from many popular media players and multimedia frameworks,
including GStreamer.

Please tell me if this is the case and is there a way to run such videos without first converting?