use crate::video_processing::VideoProcessingManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::oneshot;
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref VIDEO_MANAGER: VideoProcessingManager = VideoProcessingManager::new();
}

pub fn register_video_processing(
    video_id: Uuid,
    stop_flag: Arc<AtomicBool>,
) -> oneshot::Receiver<()> {
    VIDEO_MANAGER.register_video(video_id, stop_flag)
}

pub fn stop_video_processing(video_id: &Uuid) -> bool {
    VIDEO_MANAGER.stop_video(video_id)
}

pub fn unregister_video_processing(video_id: &Uuid) {
    VIDEO_MANAGER.unregister_video(video_id)
}
