//! Runs a model that was trained in PyTorch and exported to ONNX.
//!
//! Create the model first with `python/train_export.py`. Neither Python nor
//! PyTorch is needed here: everything the network does is in `spiral.onnx`,
//! and ONNX Runtime executes it.

use std::error::Error;
use std::fs;

use ort::session::Session;
use ort::value::TensorRef;
use serde::Deserialize;

const MODEL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/model/spiral.onnx");
const VECTORS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/model/test_vectors.json");

/// The largest difference from PyTorch's output we accept. The two runtimes
/// use different numeric kernels, so tiny rounding differences are normal, but
/// anything larger would mean the export changed the model.
const TOLERANCE: f32 = 1e-4;

/// Held-out points, with the answers PyTorch gave for them.
#[derive(Deserialize)]
struct TestVectors {
    points: Vec<[f32; 2]>,
    labels: Vec<usize>,
    logits: Vec<[f32; 2]>,
}

/// Runs the model on a batch of points and returns one pair of logits per point.
fn predict(session: &mut Session, points: &[[f32; 2]]) -> Result<Vec<[f32; 2]>, Box<dyn Error>> {
    // ONNX Runtime takes a shape plus a flat buffer, with no copy on the way in.
    let input = TensorRef::from_array_view(([points.len(), 2], points.as_flattened()))?;
    // The names are the ones given to `torch.onnx.export`.
    let outputs = session.run(ort::inputs!["points" => input])?;
    let (_shape, logits) = outputs["logits"].try_extract_tensor::<f32>()?;
    Ok(logits.as_chunks::<2>().0.to_vec())
}

/// Index of the class with the higher logit.
fn class_of(logits: [f32; 2]) -> usize {
    usize::from(logits[1] > logits[0])
}

/// Converts logits to probabilities. The exported model stops at logits,
/// because the softmax was part of the loss function, not of the network.
fn softmax(logits: [f32; 2]) -> [f32; 2] {
    let [a, b] = logits.map(f32::exp);
    [a / (a + b), b / (a + b)]
}

fn describe_model(session: &Session) {
    println!("=== 1. The loaded model ===");
    for input in session.inputs() {
        println!("Input:  {} {:?}", input.name(), input.dtype());
    }
    for output in session.outputs() {
        println!("Output: {} {:?}", output.name(), output.dtype());
    }
}

/// Compares against PyTorch, which is what shows that the export is faithful.
fn check_against_pytorch(session: &mut Session) -> Result<(), Box<dyn Error>> {
    println!("\n=== 2. Same answers as PyTorch? ===");
    let vectors: TestVectors = serde_json::from_str(&fs::read_to_string(VECTORS_PATH)?)?;

    let logits = predict(session, &vectors.points)?;

    let largest_difference = logits
        .iter()
        .flatten()
        .zip(vectors.logits.iter().flatten())
        .map(|(rust, python)| (rust - python).abs())
        .fold(0.0, f32::max);
    println!(
        "Largest difference from PyTorch over {} points: {largest_difference:.2e}",
        vectors.points.len()
    );
    if largest_difference > TOLERANCE {
        return Err(format!("outputs differ by more than {TOLERANCE}").into());
    }

    let correct = logits
        .iter()
        .zip(&vectors.labels)
        .filter(|&(&l, &label)| class_of(l) == label)
        .count();
    println!(
        "Test accuracy: {:.1}%",
        100.0 * correct as f32 / vectors.labels.len() as f32
    );
    Ok(())
}

fn classify_points(session: &mut Session) -> Result<(), Box<dyn Error>> {
    println!("\n=== 3. Classifying new points ===");
    let points = [[0.5, 0.0], [-0.3, 0.2], [0.0, -0.8]];
    let logits = predict(session, &points)?;
    for (point, logits) in points.iter().zip(logits) {
        let probabilities = softmax(logits);
        let class = class_of(logits);
        println!(
            "{point:?} -> class {class} ({:.1}% sure)",
            100.0 * probabilities[class]
        );
    }
    Ok(())
}

/// Draws the decision boundary. All cells go through the model as one batch,
/// which works because the batch size was left dynamic in the export.
fn draw_decision_map(session: &mut Session) -> Result<(), Box<dyn Error>> {
    const COLUMNS: usize = 61;
    const ROWS: usize = 25;
    println!("\n=== 4. What the network learned ===");

    let mut grid = Vec::with_capacity(COLUMNS * ROWS);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let x = -1.1 + 2.2 * column as f32 / (COLUMNS - 1) as f32;
            let y = 1.1 - 2.2 * row as f32 / (ROWS - 1) as f32;
            grid.push([x, y]);
        }
    }

    let logits = predict(session, &grid)?;
    for row in logits.as_chunks::<COLUMNS>().0 {
        let line: String = row
            .iter()
            .map(|&l| if class_of(l) == 0 { '.' } else { '#' })
            .collect();
        println!("{line}");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let environment = ort::init().build()?;
    let mut session = Session::builder(&environment)?
        .commit_from_file(MODEL_PATH)
        .map_err(|e| {
            format!("cannot load {MODEL_PATH} ({e}). Run python/train_export.py first.")
        })?;

    describe_model(&session);
    check_against_pytorch(&mut session)?;
    classify_points(&mut session)?;
    draw_decision_map(&mut session)?;
    Ok(())
}
