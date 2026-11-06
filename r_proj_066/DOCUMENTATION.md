# Documentation

This project trains a small neural network in PyTorch, exports it to the ONNX format, and runs it from Rust. The sections below cover what you need installed, how to use the project, how the code works, and what to do when something goes wrong.

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick start](#quick-start)
- [How to use it](#how-to-use-it)
- [How the code works](#how-the-code-works)
- [Troubleshooting](#troubleshooting)
- [Limits of this example](#limits-of-this-example)

## Overview

The project has two separate programs, and an ONNX file connects them:

```text
python/train_export.py  --->  model/spiral.onnx  --->  src/main.rs
PyTorch: train, export        graph + weights          ONNX Runtime: inference
                              model/test_vectors.json
                              PyTorch's own answers
```

| Path | What it is | Needed to run the Rust program? |
| --- | --- | --- |
| `src/main.rs` | The Rust program: loads the model and runs it | Yes |
| `model/spiral.onnx` | The trained network | Yes |
| `model/test_vectors.json` | 400 test points with PyTorch's outputs | Yes, for the comparison in step 2 |
| `python/train_export.py` | Trains the network and writes both model files | No, only to retrain |
| `.venv/` | Python environment with PyTorch (not in git) | No, only to retrain |

The network is a classifier. It takes a point `(x, y)` and decides which of two interleaved spirals it belongs to, the same task as in `r_proj_065`. A straight line can't separate the spirals, so the network has to learn a curved boundary.

**ONNX** is a file format that describes a network as a graph of standard operations together with the trained weights. Any ONNX runtime can execute such a file, so the program that trains a model and the program that uses it don't have to share a language or a framework. This model contains only two kinds of operation: `Gemm` (a linear layer) and `Relu`.

**Why not `tch`, as in `r_proj_065`?** They solve different problems:

| | `tch` (`r_proj_065`) | ONNX + `ort` (this project) |
| --- | --- | --- |
| Train in Rust | Yes | No |
| Run a trained model | Yes, but the Rust code has to rebuild the network or load TorchScript | Yes, the file holds the whole network |
| Added to the build | libtorch, over 1 GB in `target` | ONNX Runtime, linked into the binary |
| Export to ONNX | Not supported | Done in Python with `torch.onnx.export` |

Use `tch` to train in Rust, and ONNX to deploy a model that is already trained.

## Prerequisites

Everything here was tested on Ubuntu 24.04 (x86-64) under WSL2. Versions marked "minimum" were not tested themselves; they are the lowest versions the dependencies declare.

### To build and run the Rust program

| Requirement | Version | Notes |
| --- | --- | --- |
| Operating system | Linux x86-64 | Tested platform, see [other platforms](#other-platforms) |
| Rust and Cargo | Minimum 1.92, tested with 1.99 | `ort` declares 1.92. Install with [rustup](https://rustup.rs/) |
| C and C++ build tools | Tested with GCC 13.3 | The `ring` crate compiles C code, and ONNX Runtime links against `libstdc++` |
| Internet connection | First build only | Downloads crates from crates.io and ONNX Runtime from `cdn.pyke.io` |
| Disk space | About 450 MB | 343 MB for `target` in a debug build, 107 MB for the ONNX Runtime cache in `~/.cache/ort.pyke.io` |

On Ubuntu or Debian, this installs the build tools:

```bash
sudo apt install build-essential
```

You can check what you have with:

```bash
rustc --version   # 1.92 or newer
cc --version
c++ --version
```

You **don't** need Python, PyTorch, libtorch, a GPU, the OpenSSL headers, or `LD_LIBRARY_PATH`. The trained model is already in `model/`.

### To retrain the model (optional)

| Requirement | Version | Notes |
| --- | --- | --- |
| Python | Minimum 3.11, tested with 3.12.4 | PyTorch 2.15 declares 3.11 |
| `venv` module | | On Ubuntu's system Python, install it with `sudo apt install python3-venv` |
| `torch` | Tested with 2.15.0 (CPU) | Trains the model and runs the exporter |
| `onnx` | Tested with 1.23.2 | Validates the exported file |
| `onnxscript` | Tested with 0.7.2 | The exporter needs it |
| Disk space | About 1.2 GB | For `.venv` |

The commands to install them are under [Retraining the model](#retraining-the-model). No GPU is needed: training takes about 5 seconds on a 32-thread CPU.

### To run a built binary on another machine

The ONNX Runtime code is inside the binary, but the binary still needs the system's C and C++ libraries. Built on Ubuntu 24.04, it requires **glibc 2.38** and **`libstdc++` with `GLIBCXX_3.4.31`** or newer, so it won't start on older systems such as Ubuntu 22.04 (glibc 2.35). It also needs the model files, see [Moving the binary](#moving-the-binary).

### Other platforms

`ort` offers prebuilt ONNX Runtime binaries for other targets, including Linux on ARM64, macOS on Apple Silicon, and Windows on x64. The code has nothing Linux-specific in it, but this project has not been tested there, and the feature list in `Cargo.toml` may need adjusting.

## Quick start

```bash
cd r_proj_066
cargo run
```

A clean debug build took about 10 seconds with the crates and ONNX Runtime already downloaded. The first build also has to download them.

You should see this output. The accuracy and the probabilities apply to the model in this repository, and the difference from PyTorch may vary slightly between machines:

```text
=== 1. The loaded model ===
Input:  points Tensor { ty: Float32, shape: [-1, 2], dimension_symbols: SymbolicDimensions(["batch", ""]) }
Output: logits Tensor { ty: Float32, shape: [-1, 2], dimension_symbols: SymbolicDimensions(["batch", ""]) }

=== 2. Same answers as PyTorch? ===
Largest difference from PyTorch over 400 points: 3.81e-6
Test accuracy: 96.5%

=== 3. Classifying new points ===
[0.5, 0.0] -> class 1 (55.5% sure)
[-0.3, 0.2] -> class 1 (72.3% sure)
[0.0, -0.8] -> class 1 (66.1% sure)

=== 4. What the network learned ===
#############################################################
#############################################################
#############################################################
########################.......##############################
##################.......................####################
#############..........###....##............#################
############.....######################......################
##########......########.............###......###############
#########.....######.........###......#########..############
########.....#####......##########......######....###########
#######.....####.....######.#########.....####.....##########
#####......#####...######.....########.....###......#########
####.......####...######......#######......###....#####......
###.......#####....#####.......#####....###......######......
..........#####.....#####...............####.....#####.......
.........######......#######...........####.....######.......
...........######.....#######........###.......#####.........
.............######..........####....###......#####..........
................#######..............#..#########............
..................##########.......###########...............
......................#####################..................
..........................######.###.........................
.............................................................
.............................................................
.............................................................
```

How to read it:

1. **The loaded model:** `points` and `logits` are the names set in Python. In the shape, `-1` means "any batch size" and `2` means two values per point.
2. **Same answers as PyTorch?** This is the most important line. A difference around `1e-6` means the export is faithful. If it exceeds `1e-4`, the program stops with an error.
3. **Classifying new points:** the predicted class of each point and the model's probability for it.
4. **What the network learned:** `.` is class 0 and `#` is class 1, drawn for `x` and `y` from -1.1 to 1.1.

## How to use it

### Classifying your own points

Edit the array in `classify_points` in `src/main.rs`, then run `cargo run` again:

```rust
let points = [[0.1, 0.1], [0.9, -0.2]];
```

```text
[0.1, 0.1] -> class 1 (98.3% sure)
[0.9, -0.2] -> class 0 (74.8% sure)
```

The training data lies between about -1 and 1 on both axes, so answers for points far outside that range mean little.

### Retraining the model

Create the Python environment once:

```bash
python3 -m venv .venv
.venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu
.venv/bin/pip install onnx onnxscript
```

The first `pip` line uses PyTorch's own package index to get the CPU build, which avoids downloading the much larger CUDA libraries.

Then train and export, which overwrites both files in `model/`:

```bash
.venv/bin/python python/train_export.py
```

Expected output, apart from some exporter progress messages:

```text
epoch 100  loss 0.5641
epoch 200  loss 0.4466
epoch 300  loss 0.3138
epoch 400  loss 0.1955
PyTorch test accuracy: 96.5%
Wrote spiral.onnx (25869 bytes)
Wrote test_vectors.json (400 points)
```

The warnings saying that `torchvision` is not installed are harmless. The seed is fixed, and running the script twice on this machine produced byte-identical files.

Finally, run `cargo run`. You don't have to recompile for a new model, because the program reads the files when it starts.

### Changing the network or the training

Edit `python/train_export.py`, retrain, and run the Rust program again. The Rust code doesn't need to change as long as the model still takes two numbers per point and returns two logits.

For example, this makes the network twice as wide and trains it twice as long:

```python
self.layers = nn.Sequential(
    nn.Linear(2, 128),
    nn.ReLU(),
    nn.Linear(128, 128),
    nn.ReLU(),
    nn.Linear(128, 2),
)
```

```python
for epoch in range(1, 801):
```

With this change the test accuracy reached 100.0%, and the Rust program picked up the new model without being rebuilt (its largest difference from PyTorch was `8.58e-6`).

### Using your own model

1. In Python, export with `input_names` and `output_names`, and mark every dimension that can vary in `dynamic_shapes`. Remember `model.eval()`, and pass `external_data=False` to `save`.
2. In `predict` in `src/main.rs`, change the shape, element type and names to match. `describe_model` prints what the model expects.
3. Repeat in Rust any preprocessing you do in Python, such as normalizing or tokenizing. It is not part of the graph unless it happens inside the model's `forward`.
4. Write new test vectors from PyTorch, and keep the comparison.

### Building for release

```bash
cargo build --release
./target/release/r_proj_066
```

Unlike in `r_proj_065`, the binary starts directly, without `LD_LIBRARY_PATH`. It is about 30 MB, because it contains ONNX Runtime.

### Moving the binary

The paths to the model files are fixed at compile time from the project's location:

```rust
const MODEL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/model/spiral.onnx");
```

So you can start the program from any directory, but it only works while the project stays where it was built. A real deployment would take the path from a command-line argument or a configuration file, or embed the model with `include_bytes!`.

## How the code works

### Training and export: `python/train_export.py`

**The network** has two hidden layers of 64 neurons. It outputs two numbers, one per class:

```python
self.layers = nn.Sequential(
    nn.Linear(2, 64),
    nn.ReLU(),
    nn.Linear(64, 64),
    nn.ReLU(),
    nn.Linear(64, 2),
)
```

**The data** comes from `make_spirals`: 1000 points per class, with a little noise added. The second spiral is the first one rotated by half a turn. `main` shuffles the points and holds back 20% for testing, so the 400 test points were never used in training.

**The training** is a standard PyTorch loop: 400 full-batch steps with Adam and `CrossEntropyLoss`.

**The export** is the part that matters for the Rust program:

```python
model.eval()
...
onnx_program = torch.onnx.export(
    model,
    (torch.zeros(1, 2),),
    input_names=["points"],
    output_names=["logits"],
    dynamic_shapes={"points": {0: torch.export.Dim("batch")}},
)
onnx_program.save(ONNX_PATH, external_data=False)
onnx.checker.check_model(onnx.load(ONNX_PATH), full_check=True)
```

- `model.eval()` switches to inference mode. Layers such as dropout and batch norm behave differently during training, and the export records whichever mode is active. This network has neither, but the call belongs before every export.
- `(torch.zeros(1, 2),)` is an example input. The exporter runs the model on it and records the operations, so only its shape matters, not its values.
- `input_names` and `output_names` set the names the Rust code uses. They are the contract between the two programs.
- `dynamic_shapes` marks dimension 0 as free. Without it, the graph would accept only batches of one, the size of the example.
- `external_data=False` keeps the weights inside the `.onnx` file instead of a separate `.onnx.data` file.
- `onnx.checker` validates the file, which catches a broken export before the Rust program sees it.

**The test vectors** are the last step: the script writes the test points, their labels and PyTorch's outputs to `model/test_vectors.json`.

### Dependencies: `Cargo.toml`

```toml
ort = { version = "=2.0.0-rc.15", default-features = false, features = ["std", "download-binaries", "copy-dylibs", "api-31", "tls-rustls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`ort` wraps ONNX Runtime, and `serde` and `serde_json` read the test vectors. Three details of the `ort` line are deliberate:

- **`=2.0.0-rc.15`:** `ort` 2.0 is a release candidate, and its API changes between candidates. A plain `"2.0.0-rc.15"` would let Cargo move to a later candidate, so the `=` pins this exact version. Examples online often don't compile against it: `Session::builder` now takes an environment, for instance.
- **`tls-rustls`** replaces the default `tls-native`. The build script downloads ONNX Runtime over HTTPS, and `tls-native` would require the OpenSSL headers (`libssl-dev`) on the build machine.
- **`ndarray` and `tracing`** are left out because this program doesn't use them. Tensors go in and out as a shape plus a flat slice.

### The Rust program: `src/main.rs`

The program is one `main` that creates a session and calls four steps. All of them use the same `predict` function.

| Function | What it does |
| --- | --- |
| `main` | Loads the model and runs the four steps |
| `predict` | Runs a batch of points through the model |
| `class_of` | Picks the class with the higher logit |
| `softmax` | Converts logits to probabilities |
| `describe_model` | Step 1: prints the model's inputs and outputs |
| `check_against_pytorch` | Step 2: compares with PyTorch, computes accuracy |
| `classify_points` | Step 3: classifies a few new points |
| `draw_decision_map` | Step 4: draws the decision boundary |

#### Loading the model

```rust
let environment = ort::init().build()?;
let mut session = Session::builder(&environment)?
    .commit_from_file(MODEL_PATH)
```

The environment holds ONNX Runtime's global state. A session is one loaded model, and ONNX Runtime optimizes the graph while loading it. Creating a session is the expensive part, so create it once and reuse it. Running a session takes `&mut self`, which is why the step functions take `&mut Session`.

#### Running the model

`predict` is the core of the program:

```rust
fn predict(session: &mut Session, points: &[[f32; 2]]) -> Result<Vec<[f32; 2]>, Box<dyn Error>> {
    let input = TensorRef::from_array_view(([points.len(), 2], points.as_flattened()))?;
    let outputs = session.run(ort::inputs!["points" => input])?;
    let (_shape, logits) = outputs["logits"].try_extract_tensor::<f32>()?;
    Ok(logits.as_chunks::<2>().0.to_vec())
}
```

1. `as_flattened` views the `&[[f32; 2]]` as a flat `&[f32]`, and `TensorRef` pairs it with the shape `[n, 2]` without copying the data. ONNX tensors are stored row by row, which is how an array of pairs is laid out in memory.
2. `session.run` executes the graph. The input name has to match the one set in Python.
3. `try_extract_tensor::<f32>` borrows the output as a shape and a flat slice. The element type has to match the model's.
4. `as_chunks::<2>` turns the flat slice back into pairs. `to_vec` copies them, because the data belongs to `outputs` and disappears with it at the end of the function.

#### Checking the export

A file that loads is not proof of a correct export. `check_against_pytorch` reads `test_vectors.json` into this struct:

```rust
#[derive(Deserialize)]
struct TestVectors {
    points: Vec<[f32; 2]>,
    labels: Vec<usize>,
    logits: Vec<[f32; 2]>,
}
```

It runs the points through ONNX Runtime and finds the largest difference from PyTorch's outputs across all values. The two runtimes use different numeric kernels, so small rounding differences are normal. Anything above `TOLERANCE` (`1e-4`) stops the program with an error.

Keep a check like this in any real project. The usual causes of a mismatch are a forgotten `model.eval()` and preprocessing that exists in Python but wasn't repeated in Rust.

#### From logits to an answer

The network ends in raw scores called logits, which can be any number. The higher one decides the class:

```rust
fn class_of(logits: [f32; 2]) -> usize {
    usize::from(logits[1] > logits[0])
}
```

To get probabilities, the Rust code has to apply the softmax itself:

```rust
fn softmax(logits: [f32; 2]) -> [f32; 2] {
    let [a, b] = logits.map(f32::exp);
    [a / (a + b), b / (a + b)]
}
```

It isn't in the exported graph, because PyTorch's `CrossEntropyLoss` applies it inside the loss function, and the loss is not part of the model.

#### The decision map

`draw_decision_map` builds a grid of 61 × 25 = 1525 points and sends them through `predict` in one call. This only works because the batch size was left dynamic. It then prints one character per point, row by row.

## Troubleshooting

The run-time errors and the `ModuleNotFoundError` below were reproduced by making the mistake on purpose. The build problems and the `venv` errors weren't reproduced on this machine, so the exact wording may differ.

### Build problems

| Symptom | Cause | Fix |
| --- | --- | --- |
| `linker 'cc' not found`, or `ring` fails to build | No C build tools | `sudo apt install build-essential` |
| `rustc ... is not supported` for `ort` | Rust is older than 1.92 | `rustup update` |
| An error from `ort-sys` about downloading | No connection to `cdn.pyke.io` during the first build, or `cargo` running with `--offline` | Connect to the internet and build once. The download is then cached in `~/.cache/ort.pyke.io` |
| `Session::builder` takes 1 argument, or similar API errors after changing the version | `ort`'s API changes between release candidates | Keep the pinned `=2.0.0-rc.15`, or adapt the code to the new version |

### Run-time errors

| Message | Cause | Fix |
| --- | --- | --- |
| `cannot load .../model/spiral.onnx (File at ... does not exist). Run python/train_export.py first.` | The model file is missing | Retrain, or restore it from version control |
| `Error: Os { code: 2, kind: NotFound, ... }` right after the heading of step 2 | `test_vectors.json` is missing | As above. The program doesn't name the file in this case |
| `outputs differ by more than 0.0001` | `spiral.onnx` and `test_vectors.json` don't come from the same training run, or the export changed the model | Run `train_export.py` so that it writes both files together, and check for `model.eval()` |
| `Got invalid dimensions for input: points ... index: 0 Got: 400 Expected: 1` | The model was exported without `dynamic_shapes`, so the batch size is fixed. Step 1 then shows the shape `[1, 2]` instead of `[-1, 2]` | Add `dynamic_shapes` and retrain |
| `Invalid input name: coords` | The name in `ort::inputs!` differs from the one in the model | Use the name that step 1 prints |
| Panic: ``no output named `scores` `` | The name in `outputs["..."]` differs from the one in the model | Use the name that step 1 prints |

### Python errors

| Message | Cause | Fix |
| --- | --- | --- |
| `ModuleNotFoundError: No module named 'onnx'` or `'torch'` | Running the script with the wrong Python | Use `.venv/bin/python`, not `python3` |
| `No module named venv`, or `ensurepip is not available` | The `venv` module is missing | `sudo apt install python3-venv` |

## Limits of this example

- **The model is deliberately small.** It shows the workflow, and a real model would have more preprocessing and more complex inputs. Accuracy also depends on the random seed: another seed reached 99.3% with the same settings, against 96.5% for the model here.
- **Confidence is not knowledge.** The network assigns a class to every point, including the corners of the map, where it never saw training data. Be careful with inputs that look nothing like the training data.
- **Inference only.** The Rust program can't train or adjust the model. To train in Rust, see `r_proj_065`.
- **CPU only.** ONNX Runtime can also use GPUs through execution providers, but they aren't enabled in this project.
- **The test vectors are not needed in production.** They are here to prove the export is correct. A real program would run that check in its tests rather than on every start.
