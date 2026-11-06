# PyTorch to ONNX to Rust

Train a model in PyTorch, export it to the ONNX format, and run it from Rust with [ONNX Runtime](https://onnxruntime.ai/) through the [`ort`](https://ort.pyke.io/) crate. This is the usual way to ship a PyTorch model in a Rust program: the training stays in Python, and the finished program needs neither Python nor PyTorch.

```text
python/train_export.py  --->  model/spiral.onnx  --->  src/main.rs
PyTorch: train, export        graph + weights          ONNX Runtime: inference
```

The model classifies points on two interleaved spirals, the same task as in `r_proj_065`.

The Rust program in `src/main.rs` runs four steps:

1. **Inspecting the model:** input and output names, types and shapes
2. **Checking the export:** comparing the outputs with PyTorch's on 400 held-out points, and failing if they differ
3. **Classifying new points:** turning the outputs into probabilities
4. **Drawing the decision boundary:** running a whole grid of points as one batch

## Running

The trained model is already in `model/`, so this is all you need:

```bash
cargo run
```

You need Rust 1.92 or newer and the C and C++ build tools (`sudo apt install build-essential` on Ubuntu). Python is not needed. The first build downloads ONNX Runtime, which needs an internet connection.

ONNX Runtime is linked statically, so unlike in `r_proj_065` the binary starts without `LD_LIBRARY_PATH`. It reads the model from this project's `model/` directory, using a path set at compile time.

## Training the model again

This is only needed to change the model, and requires Python 3.11 or newer. The CPU build of PyTorch is enough, and the virtual environment takes about 1.2 GB:

```bash
python3 -m venv .venv
.venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu
.venv/bin/pip install onnx onnxscript
.venv/bin/python python/train_export.py
```

The warnings about `torchvision` not being installed are harmless. The script uses a fixed seed, so running it again on the same setup produced identical files. Tested with torch 2.15, onnx 1.23, onnxscript 0.7 and `ort` 2.0.0-rc.15.

For the exact prerequisites, usage, an explanation of the code and troubleshooting, see [DOCUMENTATION.md](DOCUMENTATION.md).
