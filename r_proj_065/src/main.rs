//! A tour of PyTorch from Rust, using the `tch` crate (bindings to libtorch,
//! the C++ library underneath PyTorch).
//!
//! 1. Tensors: creating them and doing arithmetic
//! 2. Autograd: letting PyTorch compute a derivative
//! 3. Gradient descent by hand: linear regression with raw tensors
//! 4. A real neural network: training a classifier with `nn` and Adam
//! 5. Saving the trained weights and loading them into a new model

use tch::data::Iter2;
use tch::nn::{self, Module, OptimizerConfig};
use tch::{Device, Kind, TchError, Tensor};

const FLOAT: (Kind, Device) = (Kind::Float, Device::Cpu);

fn main() -> Result<(), TchError> {
    // Makes the random numbers, and so the results, repeatable.
    tch::manual_seed(42);

    // Uses an NVIDIA GPU if libtorch was built with CUDA support and one exists.
    let device = Device::cuda_if_available();
    println!("Running on: {device:?}");

    tensor_basics();
    autograd();
    linear_regression();
    train_classifier(device)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. Tensors
// ---------------------------------------------------------------------------

fn tensor_basics() {
    println!("\n=== 1. Tensors ===");

    let a = Tensor::from_slice(&[1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0]).reshape([2, 3]);
    let b = Tensor::ones([2, 3], FLOAT) * 10;
    println!("a has shape {:?}:", a.size());
    a.print();

    // Operators borrow their operands, so `a` and `b` stay usable afterwards.
    println!("a + b:");
    (&a + &b).print();

    // `tr()` transposes, `matmul` is the matrix product: (2x3) * (3x2) = (2x2).
    println!("a * aᵀ:");
    a.matmul(&a.tr()).print();

    println!("Mean of a: {}", a.mean(Kind::Float).double_value(&[]));

    // Convert a tensor back into ordinary Rust data.
    let values = Vec::<f32>::try_from(a.flatten(0, -1)).expect("a holds f32 values");
    println!("As a Vec: {values:?}");
}

// ---------------------------------------------------------------------------
// 2. Autograd
// ---------------------------------------------------------------------------

fn autograd() {
    println!("\n=== 2. Autograd ===");

    // y = x³ + 2x, so dy/dx = 3x² + 2, which is 14 at x = 2.
    let x = Tensor::from_slice(&[2.0_f32]).set_requires_grad(true);
    let y = &x * &x * &x + &x * 2;
    y.backward();

    println!("y(2)  = {}", y.double_value(&[0]));
    println!("dy/dx = {} (expected 14)", x.grad().double_value(&[0]));
}

// ---------------------------------------------------------------------------
// 3. Gradient descent by hand
// ---------------------------------------------------------------------------

fn linear_regression() {
    println!("\n=== 3. Linear regression by hand ===");

    // Noisy samples of the line y = 3x + 2. Training should recover 3 and 2.
    let xs = Tensor::rand([200, 1], FLOAT);
    let noise = Tensor::randn([200, 1], FLOAT) * 0.05;
    let ys = &xs * 3 + 2 + noise;

    let mut weight = Tensor::zeros([1], FLOAT).set_requires_grad(true);
    let mut bias = Tensor::zeros([1], FLOAT).set_requires_grad(true);
    let learning_rate = 0.5;

    for step in 1..=300 {
        let prediction = &xs * &weight + &bias;
        let loss = prediction.mse_loss(&ys, tch::Reduction::Mean);

        weight.zero_grad();
        bias.zero_grad();
        loss.backward();

        // Updating the parameters must not itself be recorded for autograd.
        tch::no_grad(|| {
            weight -= weight.grad() * learning_rate;
            bias -= bias.grad() * learning_rate;
        });

        if step % 100 == 0 {
            println!("step {step:3}  loss {:.5}", loss.double_value(&[]));
        }
    }

    println!(
        "Learned y = {:.3}x + {:.3} (true line: y = 3x + 2)",
        weight.double_value(&[0]),
        bias.double_value(&[0])
    );
}

// ---------------------------------------------------------------------------
// 4 + 5. A neural network, and saving it
// ---------------------------------------------------------------------------

/// A small fully connected network. Its parameters are registered in a
/// `VarStore`, which is what the optimizer updates and what gets saved to disk.
#[derive(Debug)]
struct SpiralNet {
    hidden1: nn::Linear,
    hidden2: nn::Linear,
    output: nn::Linear,
}

impl SpiralNet {
    fn new(vs: &nn::Path) -> Self {
        let config = Default::default();
        Self {
            hidden1: nn::linear(vs / "hidden1", 2, 64, config),
            hidden2: nn::linear(vs / "hidden2", 64, 64, config),
            output: nn::linear(vs / "output", 64, 2, config),
        }
    }
}

/// The Rust counterpart of defining `forward` on a Python `nn.Module`.
impl Module for SpiralNet {
    fn forward(&self, xs: &Tensor) -> Tensor {
        xs.apply(&self.hidden1)
            .relu()
            .apply(&self.hidden2)
            .relu()
            .apply(&self.output)
    }
}

/// Two interleaved spirals: not separable by a straight line, so the network
/// has to learn a curved decision boundary. Returns `(points, labels)`.
fn make_spirals(points_per_class: i64, device: Device) -> (Tensor, Tensor) {
    let float = (Kind::Float, device);
    let n = points_per_class;

    let radius = Tensor::linspace(0.1, 1.0, n, float);
    let angle = &radius * 3.0 * std::f64::consts::TAU;
    let spiral = |offset: f64| {
        let theta = &angle + offset;
        Tensor::stack(&[&radius * theta.cos(), &radius * theta.sin()], 1)
    };

    let clean = Tensor::cat(&[spiral(0.0), spiral(std::f64::consts::PI)], 0);
    let points = clean + Tensor::randn([2 * n, 2], float) * 0.02;
    let labels = Tensor::cat(
        &[
            Tensor::zeros([n], (Kind::Int64, device)),
            Tensor::ones([n], (Kind::Int64, device)),
        ],
        0,
    );
    (points, labels)
}

/// Fraction of `points` the model labels correctly.
fn accuracy(model: &impl Module, points: &Tensor, labels: &Tensor) -> f64 {
    // `no_grad` skips the bookkeeping autograd only needs during training.
    tch::no_grad(|| model.forward(points).accuracy_for_logits(labels)).double_value(&[])
}

fn train_classifier(device: Device) -> Result<(), TchError> {
    println!("\n=== 4. Training a neural network ===");

    // Shuffle, then hold back 20% of the data to test on points never trained on.
    let (points, labels) = make_spirals(1000, device);
    let total = points.size()[0];
    let order = Tensor::randperm(total, (Kind::Int64, device));
    let (points, labels) = (
        points.index_select(0, &order),
        labels.index_select(0, &order),
    );
    let train_size = total * 8 / 10;
    let test_size = total - train_size;
    let (train_x, test_x) = (
        points.narrow(0, 0, train_size),
        points.narrow(0, train_size, test_size),
    );
    let (train_y, test_y) = (
        labels.narrow(0, 0, train_size),
        labels.narrow(0, train_size, test_size),
    );

    let vs = nn::VarStore::new(device);
    let model = SpiralNet::new(&vs.root());
    let mut optimizer = nn::Adam::default().build(&vs, 5e-3)?;

    println!(
        "Accuracy before training: {:.1}%",
        100.0 * accuracy(&model, &test_x, &test_y)
    );

    for epoch in 1..=200 {
        let mut batches = Iter2::new(&train_x, &train_y, 64);
        let mut total_loss = 0.0;
        let mut batch_count = 0.0;

        for (batch_x, batch_y) in batches.to_device(device).shuffle() {
            let loss = model.forward(&batch_x).cross_entropy_for_logits(&batch_y);
            // Zero the gradients, backpropagate, and update the weights.
            optimizer.backward_step(&loss);
            total_loss += loss.double_value(&[]);
            batch_count += 1.0;
        }

        if epoch % 50 == 0 {
            println!(
                "epoch {epoch:3}  loss {:.4}  test accuracy {:.1}%",
                total_loss / batch_count,
                100.0 * accuracy(&model, &test_x, &test_y)
            );
        }
    }

    println!("\n=== 5. Saving and loading ===");

    let path = std::env::temp_dir().join("spiral_net.safetensors");
    vs.save(&path)?;
    println!("Saved weights to {}", path.display());

    // A brand new model starts with random weights, until `load` replaces them.
    let mut restored_vs = nn::VarStore::new(device);
    let restored = SpiralNet::new(&restored_vs.root());
    println!(
        "New model, random weights: {:.1}%",
        100.0 * accuracy(&restored, &test_x, &test_y)
    );
    restored_vs.load(&path)?;
    println!(
        "New model, loaded weights: {:.1}%",
        100.0 * accuracy(&restored, &test_x, &test_y)
    );

    Ok(())
}
