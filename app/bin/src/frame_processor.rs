use detection::model::Model;
use std::sync::Arc;
use streaming::utils::FrameProcessor;

pub fn build_frame_processor(
    model: Arc<Model>,
    model_input_width: &i32,
    model_input_height: &i32,
) -> Arc<FrameProcessor> {
    let model_input_height = model_input_height.clone();
    let model_input_width = model_input_width.clone();
    let model = model.clone();

    Arc::new(move |frame, original_w, original_h| {
        println!("Frame processing, {}", original_w);
        println!("Frame processing, {}", model_input_height);

        Ok(())
    })
}
