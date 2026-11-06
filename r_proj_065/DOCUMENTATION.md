# Code Documentation

This document explains how the example in `src/main.rs` works and what its code does. It assumes you know some Rust, but no PyTorch or machine learning. For a quick start, see [README.md](README.md).

## Contents

1. [The big picture](#1-the-big-picture)
2. [Core concepts](#2-core-concepts)
3. [Startup: `main`](#3-startup-main)
4. [Example 1: Tensors](#4-example-1-tensors)
5. [Example 2: Autograd](#5-example-2-autograd)
6. [Example 3: Linear regression by hand](#6-example-3-linear-regression-by-hand)
7. [Example 4: Training a neural network](#7-example-4-training-a-neural-network)
8. [Example 5: Saving and loading](#8-example-5-saving-and-loading)
9. [Expected output](#9-expected-output)
10. [Errors and panics](#10-errors-and-panics)
11. [Known limitations](#11-known-limitations)
12. [Ideas for experiments](#12-ideas-for-experiments)

---

## 1. The big picture

PyTorch has three layers, and this project replaces only the top one:

```text
 Python PyTorch                 This project
 ──────────────                 ────────────
 Your Python code               src/main.rs
        │                            │
 torch (Python API)             tch (safe Rust API)
        │                            │
        │                       torch-sys (FFI bindings)
        │                            │
        └──────────► libtorch ◄──────┘
                 (C++ and BLAS/CUDA)
```

Both routes end in the same C++ library, `libtorch`, which is why the Rust API looks so much like the Python one. It also explains two things you will notice when using it:

- **Speed comes from libtorch.** Rust only tells libtorch what to do, and the calculations run in precompiled C++. Debug and release builds of this example take about the same time, roughly 2.4 seconds.
- **The program needs a shared library at run time.** The `download-libtorch` feature in `Cargo.toml` fetches libtorch while building, and the finished binary links to `libtorch_cpu.so` dynamically. `cargo run` tells the binary where to find it, but starting the binary directly needs `LD_LIBRARY_PATH` (see the README).

The whole program is one file, with one function per example:

| Function | Purpose |
| --- | --- |
| `main` | Sets the random seed, picks a device, runs the examples |
| `tensor_basics` | Example 1 |
| `autograd` | Example 2 |
| `linear_regression` | Example 3 |
| `SpiralNet` (struct and `Module` impl) | The neural network used in examples 4 and 5 |
| `make_spirals` | Generates the training data |
| `accuracy` | Measures how many points a model labels correctly |
| `train_classifier` | Examples 4 and 5 |

---

## 2. Core concepts

**Tensor.** A multi-dimensional array of numbers of a single type. Its *shape* lists the size of each dimension: `[2, 3]` is a matrix with 2 rows and 3 columns, `[200, 1]` is a column of 200 numbers, and `[]` is a single number. A Rust `Tensor` is a handle to an object owned by libtorch, which is freed when the handle is dropped.

**Kind and device.** Every tensor has a number type (`Kind::Float` is 32-bit float, `Kind::Int64` is 64-bit integer) and a device, which is where its data lives (`Device::Cpu` or `Device::Cuda(0)`). Tensors in the same calculation must be on the same device. Many constructors take both as a tuple, such as `(Kind::Float, Device::Cpu)`.

**Gradient.** The derivative of a result with respect to an input. It says how much the result changes when that input changes slightly, and in which direction.

**Autograd.** When a tensor has `requires_grad` set, libtorch records every calculation that depends on it. Calling `backward()` on the result walks that record in reverse and stores a gradient on each input tensor.

**Loss.** A single number that measures how wrong a model is. Training means adjusting the parameters to make the loss smaller.

**Gradient descent.** Subtracting a small multiple of the gradient from each parameter, which moves it in the direction that lowers the loss. The multiple is called the *learning rate*.

---

## 3. Startup: `main`

```rust
const FLOAT: (Kind, Device) = (Kind::Float, Device::Cpu);

fn main() -> Result<(), TchError> {
    tch::manual_seed(42);

    let device = Device::cuda_if_available();
    println!("Running on: {device:?}");

    tensor_basics();
    autograd();
    linear_regression();
    train_classifier(device)?;
    Ok(())
}
```

- **`FLOAT`** is a shorthand for the kind and device used in examples 1 to 3. These small examples always run on the CPU. Only `train_classifier` uses the detected `device`.
- **`tch::manual_seed(42)`** fixes libtorch's random number generator. Random weights, noise and shuffling then come out the same each time. Two runs here produced identical output.
- **`Device::cuda_if_available()`** returns `Device::Cuda(0)`, the first GPU, if libtorch was built with CUDA and finds a GPU. Otherwise it returns `Device::Cpu`.
- **`Result<(), TchError>`** lets `main` use `?` on the few calls that return a `Result`.

---

## 4. Example 1: Tensors

Function: `tensor_basics`. This example shows how to create tensors, calculate with them and get the numbers back out.

### Creating tensors

```rust
let a = Tensor::from_slice(&[1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0]).reshape([2, 3]);
let b = Tensor::ones([2, 3], FLOAT) * 10;
```

`from_slice` copies a Rust slice into a one-dimensional tensor of shape `[6]`, and its `Kind` follows from the Rust type (`f32` becomes `Kind::Float`). `reshape` rearranges the same six values row by row:

```text
a = 1 2 3      b = 10 10 10
    4 5 6          10 10 10
```

`b` starts as all ones. Multiplying by the plain number `10` applies to every element.

### Arithmetic

```rust
(&a + &b).print();
a.matmul(&a.tr()).print();
```

**`&a + &b`** adds element by element. The `&` matters: `tch` implements the operators for both owned tensors and references, and `a + b` would move both tensors, making them unusable afterwards. References keep them available.

**`a.matmul(&a.tr())`** is the matrix product of `a` with its transpose. `tr()` swaps rows and columns, turning shape `[2, 3]` into `[3, 2]`, so the result has shape `[2, 2]`:

```text
1·1 + 2·2 + 3·3 = 14       1·4 + 2·5 + 3·6 = 32
4·1 + 5·2 + 6·3 = 32       4·4 + 5·5 + 6·6 = 77
```

### Getting values back into Rust

```rust
println!("Mean of a: {}", a.mean(Kind::Float).double_value(&[]));
let values = Vec::<f32>::try_from(a.flatten(0, -1)).expect("a holds f32 values");
```

- **`a.mean(Kind::Float)`** returns a tensor of shape `[]` that holds one number, here `3.5`. It is still a tensor, not an `f64`.
- **`double_value(&[])`** reads one element as an `f64`. The argument is the element's index, and a tensor with no dimensions has an empty index.
- **`a.flatten(0, -1)`** turns all dimensions into one, because the `Vec<f32>` conversion needs a one-dimensional tensor. `-1` means the last dimension.
- **`Vec::<f32>::try_from`** copies the values into a normal `Vec`. It returns an error if the tensor's kind cannot be converted, and `expect` turns that error into a panic.

---

## 5. Example 2: Autograd

Function: `autograd`. This example has PyTorch calculate a derivative that is easy to check by hand. For `y = x³ + 2x`, the derivative is `dy/dx = 3x² + 2`, which is `14` at `x = 2`.

```rust
let x = Tensor::from_slice(&[2.0_f32]).set_requires_grad(true);
let y = &x * &x * &x + &x * 2;
y.backward();

println!("y(2)  = {}", y.double_value(&[0]));
println!("dy/dx = {} (expected 14)", x.grad().double_value(&[0]));
```

1. **`set_requires_grad(true)`** tells libtorch to record every calculation that depends on `x`.
2. **`let y = ...`** calculates the value `12` and also records how it was produced, as a graph of operations:

   ```text
   x ──► x·x ──► x·x·x ──┐
   │      ▲       ▲      ├──► add ──► y
   └──────┴───────┘      │
   x ──► x·2 ────────────┘
   ```

3. **`y.backward()`** walks the graph from `y` back to `x`, applies the chain rule at each step, and stores the result on `x`.
4. **`x.grad()`** returns that stored gradient.

`x` has shape `[1]`, so its single value needs the index `&[0]`. `backward()` can only start from a tensor holding one value, which is why losses are always reduced to one number.

No symbolic maths is involved. Autograd works from the operations that actually ran, so this also works through Rust `if` statements and loops.

---

## 6. Example 3: Linear regression by hand

Function: `linear_regression`. This example finds the line that best fits some data, using only tensors and autograd, with no `nn` module or optimizer. Every step that `nn` automates later is visible here.

### The data

```rust
let xs = Tensor::rand([200, 1], FLOAT);
let noise = Tensor::randn([200, 1], FLOAT) * 0.05;
let ys = &xs * 3 + 2 + noise;
```

`rand` draws 200 values evenly between 0 and 1. `randn` draws from a normal distribution, and multiplying by `0.05` sets its standard deviation. The result is 200 points close to the line `y = 3x + 2`. The training code never sees the numbers 3 and 2, and has to recover them from the points.

### The parameters

```rust
let mut weight = Tensor::zeros([1], FLOAT).set_requires_grad(true);
let mut bias = Tensor::zeros([1], FLOAT).set_requires_grad(true);
let learning_rate = 0.5;
```

The model is `prediction = weight · x + bias`, and both parameters start at zero. They are `mut` because the training loop changes them in place.

### The training loop

Each of the 300 steps does the following:

```rust
let prediction = &xs * &weight + &bias;
let loss = prediction.mse_loss(&ys, tch::Reduction::Mean);

weight.zero_grad();
bias.zero_grad();
loss.backward();

tch::no_grad(|| {
    weight -= weight.grad() * learning_rate;
    bias -= bias.grad() * learning_rate;
});
```

1. **Predict.** `&xs * &weight` multiplies a `[200, 1]` tensor by a `[1]` tensor. libtorch *broadcasts* the smaller shape, using the same `weight` for all 200 rows.
2. **Measure the error.** `mse_loss` with `Reduction::Mean` is the mean squared error: the average of `(prediction − y)²`.
3. **Clear the old gradients.** `backward()` adds to any gradient already stored, so without `zero_grad` the gradients of all earlier steps would pile up.
4. **Calculate the new gradients.** `loss.backward()` fills in `weight.grad()` and `bias.grad()`.
5. **Update the parameters.** Each moves against its gradient, scaled by the learning rate.

The update is inside `tch::no_grad` for two reasons. The update is not part of the model, so it must not be added to the recorded graph, and libtorch refuses in-place changes to a tensor that requires gradients unless recording is off.

### The result

The loss stops changing at about `0.002`. It cannot reach zero because of the added noise, whose variance is `0.05² = 0.0025`, and no straight line can predict random noise. The learned line is `y = 3.000x + 2.006`.

The printed loss belongs to the parameters *before* that step's update.

---

## 7. Example 4: Training a neural network

Function: `train_classifier`, using `SpiralNet`, `make_spirals` and `accuracy`. This is what real PyTorch code looks like: a network defined as a module, an optimizer that handles the updates, mini-batches, and separate test data.

### The task: two spirals

`make_spirals` generates 2D points that belong to one of two classes. The network gets a point `(x, y)` and has to say which spiral it is on. A straight line can't separate the two spirals, so linear regression could not solve this.

```rust
let radius = Tensor::linspace(0.1, 1.0, n, float);
let angle = &radius * 3.0 * std::f64::consts::TAU;
let spiral = |offset: f64| {
    let theta = &angle + offset;
    Tensor::stack(&[&radius * theta.cos(), &radius * theta.sin()], 1)
};

let clean = Tensor::cat(&[spiral(0.0), spiral(std::f64::consts::PI)], 0);
let points = clean + Tensor::randn([2 * n, 2], float) * 0.02;
```

- **`linspace`** gives `n` evenly spaced radii from 0.1 to 1.0.
- **`angle`** grows with the radius, which is what makes a spiral rather than a circle. `TAU` is one full turn (2π), so the arm winds through 0.3 to 3 turns.
- **The `spiral` closure** converts polar to Cartesian coordinates with `x = r·cos θ` and `y = r·sin θ`. `Tensor::stack(..., 1)` joins the two `[n]` tensors into one of shape `[n, 2]`.
- **`spiral(PI)`** rotates the second arm by half a turn, so it runs between the turns of the first.
- **`Tensor::cat(..., 0)`** puts the two arms one after the other, giving shape `[2n, 2]`.
- **`randn(...) * 0.02`** adds a little noise.

`stack` creates a new dimension, whereas `cat` joins along an existing one.

The labels are `n` zeros followed by `n` ones, with kind `Int64` because they are class numbers:

```rust
let labels = Tensor::cat(
    &[
        Tensor::zeros([n], (Kind::Int64, device)),
        Tensor::ones([n], (Kind::Int64, device)),
    ],
    0,
);
```

### Splitting into training and test data

```rust
let (points, labels) = make_spirals(1000, device);
let total = points.size()[0];
let order = Tensor::randperm(total, (Kind::Int64, device));
let (points, labels) = (
    points.index_select(0, &order),
    labels.index_select(0, &order),
);
let train_size = total * 8 / 10;
let test_size = total - train_size;
```

The test data only measures the result and is never used for training. A model can score well on its training data just by memorizing it, so only points it hasn't seen show whether it has really learned the pattern.

The data is ordered by class, so taking the last 20% unshuffled would give a test set containing only class 1. `randperm` creates a random ordering of the row numbers, and `index_select(0, &order)` applies the same ordering to points and labels, keeping each point with its label.

`narrow(dim, start, length)` then takes the first 1600 rows for training and the last 400 for testing. It returns a view of the same data rather than a copy.

### The network

```rust
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
```

A linear layer calculates `output = input · weight + bias`, the same formula as example 3 but with matrices. The first two numbers passed to `nn::linear` are its input and output sizes.

Data flows through the network like this, where `N` is the number of points in the batch:

```text
[N, 2] ──► hidden1 ──► ReLU ──► [N, 64] ──► hidden2 ──► ReLU ──► [N, 64] ──► output ──► [N, 2]
 points                                                                              one score
                                                                                     per class
```

These are all the network's parameters, read from the saved file:

| Variable | Shape | Number of values |
| --- | --- | --- |
| `hidden1.weight` | `[64, 2]` | 128 |
| `hidden1.bias` | `[64]` | 64 |
| `hidden2.weight` | `[64, 64]` | 4096 |
| `hidden2.bias` | `[64]` | 64 |
| `output.weight` | `[2, 64]` | 128 |
| `output.bias` | `[2]` | 2 |
| **Total** | | **4482** |

`vs / "hidden1"` creates a named sub-path in the `VarStore`, which is how these variables get their names. The default config gives weights Kaiming uniform random values (a standard choice for ReLU networks) and biases random values between `±1/√(input size)`. Random starting values matter: if all neurons started out identical, they would all receive identical updates and stay identical.

### The forward pass

```rust
impl Module for SpiralNet {
    fn forward(&self, xs: &Tensor) -> Tensor {
        xs.apply(&self.hidden1)
            .relu()
            .apply(&self.hidden2)
            .relu()
            .apply(&self.output)
    }
}
```

`xs.apply(&layer)` is the same as `layer.forward(&xs)`, but reads better in a chain.

**`relu()`** replaces negative values with zero. This bend is what makes the network able to learn curves. Stacking linear layers with nothing in between would only produce another linear function, which couldn't separate the spirals.

The final output has no `relu`, because its values are *logits*: raw scores that can be any size, where a higher score means the model favours that class.

`SpiralNet` implements `Module`, not `ModuleT`. `ModuleT` is the variant for networks containing layers such as dropout or batch normalization, which behave differently during training and evaluation.

### `VarStore` and the optimizer

```rust
let vs = nn::VarStore::new(device);
let model = SpiralNet::new(&vs.root());
let mut optimizer = nn::Adam::default().build(&vs, 5e-3)?;
```

The `VarStore` is the main structural difference from Python. Instead of a module tracking its own parameters as `model.parameters()` does, all variables are created in a store, on the store's device. The store then serves two purposes: the optimizer updates its variables, and `save` and `load` read and write them.

**Adam** is a refinement of the gradient descent in example 3. It adapts the step size for each parameter, based on the recent history of that parameter's gradients. `5e-3` is the learning rate, and the defaults are `beta1 = 0.9`, `beta2 = 0.999`, no weight decay and `eps = 1e-8`.

### The training loop

```rust
for epoch in 1..=200 {
    let mut batches = Iter2::new(&train_x, &train_y, 64);
    let mut total_loss = 0.0;
    let mut batch_count = 0.0;

    for (batch_x, batch_y) in batches.to_device(device).shuffle() {
        let loss = model.forward(&batch_x).cross_entropy_for_logits(&batch_y);
        optimizer.backward_step(&loss);
        total_loss += loss.double_value(&[]);
        batch_count += 1.0;
    }
    // ...
}
```

An *epoch* is one pass through the training data. Instead of one update per pass, the data is cut into *mini-batches* of 64 points, with an update after each. With 1600 training points, that gives 25 batches per epoch and 5000 updates in total. Each update is based on a noisier estimate of the gradient, but the many more updates more than make up for that.

**`Iter2`** walks through two tensors (points and labels) in step:

- `shuffle` changes which points are grouped into a batch on each epoch, so the model doesn't see them in a fixed order. It reorders the iterator's own copy, leaving `train_x` and `train_y` unchanged.
- `to_device` must come **before** `shuffle`. `shuffle` creates its random ordering on the iterator's current device, which starts as the CPU. On a GPU, `shuffle().to_device(device)` would mix a CPU index with GPU data and panic.
- `Iter2` skips a final batch smaller than the batch size, unless you call `return_smaller_last_batch()`. Here 1600 divides evenly by 64, so nothing is skipped.

**`cross_entropy_for_logits`** is the standard loss for classification. It converts the logits into log-probabilities with `log_softmax` and takes the negative log-probability of the correct class, averaged over the batch. If the model is confident and right the loss is near zero, and if it's confident and wrong the loss is large.

**`optimizer.backward_step(&loss)`** does three things in one call:

```rust
self.opt.zero_grad();   // the zero_grad calls from example 3
loss.backward();        // the backward call from example 3
self.opt.step();        // the no_grad update from example 3, using Adam
```

`total_loss` and `batch_count` exist only to print the average loss. `double_value` extracts a plain `f64`, so summing the values doesn't keep the autograd graph of each batch alive.

### Measuring accuracy

```rust
fn accuracy(model: &impl Module, points: &Tensor, labels: &Tensor) -> f64 {
    tch::no_grad(|| model.forward(points).accuracy_for_logits(labels)).double_value(&[])
}
```

`accuracy_for_logits` picks the class with the highest score for each point, using `argmax`. It compares that with the correct labels and averages the matches into a value between 0 and 1. `no_grad` skips the graph recording, which evaluation doesn't need.

`&impl Module` means the function accepts any model type, not just `SpiralNet`.

### Why accuracy reaches 100%

Before training, accuracy is about 50%, which is what guessing gives with two classes. After training, it reaches 100%. This shows that training works, but says little about how the network would handle harder data, because this task is easy:

- The arms grow by 1/3 in radius per turn, so neighbouring arms are about `1/6 ≈ 0.17` apart.
- The noise has a standard deviation of only `0.02`, so points almost never stray across the gap.
- Test points come from the same two spirals as the training points.

Real data rarely has such a clean separation. Raising the noise from `0.02` to `0.1` is enough to drop test accuracy to about 59% (see [section 12](#12-ideas-for-experiments)).

---

## 8. Example 5: Saving and loading

This part is at the end of `train_classifier`.

```rust
let path = std::env::temp_dir().join("spiral_net.safetensors");
vs.save(&path)?;

let mut restored_vs = nn::VarStore::new(device);
let restored = SpiralNet::new(&restored_vs.root());
// ... accuracy of the random model ...
restored_vs.load(&path)?;
// ... accuracy of the loaded model ...
```

**Saving** writes each named variable in the store to the file. The file extension selects the format, and `.safetensors` is a simple format that can be read by other tools, including Python. The file is about 18 KB, which matches 4482 parameters of 4 bytes each plus a small header. It goes in the system's temporary directory (`/tmp` on Linux) so that it doesn't clutter the project, and each run overwrites it.

**Only the numbers are saved, not the network's structure.** That's why a new `SpiralNet` must be built first. The new store then holds variables with the same names but random values, and accuracy is around 50% again.

**Loading** goes through each variable in the store, finds the entry with the same name in the file, and copies the values in without recording them for autograd. If a variable is missing from the file, `load` returns an error. `restored_vs` must be `mut` because `load` takes `&mut self`.

The `restored` model doesn't need to be touched by `load`. Its layers hold handles to the variables in the store, so the new values take effect straight away. That's why accuracy jumps to 100%, with no training on the new model.

---

## 9. Expected output

This is the output on a CPU with libtorch 2.13.0. With the same seed and setup, repeated runs were identical, but the numbers could differ with a different libtorch version or on a GPU.

```text
Running on: Cpu

=== 1. Tensors ===
a has shape [2, 3]:
 1  2  3
 4  5  6
[ CPUFloatType{2,3} ]
a + b:
 11  12  13
 14  15  16
[ CPUFloatType{2,3} ]
a * aᵀ:
 14  32
 32  77
[ CPUFloatType{2,2} ]
Mean of a: 3.5
As a Vec: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]

=== 2. Autograd ===
y(2)  = 12
dy/dx = 14 (expected 14)

=== 3. Linear regression by hand ===
step 100  loss 0.00198
step 200  loss 0.00198
step 300  loss 0.00198
Learned y = 3.000x + 2.006 (true line: y = 3x + 2)

=== 4. Training a neural network ===
Accuracy before training: 49.3%
epoch  50  loss 0.0445  test accuracy 100.0%
epoch 100  loss 0.0035  test accuracy 100.0%
epoch 150  loss 0.0010  test accuracy 100.0%
epoch 200  loss 0.0008  test accuracy 100.0%

=== 5. Saving and loading ===
Saved weights to /tmp/spiral_net.safetensors
New model, random weights: 46.5%
New model, loaded weights: 100.0%
```

---

## 10. Errors and panics

`tch` reports errors in two ways:

- **Most tensor methods panic.** `a.matmul(&b)` with incompatible shapes stops the program, with libtorch's error message. Each of these methods has a `Result` version with an `f_` prefix, such as `a.f_matmul(&b)`.
- **Operations involving the outside world return a `Result`.** In this example, those are `build`, `save` and `load`. `main` passes their `TchError` along with `?`.

This example uses the panicking versions, because a shape mismatch here would be a bug in the code rather than a condition to recover from. A long-running service that handles tensors from user input should use the `f_` versions.

---

## 11. Known limitations

- **The GPU path is untested.** The code was written to work with CUDA but has only been run on a CPU.
- **Examples 1 to 3 always run on the CPU**, because `FLOAT` fixes the device. Only examples 4 and 5 use `device`.
- **The test set is used to monitor training.** That's harmless here because no decisions depend on it. If you tune settings against it, you should hold back a third, separate data set for the final measurement.
- **Nothing checks the results automatically.** The project has no tests, so you check the output by eye against [section 9](#9-expected-output).
- **Accuracy is measured on all test points at once.** That's fine for 400 points. A large data set would need to be evaluated in batches to fit in memory.

---

## 12. Ideas for experiments

Each of these needs only a small change and shows something about how training behaves. The results come from one run of each change with the seed of 42. All the other settings were left unchanged.

| Change | Where | Result |
| --- | --- | --- |
| Remove both `.relu()` calls | `SpiralNet::forward` | The network becomes linear. Loss stays at 0.68 and test accuracy ends at 56.7% |
| Raise the noise from `0.02` to `0.1` | `make_spirals` | The arms overlap, and test accuracy falls to 59.2% |
| Raise the noise from `0.02` to `0.2` | `make_spirals` | The spirals are almost lost in the noise, and test accuracy falls to 52.5% |
| Use 100 points per class instead of 1000 | `train_classifier` | Loss keeps falling (0.35) but test accuracy stops at 60%. Only 2 full batches fit per epoch, so there are also far fewer updates |
| Set the learning rate to `2.5` | `linear_regression` | The steps overshoot, and by step 100 the loss and both parameters are `NaN` |
| Remove both `zero_grad` calls | `linear_regression` | Gradients pile up and the fit never settles. Loss jumps between 0.49 and 7.76, and the final line is `y = 2.060x + 0.507` |
| Change the hidden layer size from `64` to `4` | `SpiralNet::new` (all three layers) | Too little capacity to follow the curves, and test accuracy ends at 53% |
| Replace `nn::Adam::default()` with `nn::sgd(0.9, 0.0, 0.0, false)` | `train_classifier` | Plain SGD with momentum learns much more slowly at this learning rate, reaching 69% after 200 epochs |
