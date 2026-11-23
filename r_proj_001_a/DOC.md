# CUDA from Rust

## Does Rust support NVIDIA CUDA?

Not natively. `rustc` has no CUDA backend, so you can't write `#[kernel] fn foo()`
and have it compile straight to GPU code the way `nvcc` does for C++. There are
two practical ways to use CUDA from Rust today:

1. **Bindings to the CUDA driver/runtime API** — write your GPU kernel in CUDA C
   (or load a precompiled `.ptx`/`.cubin`), compile it to PTX, and call into the
   CUDA driver from safe-ish Rust to allocate memory, copy data, and launch the
   kernel. This works with stable Rust and today's CUDA toolkits. Crates:
   [`cudarc`](https://crates.io/crates/cudarc) (actively maintained) or the
   older `rustacuda`/`cust`.
2. **Compiling Rust itself to GPU code** — the
   [Rust-CUDA project](https://github.com/Rust-GPU/Rust-CUDA)'s
   `rustc_codegen_nvvm` backend compiles a subset of Rust to NVVM IR/PTX, so you
   write the kernel in Rust. It requires a pinned nightly toolchain and specific
   LLVM/CUDA versions, and support for brand-new GPU architectures lags behind.

This example uses approach 1 (`cudarc`) because it works on stable Rust and
against whatever CUDA toolkit is installed, including this machine's very new
GPU.

## This machine

- GPU: **NVIDIA GB10** (Grace Blackwell), compute capability **12.1**, driver `580.178.04`
- CUDA toolkit: **13.0** (`nvcc`)
- Rust: stable, `edition = "2024"`

Compute capability 12.1 is new enough that a prebuilt `.ptx`/`.cubin` compiled
elsewhere might not target it correctly. To sidestep that, the example compiles
the kernel **at runtime** using NVRTC (NVIDIA's runtime compiler), which always
targets the GPU/toolkit actually present on the machine it runs on.

## What's in this repo

- `Cargo.toml` — added dependencies:
  - [`cudarc`](https://crates.io/crates/cudarc) with features `driver`, `nvrtc`,
    and `cuda-version-from-build-system` (lets `cudarc`'s build script detect the
    installed CUDA version via `nvcc` instead of hardcoding one).
  - `anyhow` for convenient `?`-based error handling in the example binary.
- `examples/cuda_vector_add.rs` — a self-contained example: GPU vector addition
  (`c[i] = a[i] + b[i]` for ~1,000,000 elements).

## How the example works

1. **Open the GPU and a command stream**

   ```rust
   let ctx = CudaContext::new(0)?;        // open device 0
   let stream = ctx.default_stream();     // a queue of GPU work
   ```

2. **Write the kernel as a CUDA C string and compile it at runtime (NVRTC)**

   ```rust
   const KERNEL_SRC: &str = r#"
   extern "C" __global__ void vector_add(const float *a, const float *b, float *c, int n) {
       int i = blockIdx.x * blockDim.x + threadIdx.x;
       if (i < n) { c[i] = a[i] + b[i]; }
   }
   "#;
   let ptx = compile_ptx(KERNEL_SRC)?;          // CUDA C -> PTX, via NVRTC
   let module = ctx.load_module(ptx)?;          // PTX -> loaded GPU module
   let vector_add = module.load_function("vector_add")?;
   ```

   `extern "C"` avoids C++ name mangling so `load_function("vector_add")` can
   find it by that exact name.

3. **Move data to the GPU**

   ```rust
   let a_dev = stream.clone_htod(&a_host)?;      // host -> device copy
   let b_dev = stream.clone_htod(&b_host)?;
   let mut c_dev = stream.alloc_zeros::<f32>(n)?; // device-only output buffer
   ```

4. **Launch the kernel with one GPU thread per array element**

   ```rust
   let cfg = LaunchConfig::for_num_elems(n as u32); // picks grid/block size for n threads
   let mut launch_args = stream.launch_builder(&vector_add);
   launch_args.arg(&a_dev);
   launch_args.arg(&b_dev);
   launch_args.arg(&mut c_dev);
   launch_args.arg(&n_i32);
   unsafe { launch_args.launch(cfg) }?;
   ```

   The `launch` call is `unsafe` because the compiler can't verify the argument
   types/order match the kernel signature, or that the kernel itself is memory-safe
   — that's on the author of the CUDA C source.

5. **Copy the result back and check it**

   ```rust
   let c_host = stream.clone_dtoh(&c_dev)?;
   ```

## Running it

```sh
cargo run --release --example cuda_vector_add
```

Expected output (values will match, exact formatting may vary):

```
Using GPU: NVIDIA GB10
c[0]       = 0
c[n/2]     = 1572864
c[n-1]     = 3145725
Vector addition of 1048576 elements verified on the GPU.
```

## Requirements to reproduce elsewhere

- An NVIDIA GPU and driver.
- CUDA toolkit installed such that `nvcc` is on `PATH` (used by `cudarc`'s build
  script to detect the CUDA version) and the CUDA runtime/driver shared
  libraries (`libcuda.so`, `libnvrtc.so`, etc.) are discoverable at run time
  (e.g. via `/usr/local/cuda/lib64` on the loader path).
- No GPU present, or want to skip the detection step? Pin the CUDA version
  explicitly instead of using `cuda-version-from-build-system`, e.g.:
  ```toml
  cudarc = { version = "0.19.10", features = ["driver", "nvrtc", "cuda-13000"] }
  ```