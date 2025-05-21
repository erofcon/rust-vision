use anyhow::Result;
use opencv::core::{Mat, Ptr};
use opencv::hub_prelude::BackgroundSubtractorTrait;
use opencv::video;
use opencv::video::BackgroundSubtractorMOG2;

pub struct Motion {
    mog: Ptr<BackgroundSubtractorMOG2>,
    learning_rate: f64,
    motion_threshold: i32,
}

impl Motion {
    pub fn new() -> Result<Self> {
        // TODO: create config file
        let mog = video::create_background_subtractor_mog2(250, 50.0, true)?;

        Ok(Self {
            mog,
            learning_rate: -1.0,
            motion_threshold: 3000,
        })
    }

    pub fn predict(&mut self, frame: Mat) -> Result<bool> {
        let mut motion_mask = Mat::default();

        self.mog
            .apply(&frame, &mut motion_mask, self.learning_rate)?;

        let motion_pixels = opencv::core::count_non_zero(&motion_mask)?;
        let motion_detected = motion_pixels > self.motion_threshold;

        Ok(motion_detected)
    }
}
