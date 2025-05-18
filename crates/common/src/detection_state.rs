use gst_video::VideoFrame;
use gst_video::video_frame::Readable;

enum DetectionMode {
    PersonDetection,
    MotionDetection,
}

fn detect_persons(_frame: &VideoFrame<Readable>) -> bool {
    false
    // unimplemented!("Call to your person detection implementation")
}

fn detect_motion(_frame: &VideoFrame<Readable>) -> bool {
    false
    // unimplemented!("Call to your motion detection implementation")
}

pub struct DetectionState {
    mode: DetectionMode,
    frames_without_detection: usize,
    max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(max_empty_frames: usize) -> Self {
        Self {
            mode: DetectionMode::PersonDetection, // Start with person detection
            frames_without_detection: 0,
            max_empty_frames,
        }
    }

    pub fn process_frame(&mut self, frame: &VideoFrame<Readable>) -> bool {
        match self.mode {
            DetectionMode::PersonDetection => {
                let detected = detect_persons(frame);

                if detected {
                    self.frames_without_detection = 0;
                    true
                } else {
                    self.frames_without_detection += 1;

                    if self.frames_without_detection >= self.max_empty_frames {
                        // TODO: add to log
                        println!(
                            "Switching to motion detection after {} frames without persons",
                            self.max_empty_frames
                        );
                        self.mode = DetectionMode::MotionDetection;
                        self.frames_without_detection = 0;
                    }
                    false
                }
            }
            DetectionMode::MotionDetection => {
                println!("Motion detection");
                let motion_detected = detect_motion(frame);

                if motion_detected {
                    // TODO: add to log
                    // If motion detected, switch back to person detection
                    println!("Motion detected! Switching back to person detection");
                    self.mode = DetectionMode::PersonDetection;
                    self.frames_without_detection = 0;

                    return detect_persons(frame);
                }
                false
            }
        }
    }
}
