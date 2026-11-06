# PyTorch from Rust

An example of using PyTorch from Rust with the [`tch`](https://github.com/LaurentMazare/tch-rs) crate, which wraps libtorch, the C++ library underneath PyTorch. Python is not needed to build or run it.

The program in `src/main.rs` runs five steps:

1. **Tensors:** creating them, arithmetic, and converting back to Rust data
2. **Autograd:** computing a derivative automatically
3. **Gradient descent by hand:** fitting a line using raw tensors
4. **A neural network:** training a classifier on two interleaved spirals with `nn` and Adam
5. **Saving and loading:** storing the weights as a `.safetensors` file and restoring them into a new model

## Running

```bash
cargo run
```

The first build downloads libtorch (CPU version), which needs an internet connection. The `target` directory grows to over 1 GB.

A release build is not noticeably faster for this example (about 2.4 seconds either way on a 32-thread machine), because the heavy work happens inside the precompiled libtorch rather than in Rust code.

For a line-by-line explanation of the code, see [DOCUMENTATION.md](DOCUMENTATION.md).

## Running the binary directly

`cargo run` finds libtorch by itself. Starting the binary directly fails with `libtorch_cpu.so: cannot open shared object file` unless you point it to the library:

```bash
export LD_LIBRARY_PATH=$(dirname "$(find target/debug/build -name libtorch_cpu.so | head -1)")
./target/debug/r_proj_065
```

## From Python to Rust

| Python | Rust (`tch`) |
| --- | --- |
| `torch.tensor([1.0, 2.0])` | `Tensor::from_slice(&[1.0_f32, 2.0])` |
| `a + b` | `&a + &b` (borrow, or the operands are moved) |
| `a @ b` | `a.matmul(&b)` |
| `loss.item()` | `loss.double_value(&[])` |
| `with torch.no_grad():` | `tch::no_grad(\|\| { ... })` |
| `class Net(nn.Module)` | a struct that implements `nn::Module` |
| `model.parameters()` | `nn::VarStore` |
| `optimizer.step()` after `zero_grad()` and `backward()` | `optimizer.backward_step(&loss)` |
| `torch.save(model.state_dict(), path)` | `vs.save(path)` |

Most `Tensor` methods panic on an error such as mismatched shapes. Each one has a variant with an `f_` prefix, such as `f_matmul`, that returns a `Result` instead.

## Using a GPU

The code already calls `Device::cuda_if_available()`, but the downloaded libtorch is CPU-only by default. To download a CUDA build instead, set `TORCH_CUDA_VERSION`, then rebuild from clean:

```bash
export TORCH_CUDA_VERSION=cu128
cargo clean && cargo run
```

The build script of `torch-sys` 0.26 accepts `cu118`, `cu121`, `cu124`, `cu126` and `cu128`. This has not been tested here, as the machine it was written on has no GPU, so PyTorch may not publish every one of these for libtorch 2.13.

## Using an existing libtorch

Remove the `download-libtorch` feature from `Cargo.toml` and set `LIBTORCH` to the directory of your libtorch install. Its version must match what `tch` expects (libtorch 2.13.0 for `tch` 0.26).
