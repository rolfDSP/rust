//! GPU vector addition using CUDA via the `cudarc` crate.
//! Run with: cargo run --release --example cuda_vector_add

use cudarc::driver::{CudaContext, LaunchConfig, PushKernelArg};
use cudarc::nvrtc::compile_ptx;

const KERNEL_SRC: &str = r#"
extern "C" __global__ void vector_add(const float *a, const float *b, float *c, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        c[i] = a[i] + b[i];
    }
}
"#;

fn main() -> anyhow::Result<()> {
    let n: usize = 1 << 20; // ~1M elements

    // Pick GPU 0 and get its default execution stream.
    let ctx = CudaContext::new(0)?;
    let stream = ctx.default_stream();
    println!("Using GPU: {}", ctx.name()?);

    // Compile the CUDA C kernel source to PTX at runtime (via NVRTC) and
    // load it into the device as a usable function.
    let ptx = compile_ptx(KERNEL_SRC)?;
    let module = ctx.load_module(ptx)?;
    let vector_add = module.load_function("vector_add")?;

    // Prepare host-side input data.
    let a_host: Vec<f32> = (0..n).map(|i| i as f32).collect();
    let b_host: Vec<f32> = (0..n).map(|i| 2.0 * i as f32).collect();

    // Copy inputs to device memory and allocate space for the result.
    let a_dev = stream.clone_htod(&a_host)?;
    let b_dev = stream.clone_htod(&b_host)?;
    let mut c_dev = stream.alloc_zeros::<f32>(n)?;

    // Launch one GPU thread per element.
    let cfg = LaunchConfig::for_num_elems(n as u32);
    let n_i32 = n as i32;
    let mut launch_args = stream.launch_builder(&vector_add);
    launch_args.arg(&a_dev);
    launch_args.arg(&b_dev);
    launch_args.arg(&mut c_dev);
    launch_args.arg(&n_i32);
    unsafe { launch_args.launch(cfg) }?;

    // Copy the result back to host memory and verify it.
    let c_host = stream.clone_dtoh(&c_dev)?;
    for i in [0, n / 2, n - 1] {
        let expected = a_host[i] + b_host[i];
        assert_eq!(c_host[i], expected);
    }

    println!("c[0]       = {}", c_host[0]);
    println!("c[n/2]     = {}", c_host[n / 2]);
    println!("c[n-1]     = {}", c_host[n - 1]);
    println!("Vector addition of {n} elements verified on the GPU.");

    Ok(())
}