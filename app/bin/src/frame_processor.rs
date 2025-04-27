use detection::inference::run;
use detection::model::Model;
use std::sync::Arc;
use streaming::utils::FrameProcessor;
pub fn build_frame_processor(
    model: Arc<Model>,
    model_input_width: &i32,
    model_input_height: &i32,
) -> Arc<FrameProcessor> {
    // TODO: handler panic
    // TODO: out_classes handler

    let model_input_height = model_input_height.clone();
    let model_input_width = model_input_width.clone();
    let model = model.clone();

    Arc::new(move |frame, original_w, original_h| {
        let result = run(
            model.get_session(),
            frame,
            *original_w,
            *original_h,
            model_input_width,
            model_input_height,
            Some(&[0]),
        )?;

        if result.len() > 0 {
            println!("{}", result[0].1);
        }

        Ok(())
    })
}
