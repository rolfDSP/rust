# Code Documentation

This document explains how the example in `src/main.rs` works, how the `rust_bert` crate is built, and where to read more. It assumes you know some Rust, but no machine learning. For a quick start, see [README.md](README.md).

## Contents

1. [The big picture](#1-the-big-picture)
2. [Core concepts](#2-core-concepts)
3. [How rust_bert works inside](#3-how-rust_bert-works-inside)
4. [Project setup: `Cargo.toml`](#4-project-setup-cargotoml)
5. [Startup: `main`](#5-startup-main)
6. [Example 1: Sentiment analysis](#6-example-1-sentiment-analysis)
7. [Example 2: Question answering](#7-example-2-question-answering)
8. [Example 3: Semantic similarity](#8-example-3-semantic-similarity)
9. [Expected output](#9-expected-output)
10. [Customizing a pipeline](#10-customizing-a-pipeline)
11. [Errors and pitfalls](#11-errors-and-pitfalls)
12. [Known limitations](#12-known-limitations)
13. [Ideas for experiments](#13-ideas-for-experiments)
14. [Further reading](#14-further-reading)

---

## 1. The big picture

`rust_bert` is a Rust port of the most used parts of Hugging Face's Python library [Transformers](https://huggingface.co/docs/transformers). It gives you ready-made **pipelines**: objects that take plain text and return an answer such as a label, an answer span, a translation or a vector. Like the [`tch` example in r_proj_065](../r_proj_065/DOCUMENTATION.md), it runs on libtorch, the C++ core of PyTorch:

```text
 Python                               This project
 ──────                               ────────────
 Your Python code                     src/main.rs
        │                                  │
 transformers.pipeline(...)           rust_bert::pipelines        ← ready-made tasks
        │                                  │
 transformers models (BERT, ...)      rust_bert::models            ← same architectures, rewritten in Rust
        │                                  │
 tokenizers                           rust_tokenizers              ← text → token ids
        │                                  │
 torch (Python API)                   tch → torch-sys (FFI)
        │                                  │
        └────────────► libtorch ◄──────────┘
                    (C++ and BLAS/CUDA)
```

What this means in practice:

- **No Python at run time.** The network architectures are written in Rust on top of `tch`. Only the *weights* (the learned numbers) come from Python models, converted once into a format libtorch can read.
- **The models are not in the binary.** They are downloaded from the [Hugging Face Hub](https://huggingface.co/models) the first time a pipeline is created, and cached on disk.
- **The heavy work is in libtorch.** Rust code sets up the calculation; the matrix multiplications run in precompiled C++. A release build is therefore barely faster than a debug build.

The program is one file with one function per example:

| Function | Purpose |
| --- | --- |
| `main` | Runs the three examples in order |
| `sentiment_analysis` | Example 1: is a sentence positive or negative? |
| `question_answering` | Example 2: find the answer to a question inside a text |
| `semantic_similarity` | Example 3: rank sentences by meaning against a query |
| `cosine` | Helper: cosine similarity of two vectors |

---

## 2. Core concepts

### Transformers and BERT

A **Transformer** is a neural network architecture from 2017 ("Attention Is All You Need"). Its key part, **self-attention**, lets every word in a sentence look at every other word to work out its meaning in context: "bank" in "river bank" ends up represented differently from "bank" in "bank account".

**BERT** (2018) is a Transformer that was first trained on huge amounts of text to fill in hidden words. This *pre-training* teaches it a general understanding of language. It is then *fine-tuned* on a small labelled dataset for a specific task. The three models in this example are all built this way:

| Example | Model | Size | Fine-tuned on |
| --- | --- | --- | --- |
| Sentiment | `distilbert-base-uncased-finetuned-sst-2-english` | 256 MB | SST-2: movie review sentences labelled positive/negative |
| Question answering | `distilbert-base-cased-distilled-squad` | 249 MB | SQuAD: 100,000 questions about Wikipedia paragraphs |
| Embeddings | `sentence-transformers/all-MiniLM-L6-v2` | 87 MB | over 1 billion sentence pairs, to place similar sentences close together |

**DistilBERT** is a smaller BERT (6 layers instead of 12) trained to imitate the full model. It keeps about 97% of BERT's accuracy while being 40% smaller and 60% faster. **MiniLM** is another compressed BERT-style model.

### Tokens

Models don't read characters; they read **token ids**. A tokenizer splits text into pieces from a fixed vocabulary (about 30,000 entries for BERT, stored in `vocab.txt`) and turns each piece into a number. Rare words are split into sub-word pieces:

```text
"borrow checker"  →  ["borrow", "check", "##er"]  →  [ 17781, 4638, 2121 ]
```

BERT also adds special tokens: `[CLS]` at the start and `[SEP]` between and after segments. The tokenizer must match the model exactly, which is why every pipeline downloads a vocabulary file together with the weights. `rust_bert` uses the [`rust_tokenizers`](https://github.com/guillaume-be/rust-tokenizers) crate, written by the same author.

"Uncased" models (like the sentiment model) lower-case everything first; "cased" models (like the question-answering model) keep capitals, which helps with names.

### Pipelines

A pipeline bundles everything one task needs: tokenizer, model, and the code before and after the model. Every pipeline in `rust_bert::pipelines` follows the same pattern:

```rust
let model = XxxModel::new(XxxConfig::default())?;   // load (and maybe download)
let output = model.predict(&inputs);                 // run
```

Creating a model is slow (reading hundreds of megabytes into memory); `predict` is comparatively fast. Create a model once and reuse it.

### Resources and the cache

Every file a pipeline needs is a **resource**, an object implementing the `ResourceProvider` trait:

- `RemoteResource`: a URL on the Hugging Face Hub. On first use it is downloaded into a cache folder; afterwards the cached copy is used.
- `LocalResource`: a file path on your disk.
- `BufferResource`: bytes already in memory.

The cache folder is `~/.cache/.rustbert`, or the folder set in the environment variable `RUSTBERT_CACHE`. After running this example it looks like this:

```text
~/.cache/.rustbert/
├── distilbert-sst2/     config/  model/  vocab/
├── distilbert-qa/       config/  model/  vocab/
└── all-mini-lm-l6-v2/   config/  model/  vocab/  sbert-config/  sbert-pooling-config/  tokenizer-config/
```

Each file is stored under a hash of its URL, next to a `.meta` file (URL, ETag) and a `.lock` file that keeps two programs from downloading the same file at once.

### The `.ot` weight format

The model files are called `rust_model.ot`. This is libtorch's own serialization format, which `tch` can load directly into a `VarStore` (the collection of all named weight tensors of a model). Hugging Face hosts these files next to the usual PyTorch files for many popular models. For other models you convert them yourself (see [section 10](#10-customizing-a-pipeline)).

### Device

A pipeline runs on a `tch::Device`: `Device::Cpu` or `Device::Cuda(n)`. The default configs use `Device::cuda_if_available()`. The `download-libtorch` feature fetches the CPU build of libtorch, so this example always runs on the CPU.

---

## 3. How rust_bert works inside

### Crate layout

```text
rust_bert
├── pipelines/      task level: sentiment, ner, question_answering, translation, summarization,
│                   text_generation, zero_shot_classification, sentence_embeddings, keywords_extraction, ...
├── models/         architecture level: bert, distilbert, roberta, deberta, albert, electra, bart, t5,
│                   gpt2, gpt_neo, gpt_j, marian, m2m_100, nllb, pegasus, longformer, ... (25 families)
├── resources/      RemoteResource, LocalResource, BufferResource
└── common          shared layers (activations, dropout, embeddings, linear), errors (RustBertError)
```

The `models` modules are faithful rewrites of the Python implementations: the same layers with the same weight names, so a converted PyTorch checkpoint fits straight in. On top of each base model sit *heads* for different tasks, for example `DistilBertModelClassifier` (sentence classification) or `DistilBertForQuestionAnswering` (answer span).

Many pipelines are generic over the architecture. `SequenceClassificationModel` can run on BERT, DistilBERT, RoBERTa, DeBERTa, ALBERT and others; the config field `model_type: ModelType` picks which one. `SentimentModel` is a thin wrapper around `SequenceClassificationModel` with a two-class model.

### What happens in `SentimentModel::new(Default::default())`

1. **Config.** `SentimentConfig::default()` names the model type (`ModelType::DistilBert`) and three remote resources: weights, `config.json` (layer count, hidden size, label names) and `vocab.txt`. It also sets `lower_case: true` and the device.
2. **Resolve resources.** Each resource's `get_local_path()` returns a file path, downloading the file into the cache first if needed (via the `cached-path` crate).
3. **Build the tokenizer** from the vocabulary file.
4. **Build the network.** A `VarStore` is created on the device, and the Rust model struct (here a DistilBERT classifier) is constructed inside it. This allocates every weight tensor with the right shape and name, still filled with random values.
5. **Load the weights.** `VarStore::load` reads `rust_model.ot` and copies each stored tensor into the matching named variable. If a name or shape doesn't match, loading fails with an error.

### What happens in `predict`

1. **Tokenize** every input text to token ids, adding special tokens and truncating to the model's maximum length.
2. **Pad** all sequences in the batch to the same length, and build an **attention mask** (1 for real tokens, 0 for padding) so the model ignores the padding.
3. **Forward pass** inside `tch::no_grad`, with the model in evaluation mode. This skips the bookkeeping needed for training (autograd), saving memory and time.
4. **Post-process** the raw outputs (called *logits*) into Rust values: for classification, a softmax turns logits into probabilities and the highest one wins; for question answering, the best start/end pair is found and mapped back to the original text.

So a pipeline does in Rust exactly what `transformers.pipeline("sentiment-analysis")` does in Python.

### Optional backends

- **ONNX** (feature `onnx`): instead of libtorch, models can run on ONNX Runtime through the `ort` crate. `ModelResource::ONNX` replaces `ModelResource::Torch`. See [r_proj_066](../r_proj_066) for `ort` on its own.
- **Hugging Face tokenizers** (feature `hf-tokenizers`): use the `tokenizers` crate and a `tokenizer.json` file instead of `rust_tokenizers`, for models whose tokenizer `rust_tokenizers` doesn't support.

---

## 4. Project setup: `Cargo.toml`

```toml
[dependencies]
anyhow = "1.0.104"
rust-bert = { version = "0.23", default-features = false, features = ["remote", "rustls-tls", "download-libtorch"] }
```

| Feature | Why |
| --- | --- |
| `remote` | Enables `RemoteResource` and downloading from the Hugging Face Hub. Without it you must provide every file locally. |
| `rustls-tls` | HTTPS in pure Rust. The default `default-tls` uses OpenSSL and needs its development headers (`libssl-dev`), which this system doesn't have. |
| `download-libtorch` | Passed on to `tch`: downloads a matching libtorch (2.4.0 for `tch` 0.17) during the build, so you don't have to install it. |

`anyhow` gives a single error type, so `?` works for `RustBertError` without extra code.

**The `console` pin.** The download progress bar comes from the old `indicatif` 0.16 crate (through `cached-path`). `indicatif` 0.16 accepts any `console` version below 1.0, but doesn't compile with `console` 0.16. `Cargo.lock` therefore pins `console` to 0.15.11. If a `cargo update` breaks the build with `unresolved import console::Term`, run:

```bash
cargo update -p console --precise 0.15.11
```

**Versions.** `rust-bert` 0.23.0 is the newest release. It depends on `tch` 0.17, whereas r_proj_065 uses `tch` 0.26. The two can't share a libtorch, which is why each project downloads its own.

---

## 5. Startup: `main`

```rust
fn main() -> Result<()> {
    sentiment_analysis()?;
    question_answering()?;
    semantic_similarity()?;
    Ok(())
}
```

`Result` is `anyhow::Result`. Each example creates its own model, uses it and drops it at the end of the function, so only one model is in memory at a time. Peak memory use of the whole program is about 670 MB.

---

## 6. Example 1: Sentiment analysis

```rust
let model = SentimentModel::new(Default::default())?;
```

`Default::default()` is `SentimentConfig::default()`: DistilBERT fine-tuned on SST-2, as described in [section 3](#what-happens-in-sentimentmodelnewdefaultdefault). The first call downloads 256 MB.

```rust
let texts = [ "Rust's borrow checker ...", "The build took forever ...", "This crate is okay, ..." ];

for (text, s) in texts.iter().zip(model.predict(texts)) {
```

`predict` accepts anything that is `AsRef<[&str]>`: an array, a `Vec<&str>` or a slice. All texts are processed together as one **batch**, which is faster than one call per text. The result is a `Vec<Sentiment>` in the same order as the inputs, so `zip` pairs each text with its result.

```rust
pub struct Sentiment {
    pub polarity: SentimentPolarity,  // Positive or Negative
    pub score: f64,                   // probability of that polarity, 0.5 to 1.0
}
```

Inside, `SentimentModel::predict` calls the general `SequenceClassificationModel::predict`, which applies a softmax over the two classes and picks the larger. Class id 1 becomes `Positive`, id 0 `Negative`. The `score` is the probability of the chosen class, so it is never below 0.5.

**There is no "neutral".** SST-2 only has two labels, so the model *must* choose. "This crate is okay, nothing special." comes out as NEGATIVE with 0.993. A high score means "the model is sure *within its two options*", not "this is clearly negative". If you need neutral, use a three-class model (see [section 13](#13-ideas-for-experiments)).

---

## 7. Example 2: Question answering

```rust
let model = QuestionAnsweringModel::new(Default::default())?;
```

The default `QuestionAnsweringConfig` loads DistilBERT fine-tuned on SQuAD. This is **extractive** question answering: the model doesn't write an answer, it points to the span of the context that contains it. If the answer isn't in the context, it still returns the best-looking span.

```rust
let inputs: Vec<QaInput> = questions
    .iter()
    .map(|q| QaInput { question: q.to_string(), context: context.to_string() })
    .collect();
```

Each `QaInput` holds one question and its context as owned `String`s. Here all three questions share the same context, but every input could have a different one.

```rust
let answers = model.predict(&inputs, 1, 8);
```

| Argument | Meaning |
| --- | --- |
| `&inputs` | The questions with their contexts |
| `1` (`top_k`) | How many candidate answers to return per question |
| `8` (`batch_size`) | How many inputs to run through the model at once |

How it works:

1. Question and context are tokenized together: `[CLS] question [SEP] context [SEP]`.
2. For every context token, the model outputs two scores: how likely the answer *starts* here, and how likely it *ends* here.
3. The pipeline searches for the best valid start/end pair (start before end, at most 15 tokens long) and converts the token positions back to character positions in the original text.

A long context is cut into overlapping windows of at most 384 tokens (`max_seq_length`), overlapping by 128 tokens (`doc_stride`), and the best answer over all windows is kept. Both values can be changed in `QuestionAnsweringConfig`.

The result is `Vec<Vec<Answer>>`: one inner vector per question, with up to `top_k` answers, best first:

```rust
pub struct Answer {
    pub score: f64,     // confidence
    pub start: usize,   // character position of the answer in the context
    pub end: usize,
    pub answer: String, // the answer text
}
```

`a[0]` is safe here because `top_k` is 1 and the context is not empty.

**Reading the scores.** "Who" and "When" questions have short, clear answers (a name, a date), and the model is very sure: 0.996 and 0.981. "How does Rust achieve memory safety?" gets the right span, "by using a borrow checker", but only 0.281. "How" questions have fuzzier boundaries (should "without a garbage collector" be included?), so the probability is spread over several overlapping spans.

A detail you can see in the download log: the vocabulary for this model is downloaded from `bert-large-cased`. DistilBERT-cased reuses BERT's cased vocabulary, and `rust_bert` points both to the same file.

---

## 8. Example 3: Semantic similarity

```rust
let model = SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL6V2)
    .create_model()?;
```

Sentence embeddings use a **builder** instead of a config struct, because a Sentence-Transformers model is made of several modules described by its own config files. `remote(...)` picks a predefined model from the Hub; `SentenceEmbeddingsBuilder::local(path)` loads one from a folder instead.

The `all-MiniLM-L6-v2` model has three modules (from its `modules.json`):

```text
text ─► Transformer (6-layer MiniLM, BERT architecture) ─► one 384-number vector per token
     ─► Pooling (mean over all tokens)                  ─► one 384-number vector per sentence
     ─► Normalize (scale to length 1)                    ─► the embedding
```

The result is an **embedding**: a point in a 384-dimensional space, where sentences with similar meanings lie close together. Inputs longer than 256 tokens are truncated.

```rust
let query_emb = &model.encode(&[query])?[0];
let doc_embs = model.encode(&documents)?;
```

`encode` returns `Vec<Embedding>`, where `Embedding` is `Vec<f32>`, one per input. The query is encoded as a one-element batch, the documents as a batch of four. (`encode_as_tensor` returns a `SentenceEmbeddingsModelOutput` holding a `tch::Tensor` instead, which is useful when you want to keep calculating with libtorch.)

```rust
let mut ranked: Vec<(f32, &str)> = doc_embs.iter().zip(documents)
    .map(|(e, d)| (cosine(query_emb, e), d))
    .collect();
ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
```

Each document gets its cosine similarity to the query, and the list is sorted best first. `f32` has no total order (because of `NaN`), so `sort_by` with `total_cmp` is used instead of `sort`. Swapping `a` and `b` sorts in descending order.

```rust
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}
```

**Cosine similarity** measures the angle between two vectors: 1 means same direction (same meaning), 0 means unrelated. Because this model already normalizes its embeddings to length 1, `na` and `nb` are 1, and the dot product alone would give the same result. The full formula is kept so the function also works for models without a Normalize module.

This is the basis of **semantic search** and of retrieval-augmented generation (RAG): embed all documents once, store the vectors, then embed each query and look for the nearest vectors.

**The surprising ranking.** The Tokio sentence (0.434) beats the correct `Result`/`?` sentence (0.399). A small model like MiniLM leans heavily on shared words: "Rust" appears in both the query and the Tokio sentence, but not in the `Result` sentence. The ranking at the extremes is still sensible: the weather sentence is far away at 0.092. Larger models such as `AllMiniLmL12V2` (12 layers instead of 6) or `AllDistilrobertaV1` can do better, at the cost of size and speed.

---

## 9. Expected output

First run (downloads lines shortened):

```text
=== 1. Sentiment analysis ===
Downloading https://huggingface.co/distilbert-base-uncased-finetuned-sst-2-english/resolve/main/rust_model.ot [255.44MiB]... ✓ Done!
POSITIVE (0.999)  Rust's borrow checker saved me from a nasty bug today. Love it!
NEGATIVE (1.000)  The build took forever and then failed with a cryptic linker error.
NEGATIVE (0.993)  This crate is okay, nothing special.

=== 2. Question answering ===
Downloading ...
Q: Who designed Rust?
A: Graydon Hoare  (score 0.996)
Q: When was Rust 1.0 released?
A: May 2015  (score 0.981)
Q: How does Rust achieve memory safety?
A: by using a borrow checker  (score 0.281)

=== 3. Semantic similarity (sentence embeddings) ===
Downloading ...
Query: How do I handle errors in Rust?
  0.434  Tokio is an asynchronous runtime for Rust.
  0.399  Use the Result type and the ? operator to propagate failures.
  0.253  panic! aborts the current thread when something unrecoverable happens.
  0.092  The weather in Berlin is sunny today.
```

| | Time | Notes |
| --- | --- | --- |
| First build | about 2.5 minutes | includes downloading libtorch |
| First run | about 2 minutes | almost all of it is downloading 600 MB of models |
| Later runs | about 5 seconds | mostly loading the weights from disk; peak memory about 670 MB |

Measured on a 32-thread machine under WSL2. The numbers are deterministic: the models are only used for inference, with no randomness involved, so every run prints the same scores.

To start the binary without `cargo run`, tell it where libtorch is, as in r_proj_065:

```bash
export LD_LIBRARY_PATH=$(dirname "$(find target/debug/build -name libtorch_cpu.so | head -1)")
./target/debug/r_proj_068
```

---

## 10. Customizing a pipeline

### Another predefined model

`rust_bert` ships constants for the models it hosts on the Hub, grouped by architecture: `DistilBertModelResources`, `BertModelResources`, `RobertaModelResources` and so on, with matching `...ConfigResources` and `...VocabResources`. A config built from them:

```rust
use rust_bert::distilbert::{DistilBertConfigResources, DistilBertModelResources, DistilBertVocabResources};
use rust_bert::pipelines::common::{ModelResource, ModelType};
use rust_bert::pipelines::sentiment::{SentimentConfig, SentimentModel};
use rust_bert::resources::RemoteResource;

let config = SentimentConfig {
    model_type: ModelType::DistilBert,
    model_resource: ModelResource::Torch(Box::new(RemoteResource::from_pretrained(
        DistilBertModelResources::DISTIL_BERT_SST2,
    ))),
    config_resource: Box::new(RemoteResource::from_pretrained(DistilBertConfigResources::DISTIL_BERT_SST2)),
    vocab_resource: Box::new(RemoteResource::from_pretrained(DistilBertVocabResources::DISTIL_BERT_SST2)),
    ..Default::default()
};
let model = SentimentModel::new(config)?;
```

The `..Default::default()` fills in the remaining fields (lower-casing, device and so on).

### Any model from the Hub, with your own files

For a model `rust_bert` doesn't list, download its files and convert the PyTorch weights to `.ot` with the script that comes with the crate (needs Python with PyTorch):

```bash
python utils/convert_model.py path/to/pytorch_model.bin    # or model.safetensors
```

The script is in the [rust-bert repository](https://github.com/guillaume-be/rust-bert/tree/main/utils). Then point the config to the files with `LocalResource`:

```rust
use rust_bert::resources::LocalResource;
use std::path::PathBuf;

let weights = LocalResource { local_path: PathBuf::from("models/my-model/rust_model.ot") };
```

The model's architecture must be one of the families in `rust_bert::models`, and `model_type` must match it.

### GPU and precision

Set `device: Device::Cuda(0)` in the config, and build against a CUDA libtorch: with `download-libtorch` on Linux or Windows, set `TORCH_CUDA_VERSION` (for example `cu124`) before building. The `kind` field can load weights in half precision (`Some(Kind::Half)`) to save memory on the GPU.

### Using a pipeline from async code or several threads

`predict` blocks the thread and keeps CPU cores busy. In a web server (like the axum example) don't call it directly inside an async handler. The [async-sentiment example](https://github.com/guillaume-be/rust-bert/blob/main/examples/async-sentiment.rs) in the crate shows the usual pattern: the model lives in its own thread, and handlers send texts to it through a channel and wait for the reply. Alternatively, use `tokio::task::spawn_blocking`.

---

## 11. Errors and pitfalls

| Symptom | Cause | Fix |
| --- | --- | --- |
| `unresolved import console::Term` while building `indicatif` | `console` 0.16 in `Cargo.lock` | `cargo update -p console --precise 0.15.11` |
| `failed to run custom build command for openssl-sys` | The default `default-tls` feature needs OpenSSL headers | Use `rustls-tls` as in this project, or install `libssl-dev` |
| `libtorch_cpu.so: cannot open shared object file` | Binary started without `cargo run` | Set `LD_LIBRARY_PATH` (section 9) |
| Download errors on first run | No internet, or the Hub is not reachable | Retry; or copy a filled `~/.cache/.rustbert` from another machine; or use `LocalResource` |
| A download was interrupted | `cached-path` writes into a temporary file and only renames it when complete, so the cache stays consistent | Rerun; delete the model's cache folder if loading still fails |
| `RustBertError::TchError` while creating the model | Weights don't fit the architecture (`model_type` wrong, or a file from a different model) | Check `model_type` and that config, vocabulary and weights belong to the same model |
| Mixing `tch` versions | `rust-bert` 0.23 needs `tch` 0.17 | Don't add a different `tch` version to the same project; if you need `tch` directly, add `tch = "0.17"` |

All pipeline constructors return `Result<_, RustBertError>`; `predict` methods mostly return the values directly, while `encode` returns a `Result`. With `anyhow` all of them work with `?`.

---

## 12. Known limitations

- **The crate is maintained slowly.** The latest release, 0.23.0, is from 2024 and pins libtorch 2.4.0 through `tch` 0.17. Newer architectures (Llama, Mistral, Qwen and other modern LLMs) are not included. For those, look at [Candle](https://github.com/huggingface/candle) (section 14).
- **Old dependencies** such as `indicatif` 0.16 and `reqwest` 0.11 cause problems like the `console` pin.
- **Model size.** Models are hundreds of MB to several GB; the default NER model (BERT large) is 1.3 GB, the default zero-shot model (BART large) 1.6 GB.
- **CPU speed.** Transformers are designed for GPUs. On a CPU, small models like these are fine; text generation or summarization with large models is slow.
- **No training API.** The pipelines are for inference. You can fine-tune with `tch` directly, but `rust_bert` gives no help with it; fine-tuning is usually done in Python, then converted.
- **English only** for the three models used here. For other languages, use a multilingual model (for example `SentenceEmbeddingsModelType::DistiluseBaseMultilingualCased`).

---

## 13. Ideas for experiments

1. **Make the semantic search fair.** Remove "Rust" from the Tokio sentence, or add it to the `Result` sentence, and see how the ranking changes. Then try `SentenceEmbeddingsModelType::AllMiniLmL12V2` or `AllDistilrobertaV1`.
2. **A question without an answer.** Ask "What is the capital of France?" against the Rust context. The model still returns a span; check how low the score gets, and think about a threshold for "no answer".
3. **`top_k` answers.** Set `top_k` to 3 for the "How" question and look at the alternative spans and their scores.
4. **Named entities.** Add a `NERModel` (`rust_bert::pipelines::ner`) and extract persons, organizations and places from the Rust context. Note the 1.3 GB download.
5. **Zero-shot classification.** With `ZeroShotClassificationModel` you can classify texts into labels you invent at run time, for example "bug report", "feature request", "question". This also solves the "no neutral" problem from example 1.
6. **Summarization or translation.** Try `SummarizationModel` or `TranslationModelBuilder` (Marian models, for example English → German).
7. **Batch speed.** Run sentiment analysis on 1,000 sentences, once with one `predict` call and once with 1,000 calls, and compare the time. Then try a release build.
8. **Keyword extraction.** `KeywordExtractionModel` combines sentence embeddings with candidate words to find the keywords of a text.

The crate's [examples folder](https://github.com/guillaume-be/rust-bert/tree/main/examples) has a working program for each pipeline.

---

## 14. Further reading

### rust-bert

- [rust-bert on GitHub](https://github.com/guillaume-be/rust-bert): source, README with a list of all pipelines and supported models, and benchmarks against Python
- [API documentation on docs.rs](https://docs.rs/rust-bert/0.23.0/rust_bert/): the crate-level page explains setup, resources and every pipeline with examples
- [rust-bert on crates.io](https://crates.io/crates/rust-bert): versions and features
- [Examples folder](https://github.com/guillaume-be/rust-bert/tree/main/examples): one runnable program per pipeline, including ONNX variants
- [Model conversion script](https://github.com/guillaume-be/rust-bert/blob/main/utils/convert_model.py)
- [rust-tokenizers](https://github.com/guillaume-be/rust-tokenizers): the tokenizer library underneath

### The underlying libraries

- [tch-rs](https://github.com/LaurentMazare/tch-rs) and its [documentation](https://docs.rs/tch/0.17.0/tch/): the PyTorch bindings `rust_bert` builds on
- [PyTorch C++ (libtorch) documentation](https://pytorch.org/cppdocs/)
- [Hugging Face Transformers documentation](https://huggingface.co/docs/transformers/index): the Python library rust-bert follows; its [pipelines page](https://huggingface.co/docs/transformers/main_classes/pipelines) maps closely to `rust_bert::pipelines`
- [Sentence Transformers documentation](https://www.sbert.net/): background on embeddings, pooling and semantic search

### The models used here

- [distilbert-base-uncased-finetuned-sst-2-english](https://huggingface.co/distilbert/distilbert-base-uncased-finetuned-sst-2-english)
- [distilbert-base-cased-distilled-squad](https://huggingface.co/distilbert/distilbert-base-cased-distilled-squad)
- [sentence-transformers/all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)

### Background and papers

- [The Illustrated Transformer](https://jalammar.github.io/illustrated-transformer/) and [The Illustrated BERT](https://jalammar.github.io/illustrated-bert/) by Jay Alammar: the best visual introductions
- [Hugging Face LLM Course](https://huggingface.co/learn/llm-course/chapter1/1): free course on Transformers, tokenizers and fine-tuning (in Python, but the concepts carry over)
- Vaswani et al., 2017: [Attention Is All You Need](https://arxiv.org/abs/1706.03762), the Transformer
- Devlin et al., 2018: [BERT: Pre-training of Deep Bidirectional Transformers](https://arxiv.org/abs/1810.04805)
- Sanh et al., 2019: [DistilBERT, a distilled version of BERT](https://arxiv.org/abs/1910.01108)
- Wang et al., 2020: [MiniLM: Deep Self-Attention Distillation](https://arxiv.org/abs/2002.10957)
- Reimers & Gurevych, 2019: [Sentence-BERT](https://arxiv.org/abs/1908.10084), sentence embeddings
- Rajpurkar et al., 2016: [SQuAD: 100,000+ Questions for Machine Comprehension of Text](https://arxiv.org/abs/1606.05250)
- Socher et al., 2013: [Recursive Deep Models for Semantic Compositionality Over a Sentiment Treebank](https://nlp.stanford.edu/sentiment/), the SST dataset

### Alternatives in Rust

- [Candle](https://github.com/huggingface/candle): Hugging Face's own ML framework in pure Rust, no libtorch, actively developed, supports modern LLMs
- [ort](https://github.com/pykeio/ort): ONNX Runtime bindings (used in r_proj_066)
- [Burn](https://github.com/tracel-ai/burn): a deep-learning framework written in Rust, with training support
- [tokenizers](https://github.com/huggingface/tokenizers): Hugging Face's tokenizer library, which is written in Rust
