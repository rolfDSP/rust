# rust-bert / candle on this machine — what was done and how to reuse it

## 1. Hardware/environment context

- OS: Ubuntu 24.04.5 LTS, `aarch64`
- GPU: NVIDIA GB10 (Grace Blackwell), driver reports CUDA 13.0, `nvcc` 13.0 installed
- Rust: 1.100.0 / Cargo 1.100.0

This combination (very new aarch64 server-class Blackwell chip) is the reason the investigation below was necessary: most prebuilt ML native libraries target `x86_64` first, and aarch64+CUDA+Blackwell support is only present in the newest releases.

## 2. Why `rust-bert` can't use the GPU here

`rust-bert` depends on the `tch` crate (Rust bindings for libtorch, PyTorch's C++ library). `tch` is **tightly version-pinned** to a specific libtorch release per `tch` version — not a range.

Checked on crates.io / GitHub:
- `rust-bert` 0.23.0 (latest release) → depends on `tch = "0.17.0"`
- `rust-bert` git `main` (unreleased) → **still** depends on `tch = "0.17.0"`
- `tch` 0.17.0 → requires **libtorch 2.4.0** exactly (hardcoded `TORCH_VERSION` in `torch-sys`'s `build.rs`)

PyTorch 2.4.0 (released mid-2024):
- Has no official aarch64 **CUDA** wheel (aarch64 CUDA/"SBSA" wheels for server GPUs like GH200/GB200/GB10 only started appearing around PyTorch 2.5–2.6)
- Predates Blackwell GPU kernel support entirely, so even if a build existed, it would have no compiled kernels for this GPU's compute capability

**Conclusion:** as of today, the published `rust-bert` crate cannot drive this GPU. Its only realistic option on this machine is CPU-only inference via libtorch 2.4.0 (CPU build).

This was raised to the user via `AskUserQuestion`; the decision made was: **use `candle` instead**, a pure-Rust ML framework with no libtorch/ABI version pinning, which does support this GPU.

## 3. What was set up

### 3a. A general-purpose CUDA-enabled libtorch (for any future `tch`-based work)

Even though `rust-bert` itself can't use it (due to the 0.17.0/2.4.0 pin above), a modern CUDA-enabled libtorch was installed system-wide in case a future project uses `tch` directly at a newer version (`tch` 0.27.0, which targets libtorch **2.15.0** and does support Blackwell/aarch64/CUDA 13):

```bash
python3 -m venv ~/.local/share/pytorch-venv
source ~/.local/share/pytorch-venv/bin/activate
pip install torch==2.15.0   # resolves to torch-2.15.0+cu132 on aarch64, CUDA 13 support, confirmed detects the GB10
```

Verified with:
```python
import torch
torch.__version__          # '2.15.0+cu132'
torch.cuda.is_available()  # True
torch.cuda.get_device_name(0)  # 'NVIDIA GB10'
```

Made available to **all** Rust projects by exporting env vars in `~/.bashrc` (no per-project config needed — `tch`'s build script picks up `LIBTORCH`/`LD_LIBRARY_PATH` automatically):

```bash
# rust-bert / tch-rs: shared libtorch (PyTorch C++ lib) for all Rust projects
export LIBTORCH="$HOME/.local/share/pytorch-venv/lib/python3.12/site-packages/torch"
export LD_LIBRARY_PATH="$LIBTORCH/lib:$LD_LIBRARY_PATH"
```

Why point `LIBTORCH` at a *pip-installed* torch package directory instead of the official standalone `libtorch` zip: PyTorch only publishes standalone `libtorch-*.zip` archives for `x86_64`/Windows/macOS. For aarch64, the pip wheel is the only distribution channel, and its internal layout (`torch/lib/*.so`, `torch/include/*`) matches what `tch`'s build script expects from a `LIBTORCH` directory. Since this directory has no `build-version` file (that's specific to the official zip), `tch`'s version check is silently skipped — this is safe here because the installed version (2.15.0) is intentionally the exact version the latest `tch` (0.27.0) expects.

**This venv is CPU+CUDA capable but currently only useful to projects depending on `tch` ≥ 0.27 / libtorch 2.15.x — not to `rust-bert` 0.23.0, which needs the older 2.4.0 ABI.** See §5 if you want to actually run `rust-bert` here.

### 3b. A working candle + BERT example on GPU (this project, `r_proj_085`)

`Cargo.toml`:
```toml
[dependencies]
candle-core = { version = "0.11.1", features = ["cuda"] }
candle-nn = { version = "0.11.1", features = ["cuda"] }
candle-transformers = { version = "0.11.1", features = ["cuda"] }
hf-hub = { version = "1.0.0", features = ["blocking"] }
tokenizers = "0.23"
anyhow = "1"
serde_json = "1"
```

`src/hub.rs` — a small blocking Hugging Face Hub client. Written from scratch because `hf-hub` 1.0 completely replaced the old `api::sync::Api` interface with a new `HFClientSync` / builder-based API (`.download_file().filename(..).maybe_revision(..).send()`). It mirrors the pattern used internally by candle's own (unpublished) `candle-examples` helper crate.

`src/main.rs` — adapted from the official `candle/candle-examples/examples/bert/main.rs`, trimmed down (no CLI args, no tracing/mkl/accelerate features) and hardcoded to prefer `Device::new_cuda(0)` with a CPU fallback. It:
1. Downloads `sentence-transformers/all-MiniLM-L6-v2` (config, tokenizer, safetensors weights) from the HF Hub, cached under `~/.cache/huggingface` after the first run.
2. Loads the model via `candle_transformers::models::bert::BertModel`.
3. Tokenizes 8 example sentences, batches them, runs a forward pass on the GPU.
4. Mean-pools token embeddings (masking out padding) and L2-normalizes them.
5. Computes pairwise cosine similarity and prints the top 5 most similar sentence pairs.

Verified output (run via `cargo run --release`):
```
Running on GPU: Cuda(CudaDevice(DeviceId(1)))
running inference on batch [8, 8]
generated embeddings [8, 8, 384]
pooled embeddings [8, 384]

Top 5 most similar sentence pairs:
score: 0.89 'The new movie is awesome' 'The new movie is so great'
score: 0.68 'The cat sits outside' 'The cat plays in the garden'
score: 0.51 'I love pasta' 'Do you like pizza?'
...
```

Key library versions resolved: `candle-core`/`candle-nn`/`candle-transformers` 0.11.1, `cudarc` 0.19.10, `hf-hub` 1.0.0, `tokenizers` 0.23.2.

## 4. How future projects can use `candle` (recommended path on this machine)

For any new BERT/transformer-style Rust+GPU project here:

1. Copy the dependency block from §3b's `Cargo.toml` into the new project (bump versions if newer ones exist by then — check `cargo info candle-core`).
2. Reuse `src/hub.rs` as-is (copy the file) for downloading HF Hub models — it needs no changes per-model.
3. Pick a model implementation from `candle_transformers::models::*` (the crate bundles dozens: `bert`, `llama`, `mistral`, `whisper`, `stable_diffusion`, `clip`, etc. — check `candle-transformers`'s `src/models/` directory on GitHub for the full list and that model's expected config/weight filenames).
4. Device selection pattern to reuse:
   ```rust
   let device = match candle_core::Device::new_cuda(0) {
       Ok(d) => d,
       Err(_) => candle_core::Device::Cpu,
   };
   ```
5. Build with `cargo build --release` — the `cuda` feature triggers `cudarc` to compile CUDA kernels via `nvcc` (already installed, confirmed working) on first build; expect ~1–2 min extra compile time the first time, cached afterward.
6. No environment variables are required for candle (unlike `tch`/libtorch) — it links directly against the CUDA toolkit found via `nvcc`/`CUDA_HOME`.

**Why candle over `tch`/`rust-bert` on this machine:** no native-library version pinning problem, pure Rust + CUDA kernels compiled per-project, and it's already confirmed working on this exact GPU/arch. This is the default recommendation going forward.

## 5. How future projects could still use `rust-bert` (if ever actually needed)

Only consider this if a project specifically needs a model/pipeline that `candle-transformers` doesn't implement but `rust-bert` does (e.g. some of its ready-made pipelines like NER, summarization, translation, conversation).

Two real options, in order of how much risk you're willing to take:

**A. CPU-only (reliable, no GPU).**
Install libtorch 2.4.0 CPU build (the exact version `rust-bert` 0.23.0 / `tch` 0.17.0 expects) into its own venv, separate from the `pytorch-venv` described in §3a (that one is 2.15.0, wrong version for this):
```bash
python3 -m venv ~/.local/share/pytorch-venv-cpu-2.4
~/.local/share/pytorch-venv-cpu-2.4/bin/pip install torch==2.4.0
```
Then point a specific project's build at it (don't put this in `~/.bashrc` — it would conflict with the CUDA 2.15.0 `LIBTORCH` already configured globally):
```bash
LIBTORCH=~/.local/share/pytorch-venv-cpu-2.4/lib/python3.12/site-packages/torch \
LD_LIBRARY_PATH=~/.local/share/pytorch-venv-cpu-2.4/lib/python3.12/site-packages/torch/lib \
cargo build --release
```
Add `rust-bert = "0.23"` to that project's `Cargo.toml` (it pulls in the matching `tch` automatically). This will work, just without GPU acceleration.

**B. Attempt GPU by forcing a newer `tch`.** Not attempted here — flagged by the user as experimental/risky and declined in favor of candle. Would require `[patch.crates-io]` overriding `rust-bert`'s `tch` dependency to 0.27.0 and very likely patching `rust-bert`'s own source, since its internals call `tch` 0.17 APIs that may have changed across 10 releases. No guarantee of success; only worth attempting if a `candle` equivalent genuinely doesn't exist for the needed model/pipeline.

## 6. Quick reference

| Goal | Crate | GPU on this machine? | Notes |
|---|---|---|---|
| New transformer/BERT work | `candle` (`candle-core`/`-nn`/`-transformers`) | Yes | Recommended default; see §4 |
| Must use `rust-bert` specifically | `rust-bert` + `tch` 0.17 + libtorch 2.4.0 | No (CPU only) | See §5A |
| Raw `tch` access to a recent PyTorch | `tch` ≥ 0.27 + libtorch 2.15.0 | Yes | Already installed globally, see §3a |