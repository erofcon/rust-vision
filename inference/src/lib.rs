use anyhow::Result;
use ndarray::{Array, ArrayBase, Dim, IxDynImpl, OwnedRepr};
use ort::execution_providers::CUDAExecutionProvider;
use ort::session::Session;
use std::any::type_name_of_val;
use std::path::Path;
use std::sync::Arc;

pub struct Inference {
    session: Arc<Session>,
}

impl Inference {
    pub fn new(model_path: &str) -> Self {
        ort::init()
            .with_execution_providers([CUDAExecutionProvider::default().build()])
            .commit()
            .unwrap();

        let session = Arc::new(
            Session::builder()
                .unwrap()
                .commit_from_file(model_path)
                .unwrap(),
        );

        println!("Initializing model from: {}", model_path);

        Self { session }
    }

    pub fn run(&self, input: Array<f32, ndarray::Dim<[usize; 4]>>) -> Result<()> {

        let input = ort::inputs!["images"=>input]?;
        let outputs = self.session.run(input)?;

        let output = outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        Ok(())
    }

    fn parse_prediction(output: ArrayBase<OwnedRepr<f32>, Dim<IxDynImpl>>) {
        println!("{}", type_name_of_val(&output));
    }
}
