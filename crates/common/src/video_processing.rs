use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use uuid::Uuid;

struct StopInfo {
    flag: Arc<AtomicBool>,
    cancel_sender: Option<oneshot::Sender<()>>,
}

pub struct VideoProcessingManager {
    processing_videos: Mutex<HashMap<Uuid, StopInfo>>,
}

impl VideoProcessingManager {
    pub fn new() -> Self {
        VideoProcessingManager {
            processing_videos: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_video(
        &self,
        video_id: Uuid,
        stop_flag: Arc<AtomicBool>,
    ) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();

        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            println!("Warning: poisoned mutex detected while registering video!");
            poisoned.into_inner()
        });

        videos.insert(
            video_id,
            StopInfo {
                flag: stop_flag,
                cancel_sender: Some(tx),
            },
        );

        rx
    }

    pub fn stop_video(&self, video_id: &Uuid) -> bool {
        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            println!("Warning: poisoned mutex detected when video stopped!");
            poisoned.into_inner()
        });

        if let Some(stop_info) = videos.get_mut(video_id) {
            stop_info.flag.store(true, Ordering::SeqCst);
            if let Some(sender) = stop_info.cancel_sender.take() {
                let _ = sender.send(());
            }

            true
        } else {
            false
        }
    }

    pub fn unregister_video(&self, video_id: &Uuid) {
        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            println!("Warning: poisoned mutex detected while deleting video!");
            poisoned.into_inner()
        });

        videos.remove(video_id);
    }
}
