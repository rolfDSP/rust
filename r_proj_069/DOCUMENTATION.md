# Code Documentation

This document explains how the example in `src/main.rs` works: how the web search becomes training data, how the GPT-2 network is built layer by layer, how the fine-tuning works, and exactly where and how the next token of the generated text is chosen. It assumes you know some Rust, but no machine learning. For a quick start, see [README.md](README.md).

## Contents

1. [The big picture](#1-the-big-picture)
2. [Core concepts](#2-core-concepts)
3. [Project setup: `Cargo.toml`](#3-project-setup-cargotoml)
4. [Step 1: Web search](#4-step-1-web-search)
5. [Step 2: Training data](#5-step-2-training-data)
6. [Step 3: The GPT-2 network, layer by layer](#6-step-3-the-gpt-2-network-layer-by-layer)
7. [Step 4: Fine-tuning](#7-step-4-fine-tuning)
8. [Step 5: Generating text, one token at a time](#8-step-5-generating-text-one-token-at-a-time)
9. [Saving the model](#9-saving-the-model)
10. [Expected output](#10-expected-output)
11. [Errors and pitfalls](#11-errors-and-pitfalls)
12. [Known limitations](#12-known-limitations)
13. [Ideas for experiments](#13-ideas-for-experiments)
14. [Further reading](#14-further-reading)

---

## 1. The big picture

The program takes a pretrained language model from Hugging Face, `distilbert/distilgpt2`, and trains it a little further on text that it has just found on the web. Everything is Rust; there is no Python and no libtorch. The neural network itself is written in this file on top of [candle](https://github.com/huggingface/candle), Hugging Face's tensor library for Rust.

```text
 ┌────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
 │ Web search │──►│ cleaned text │──►│ token ids,   │──►│ GPT-2 (candle),  │──►│ finetuned-model/ │
 │ Wikipedia  │   │ per document │   │ windows of   │   │ pretrained       │   │ safetensors +    │
 │ or Brave   │   │              │   │ 129 tokens   │   │ weights, AdamW   │   │ config+tokenizer │
 └────────────┘   └──────────────┘   └──────────────┘   └──────────────────┘   └──────────────────┘
    reqwest          own code           tokenizers         candle-core/-nn         VarMap::save
```

`main` (`src/main.rs:113`) runs five steps and prints a section for each:

| Step | Functions | What happens |
| --- | --- | --- |
| 1. Web search | `search_backend`, `WikipediaSearch::search`, `BraveSearch::search` | Finds up to 6 pages about the topic and returns their text |
| 2. Training data | `download_model`, `build_windows`, `to_batch` | Downloads the model files, tokenizes the text, cuts it into windows |
| 3. Model | `Gpt2::new`, `VarMap::load`, `evaluate`, `generate` | Builds the network, loads the pretrained weights, measures the "before" state |
| 4. Fine-tuning | `train_model`, `batch_loss` | Trains for 4 epochs and keeps the best weights |
| 5. Comparison | `evaluate`, `generate`, `print_next_token_probabilities`, `save_model` | Shows "after" next to "before" and saves the model |

The two command-line arguments are the search topic and a prompt. The prompt is completed by the model before and after the training, so you can see what changed.

With the option `--no-finetune`, none of the five steps runs. The program loads the model that an earlier run saved in `finetuned-model/` and only completes the prompt (chapter 9.1).

---

## 2. Core concepts

### A language model predicts the next token

A **language model** like GPT-2 does one thing: given a sequence of tokens, it returns a probability for every possible next token. "Rust is a programming language that" → `" is"` 16.2 %, `" has"` 6.5 %, `" allows"` 6.5 %, `" can"` 5.2 %, ... Writing a whole text is just repeating this: pick a token, append it, ask again. Chapter 8 shows the code for this.

### The probability of a whole text: the chain rule

A model that only predicts *one* next token can still assign a probability to a whole text. By the chain rule of probability, the probability of a token sequence `x₁ x₂ … x_T` is the product of the next-token probabilities, each conditioned on everything before it:

```text
  P(x₁ x₂ … x_T)  =  P(x₁) · P(x₂ | x₁) · P(x₃ | x₁ x₂) · … · P(x_T | x₁ … x_{T−1})
```

For the prompt and the first generated token:

```text
  P("Rust is a programming language that has")
     = P("Rust")
     · P(" is"          | "Rust")
     · P(" a"           | "Rust is")
     · P(" programming" | "Rust is a")
     · P(" language"    | "Rust is a programming")
     · P(" that"        | "Rust is a programming language")
     · P(" has"         | "Rust is a programming language that")
```

This factorization is exact, not an approximation, and it is the whole probability model of GPT-2. Each factor is one output of the network, and it ties the three phases of this program together:

- **Training** (chapter 7) makes every factor for the real text as large as possible. The loss is the average of `−log` of these factors, so lowering the loss means raising the probability of the whole training text.
- **Evaluation** measures the same average on the validation text. Perplexity is `exp` of it.
- **Generation** (chapter 8) runs the product forwards: it computes the last factor for every possible token, picks one, appends it, and repeats with a context that is one token longer.

The network never sees a "sentence" as a unit. The previous token sequence influences the result only through the conditioning `| x₁ … x_{t−1}` of each factor. Chapter 8.2 shows how the code computes one such factor.

Everything the model "knows" is in its **weights** (also called parameters): 82 million numbers in matrices. They were set during **pre-training**, in which GPT-2 read about 40 GB of web text and its weights were adjusted, over and over, to give the token that actually came next a higher probability. **Fine-tuning** is the same process continued on a small, specific text, here the search results. It shifts the model's word choice and style towards that text.

### Tokens

The model doesn't see letters or words but **tokens**: pieces of text from a fixed list, the **vocabulary**. GPT-2 uses *byte-level BPE* (byte pair encoding) with 50,257 tokens. Frequent words are one token, rare words are split into several pieces. A leading space belongs to the token, so `" language"` and `"language"` are different tokens:

```text
"Rust is a programming language that"
 → ["Rust", " is", " a", " programming", " language", " that"]
 → [49444, 318, 257, 8300, 3303, 326]
```

Token 50256 is `<|endoftext|>`. The program uses it as a separator between documents and as a stop signal during generation (`EOS_TOKEN`, `src/main.rs:447`).

### Tensors and shapes

A **tensor** is a multi-dimensional array of numbers. Its **shape** says how big each dimension is. In this file the same letters are used throughout:

| Letter | Meaning | Value here |
| --- | --- | --- |
| `b` | batch: how many text windows are processed together | 8 in training, 1 in generation |
| `t` | time: number of tokens in each window | 128 in training, prompt length in generation |
| `c` | channels: size of the vector that represents one token (`n_embd`) | 768 |
| `n_head` | number of attention heads | 12 |
| `head_dim` | `c / n_head`, size of each head | 64 |
| `v` | vocabulary size | 50,257 |

The model's input has shape `(b, t)` (token ids), inside the network every token is a vector, so the data has shape `(b, t, c)`, and the output has shape `(b, t, v)`: for every position a score for every possible next token.

### Logits and softmax

The network doesn't directly output probabilities but **logits**: one unbounded number per vocabulary token. Bigger means more likely. The **softmax** function turns them into probabilities that are positive and add up to 1:

```text
                    exp(logit_i)
  P(token i) = ─────────────────────
                Σ_j  exp(logit_j)
```

Softmax keeps the order, so the token with the highest logit also has the highest probability. Softmax is also used inside the attention layers (chapter 6.6).

### candle, Var and VarMap

[candle](https://github.com/huggingface/candle) provides tensors and the operations on them (matrix multiplication, `exp`, `mean`, ...). It also records which operations produced a result, so it can compute **gradients**: for every weight, how the loss would change if that weight changed a little. This is called automatic differentiation or **backpropagation**.

- A `Var` is a tensor whose gradient candle tracks. All weights of the model are `Var`s.
- A `VarMap` is a map from names like `transformer.h.0.attn.c_attn.weight` to `Var`s. It can load and save all of them in the `safetensors` format.
- A `VarBuilder` hands out `Var`s while the model is built. `vb.pp("ln_1")` ("push prefix") adds a name segment, so every layer gets the same name as in the Hugging Face checkpoint.

---

## 3. Project setup: `Cargo.toml`

| Crate | Used for |
| --- | --- |
| `candle-core` | Tensors, matrix operations, gradients (`Tensor`, `Var`, `Device`) |
| `candle-nn` | Building blocks for training: `VarMap`, `VarBuilder`, `AdamW`, `softmax`, `cross_entropy` |
| `tokenizers` | Hugging Face's tokenizer library (itself written in Rust); reads `tokenizer.json` |
| `hf-hub` | Downloads files from the Hugging Face Hub into the local cache (`blocking` = no async) |
| `reqwest` | HTTP client for the search APIs, with `rustls` (no OpenSSL needed) |
| `serde`, `serde_json` | Parsing the JSON answers of the search APIs and `config.json` |
| `rand` | Shuffling the training windows with a fixed seed |
| `anyhow` | Simple error handling with `?`, `ensure!`, `bail!` and `.context(...)` |

Always run it with `--release`. Unlike with libtorch, the matrix operations here are compiled Rust code; a debug build is many times slower.

---

## 4. Step 1: Web search

The search is hidden behind a small trait (`src/main.rs:201`):

```rust
trait WebSearch {
    fn search(&self, query: &str, max_results: usize) -> Result<Vec<Document>>;
}
```

A `Document` is a title, a URL and the text to train on. `search_backend` (`src/main.rs:206`) picks the implementation: Brave if the environment variable `BRAVE_API_KEY` is set, otherwise Wikipedia. To add another search engine, write a struct, implement `WebSearch` for it and return it from `search_backend`.

### Wikipedia (default, no key needed)

`WikipediaSearch::search` (`src/main.rs:255`) makes two kinds of requests to the MediaWiki API:

1. `action=query&list=search&srsearch=<topic>` returns the titles of the best-matching articles.
2. For each title, `prop=extracts&explaintext=1` returns the article as plain text. The API only returns the full text of one page per request, so this is a loop.

`clean_wikipedia_text` (`src/main.rs:303`) removes what would be bad training text:

- Lines like `== History ==` are headings and are dropped.
- At "See also", "References", "Notes", "External links" or "Further reading" the rest of the article is cut off: these are lists of links and citations.
- Lines of 40 characters or less are dropped. They are mostly list items or leftovers of tables and formulas.
- At most 8,000 characters per article are kept (`MAX_CHARS_PER_PAGE`), so one long article can't dominate the training.

Wikipedia rejects requests without a `User-Agent`, so the HTTP client sets one (`USER_AGENT`).

### Brave Search (with `BRAVE_API_KEY`)

`BraveSearch::search` (`src/main.rs:347`) calls the Brave web search API with `extra_snippets=true`. Brave doesn't return full pages, only a title, a description and up to five extra snippets per result, so the training text is much shorter than with Wikipedia. The snippets contain HTML markup such as `<strong>`, which `strip_html` (`src/main.rs:376`) removes, together with the most common HTML entities.

---

## 5. Step 2: Training data

### Downloading the model

`download_model` (`src/main.rs:410`) downloads three files from the Hub with `hf-hub`:

| File | Content |
| --- | --- |
| `config.json` | Architecture: 6 layers, 12 heads, 768 dimensions, 50,257 tokens, up to 1,024 positions |
| `tokenizer.json` | The BPE vocabulary and merge rules |
| `model.safetensors` | The 82 M pretrained weights as 32-bit floats (about 330 MB) |

`MODEL_REVISION` pins a specific Git commit of the model repository, so the program always gets the same files even if the model on the Hub is updated. The files land in `~/.cache/huggingface` and are only downloaded once.

### From documents to windows

`build_windows` (`src/main.rs:454`) tokenizes every document and appends the ids to one long stream, with `<|endoftext|>` between the documents:

```text
[doc 1 tokens ...] 50256 [doc 2 tokens ...] 50256 [doc 3 tokens ...] 50256 ...
```

The stream is then cut into windows of `SEQ_LEN + 1 = 129` tokens with `stream.windows(129).step_by(128)`. Consecutive windows overlap by exactly one token, so every token in the stream is a prediction target exactly once. The tail of the stream that doesn't fill a whole window is dropped.

Every 10th window (`VALIDATION_EVERY`) goes into the **validation set**. The model never trains on it; it measures how well the model does on text it hasn't seen.

### Why 129 tokens: inputs and targets

`to_batch` (`src/main.rs:475`) turns a list of windows into two tensors of shape `(batch, 128)`: the **inputs** are tokens 0 to 127, the **targets** are tokens 1 to 128, the same window shifted by one:

```text
window:   Rust   is    a     programming  language  that   ...
input:    Rust   is    a     programming  language  ...
target:   is     a     programming  language  that  ...
```

At every position, the target is the token that comes next. So one window of 128 inputs gives 128 training examples at once: "after `Rust` comes `is`", "after `Rust is` comes `a`", and so on. The causal mask (chapter 6.3) makes sure the model can't cheat by looking at the target.

---

## 6. Step 3: The GPT-2 network, layer by layer

### Why the model is written by hand

`candle-transformers` already contains a GPT-2, but its models are built for inference. They use fused kernels, such as a single optimized operation for the whole attention, for which candle has no gradient. A model built from those can't be trained. The GPT-2 in `src/main.rs:492-663` uses only basic tensor operations (`matmul`, `add`, `exp`, `mean`, ...), so candle can differentiate everything.

### The whole network

DistilGPT2 has the GPT-2 architecture with 6 instead of 12 blocks. This is the path of one batch through `Gpt2::forward` (`src/main.rs:640`):

```text
 token ids                                   (b, t)
     │
     ├── wte: token embedding    ──┐         (b, t, 768)   "what" each token is
     │                             + ──►  x  (b, t, 768)
     └── wpe: position embedding ──┘         (t, 768)      "where" each token is
                │
   ┌────────────▼─────────────────────────────────────┐
   │ Block (6 times, h.0 ... h.5)                     │
   │                                                  │
   │   x ──► ln_1 ──► Attention (12 heads) ──► + ◄── x│  tokens exchange information
   │   x ──► ln_2 ──► c_fc ─► GELU ─► c_proj ─► + ◄─ x│  every token is processed alone
   └────────────┬─────────────────────────────────────┘
                │                            (b, t, 768)
              ln_f: final layer norm
                │
           x · wteᵀ : output layer           (b, t, 50257)  logits: a score for every
                                                            possible next token
```

The individual parts are described below in the order of the code.

### 6.1 Token and position embeddings: `wte`, `wpe`

```rust
let tok = self.wte.embedding(&ids.flatten_all()?)?.reshape((ids.dim(0)?, t, ()))?;
let pos = self.wpe.embedding(&positions)?;
let mut x = tok.broadcast_add(&pos)?;
```

- **`wte`** (word token embedding) is a matrix of shape `(50257, 768)`: one row of 768 numbers for every token in the vocabulary. `embedding` simply looks up the row for each token id. During pre-training these vectors were arranged so that tokens used in similar ways have similar vectors.
- **`wpe`** (word position embedding) is a matrix of shape `(1024, 768)`: one learned vector for every position 0 to 1023. Attention by itself doesn't know the order of tokens; without `wpe`, "dog bites man" and "man bites dog" would look the same.

The two are added. From now on, every token is a vector of 768 numbers that the following blocks gradually transform. This vector stream is often called the **residual stream**.

### 6.2 `Conv1D`: a linear layer

```rust
fn forward(&self, x: &Tensor) -> Result<Tensor> {
    Ok(x.broadcast_matmul(&self.weight)?.broadcast_add(&self.bias)?)
}
```

`Conv1D` (`src/main.rs:540`) is just a **linear layer**: `y = x · W + b`. Every output number is a weighted sum of all input numbers plus a constant. The name is historical: OpenAI's original code implemented it as a 1-D convolution. The practical difference to a normal `candle_nn::Linear` is that the weight is stored as `(in, out)` instead of `(out, in)`, so it can be multiplied without transposing. Keeping this layout is what lets the Hugging Face weights load unchanged.

### 6.3 The causal mask

```rust
let mask: Vec<f32> = (0..t)
    .flat_map(|i| (0..t).map(move |j| if j > i { -1e9 } else { 0.0 }))
    .collect();
```

The mask (`src/main.rs:651`) is a `t × t` matrix. Row `i` is the token at position `i`, column `j` the token it might look at. For `t = 4`:

```text
            looks at → j=0    j=1    j=2    j=3
  token i=0          0     -1e9   -1e9   -1e9
  token i=1          0      0     -1e9   -1e9
  token i=2          0      0      0     -1e9
  token i=3          0      0      0      0
```

It is added to the attention scores before the softmax. `exp(-1e9)` is 0, so every token gets an attention weight of exactly zero for all tokens **after** it. This is what makes GPT-2 a *causal* or *autoregressive* model: the prediction at position `i` can only depend on tokens `0..=i`. In training this means the model can't see the target it is supposed to predict; in generation it means the model works the same way as in training.

### 6.4 `LayerNorm`

```rust
let mean = x.mean_keepdim(D::Minus1)?;
let x = x.broadcast_sub(&mean)?;
let var = x.sqr()?.mean_keepdim(D::Minus1)?;
let x = x.broadcast_div(&(var + self.eps)?.sqrt()?)?;
Ok(x.broadcast_mul(&self.weight)?.broadcast_add(&self.bias)?)
```

Layer normalization (`src/main.rs:515`) works on each token vector separately:

```text
            x - mean(x)
  y = ───────────────────────── · weight + bias
       sqrt(variance(x) + eps)
```

It shifts and scales the 768 numbers of each token so that they have mean 0 and variance 1, and then applies a learned scale (`weight`) and shift (`bias`) for every dimension. Without it, the numbers in the residual stream would grow or shrink from block to block, and training would become unstable. `eps = 1e-5` prevents a division by zero.

GPT-2 normalizes the input of each sub-layer (`ln_1` before attention, `ln_2` before the MLP), not its output. This is called **pre-norm**. The `ln_f` after the last block normalizes the final vectors before the output layer.

### 6.5 The block: residual connections

```rust
fn forward(&self, x: &Tensor, mask: &Tensor) -> Result<Tensor> {
    let x = (x + self.attn.forward(&self.ln_1.forward(x)?, mask)?)?;
    let mlp = self.c_proj.forward(&self.c_fc.forward(&self.ln_2.forward(&x)?)?.gelu()?)?;
    Ok((x + mlp)?)
}
```

A `Block` (`src/main.rs:585`) has two sub-layers, attention and MLP. Neither replaces `x`; each **adds** its result to it (`x + ...`). These **residual connections** have two effects:

- Each sub-layer only has to learn a *change* to the representation, not rebuild it.
- During backpropagation, the gradient flows directly through the `+` back to earlier layers. Without this, deep networks are very hard to train.

Roughly speaking: **attention moves information between tokens, the MLP processes each token on its own.** The two alternate six times.

### 6.6 Self-attention: how a token uses its context

This is the part where the context of the previous tokens comes in. `Attention::forward` (`src/main.rs:565`):

```rust
let qkv = self.c_attn.forward(x)?;                  // (b, t, 3·768)
let (q, k, v) = (split(0)?, split(1)?, split(2)?);  // each (b, 12, t, 64)

let scores = (q.matmul(&k.t()?)? / (head_dim as f64).sqrt())?.broadcast_add(mask)?;
let weights = ops::softmax(&scores, D::Minus1)?;    // (b, 12, t, t)
let y = weights.matmul(&v)?.transpose(1, 2)?.reshape((b, t, c))?;
self.c_proj.forward(&y)
```

Step by step:

1. **Query, key, value.** `c_attn` is one linear layer from 768 to 3 × 768 numbers per token. The result is split into three parts:
   - **Query** `q`: "what am I looking for?"
   - **Key** `k`: "what do I offer?"
   - **Value** `v`: "what information do I pass on if someone attends to me?"
2. **Heads.** Each of the three is reshaped from 768 to 12 heads × 64 numbers and transposed to `(b, 12, t, 64)`. The 12 heads work independently and in parallel; each can learn to track a different relation, for example "the subject of this verb", "the previous token" or "the matching opening bracket".
3. **Scores.** `q · kᵀ` computes, for each head, the dot product of every query with every key: a `t × t` matrix that says how well token `i`'s question matches token `j`'s offer. Dividing by `√64 = 8` keeps the numbers in a range where the softmax doesn't become too sharp.
4. **Mask.** Adding the causal mask sets the scores for future tokens to −1,000,000,000.
5. **Attention weights.** The softmax over each row turns the scores into weights between 0 and 1 that add up to 1. Masked positions get weight 0.
6. **Weighted sum.** `weights · v`: every token receives a mix of the value vectors of the tokens it attends to, weighted by these weights.
7. **Merge heads.** The 12 results of 64 numbers are put back side by side into 768 numbers, and `c_proj` (768 → 768) mixes them.

An illustration with made-up numbers: when processing `" that"` in "Rust is a programming language that", one head might attend like this:

```text
 attends to:   Rust   is    a   programming  language  that
 weight:       0.31  0.02  0.01     0.09       0.45    0.12     (sum = 1)
```

The vector for `" that"` then picks up mostly information from `" language"` and `"Rust"`, which is what it needs to predict a verb that fits a programming language. Over six blocks, the vector of the last token gathers more and more of the relevant context. **This is the only place where tokens influence each other**; everything else in the network works on each position separately.

### 6.7 The MLP: feed-forward layer with GELU

```rust
let mlp = self.c_proj.forward(&self.c_fc.forward(&self.ln_2.forward(&x)?)?.gelu()?)?;
```

The MLP (multi-layer perceptron) of each block is two linear layers with an activation function in between:

```text
 768 ──c_fc──► 3072 ──GELU──► 3072 ──c_proj──► 768
```

- `c_fc` expands each token vector to four times its size.
- **GELU** (Gaussian Error Linear Unit) is a smooth non-linear function: roughly `0` for negative inputs and roughly the input itself for positive ones. Without a non-linearity, two linear layers would collapse into a single one and the network could only compute linear functions. candle's `gelu` is the tanh approximation that GPT-2 calls `gelu_new` (`config.json`: `"activation_function": "gelu_new"`); `gelu_erf` would be the exact version, which gives slightly different results.
- `c_proj` reduces back to 768.

The MLPs hold two thirds of the weights in every block. Research suggests they work partly like a key–value memory, where much of the factual and lexical knowledge of the model is stored.

### 6.8 Final layer norm and output layer

```rust
let x = self.ln_f.forward(&x)?;
Ok(x.broadcast_matmul(&self.wte.t()?)?)
```

After the six blocks, `ln_f` normalizes every token vector once more. Then each vector `h` of 768 numbers is multiplied with the **transposed token embedding** `wteᵀ` (768 × 50,257). The result for each token `i` of the vocabulary is the dot product `h · wte[i]`: how similar the final vector is to that token's embedding. These 50,257 numbers are the **logits**.

Using `wte` for both input and output is called **tied weights**. It saves 38.6 M parameters, and it is why the Hugging Face checkpoint has no separate `lm_head` weight.

### 6.9 Where the parameters are

| Part | Shape | Parameters | Trained in this example |
| --- | --- | ---: | --- |
| `wte` token embedding (also output layer) | 50,257 × 768 | 38,597,376 | no (frozen) |
| `wpe` position embedding | 1,024 × 768 | 786,432 | no (frozen) |
| per block: `ln_1`, `ln_2` | 2 × (768 + 768) | 3,072 | yes |
| per block: `attn.c_attn` | 768 × 2,304 + 2,304 | 1,771,776 | yes |
| per block: `attn.c_proj` | 768 × 768 + 768 | 590,592 | yes |
| per block: `mlp.c_fc` | 768 × 3,072 + 3,072 | 2,362,368 | yes |
| per block: `mlp.c_proj` | 3,072 × 768 + 768 | 2,360,064 | yes |
| 6 blocks together | | 42,527,232 | yes |
| `ln_f` | 768 + 768 | 1,536 | yes |
| **Total** | | **81,912,576** | 42.5 M trained |

### 6.10 How the pretrained weights get in

```rust
let mut varmap = VarMap::new();
let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
let model = Gpt2::new(&config, vb.pp("transformer"))?;
varmap.load(&files.weights)?;
```

`Gpt2::new` creates every weight through the `VarBuilder`, which registers it in the `VarMap` under its full name, for example `transformer.h.3.mlp.c_fc.weight`. At this point the weights are initialized randomly or with constants (the `get_with_hints` init values). `varmap.load` then reads `model.safetensors` and overwrites each `Var` with the tensor of the same name. Because the model holds the same `Var`s (they share their storage), the model now has the pretrained weights. If a name or a shape didn't match the checkpoint, `load` would return an error.

---

## 7. Step 4: Fine-tuning

### The loss: cross-entropy

`batch_loss` (`src/main.rs:670`) runs the model on a batch and compares its predictions with the targets:

```rust
let logits = model.forward(inputs)?;                     // (8, 128, 50257)
loss::cross_entropy(&logits.reshape((b * t, v))?, &targets.reshape(b * t)?)
```

For every one of the 8 × 128 = 1,024 positions, `cross_entropy` applies a (log-)softmax to the logits and takes the negative logarithm of the probability the model gave to the **correct** next token, then averages:

```text
  loss = average over all positions of  −log P(correct next token | previous tokens)
```

If the model gives the right token probability 1, the loss is 0; at probability 0.02 it is 3.9. Lower is better.

**Perplexity** is `exp(loss)`. A perplexity of 50 means the model is, on average, as uncertain as if it had to choose evenly between 50 tokens. `main` prints both.

`evaluate` (`src/main.rs:676`) computes the average loss over the validation windows without training.

### The training loop

`train_model` (`src/main.rs:691`):

```rust
for epoch in 1..=EPOCHS {
    order.shuffle(&mut rng);
    for batch in order.chunks(BATCH_SIZE) {
        let (inputs, targets) = to_batch(&windows, device)?;
        let loss = batch_loss(model, &inputs, &targets)?;
        optimizer.backward_step(&loss)?;
    }
    let valid_loss = evaluate(model, valid, device)?;
    ...
}
```

- An **epoch** is one pass over all training windows. The order is shuffled every epoch with a fixed seed, so runs are reproducible.
- `optimizer.backward_step(&loss)` does two things. **Backward:** candle walks back through all operations that produced `loss` and computes the gradient of every trainable weight. **Step:** the optimizer changes each weight a little in the direction that lowers the loss.
- **AdamW** is the standard optimizer for Transformers. It keeps a running average of each weight's gradients and of their squares, so every weight gets its own effective step size. `weight_decay: 0.01` pulls weights slightly towards 0, which counteracts overfitting. The learning rate `5e-5` is small on purpose: the pretrained weights are already good and should only be adjusted, not overwritten.

Because of the causal mask, one forward pass computes the predictions for all 128 positions of a window in parallel, each with exactly the context it would have had when reading from left to right. This is called **teacher forcing** and is why training a GPT is so much more efficient than generating with it.

### Two protections against overfitting

The search results are only about 8,000 tokens, compared to the billions of tokens of pre-training. A model with 82 M parameters can memorize such a small text after a few passes; it then gets better on the training text but worse on everything else. The code limits this in two ways:

1. **Frozen embeddings.** The `Var`s whose names end in `wte.weight` or `wpe.weight` are left out of the optimizer. These are 39.4 M of the 82 M weights. Because `wte` is also the output layer, changing it would shift the probabilities of tokens across the whole vocabulary.
2. **Keeping the best epoch.** After every epoch the validation loss is measured. If it is the best so far, a copy of the trainable weights is made (`snapshot`, `src/main.rs:709`). At the end, the best copy is written back with `var.set`. This is a simple form of **early stopping**.

Note the comment in `snapshot`: `Tensor::clone` in candle only copies a reference to the same storage, so a "clone" would change along with the training. `copy()` makes a real copy of the data.

If the validation loss becomes `NaN` or infinite, the training has diverged (usually a learning rate that is too high) and the program stops with an error.

---

## 8. Step 5: Generating text, one token at a time

This chapter answers the question: **where is the next token chosen, and how does its probability depend on the previous tokens?**

### 8.1 The generation loop

The next token is chosen in `generate` (`src/main.rs:754`). This is the whole loop, with the important lines marked:

```rust
let mut ids = tokenizer.encode(prompt, false)?.get_ids().to_vec();   // the prompt as token ids

for _ in 0..GENERATED_TOKENS {                                         // 40 times
    let input = Tensor::new(ids.as_slice(), device)?.unsqueeze(0)?;    // shape (1, t)
    let logits = model.forward(&input)?                                // (1, t, 50257)
        .squeeze(0)?                                                   // (t, 50257)
        .get(ids.len() - 1)?;                                          // (50257): last position only  ◄── (A)
    let mut logits: Vec<f32> = logits.to_vec1()?;
    for &id in &ids { /* repetition penalty */ }                       //                             ◄── (B)
    let next = /* index of the largest logit */;                       //                             ◄── (C)
    if next == EOS_TOKEN { break; }
    ids.push(next);                                                    // context grows by one token  ◄── (D)
}
```

- **(A)** The model returns logits for **every** position of the input, because it was built for training (chapter 7). The logits at position `i` are the prediction for the token *after* token `i`. For generation only the prediction after the **last** token is needed, so `get(ids.len() - 1)` picks that row: 50,257 numbers, one per possible next token.
- **(B)** The repetition penalty lowers the logits of tokens that already occurred (8.4).
- **(C)** The token with the highest logit is chosen. This is **greedy decoding** (8.5).
- **(D)** The chosen token is appended to `ids`, and the next iteration runs the model on the prompt **plus everything generated so far**. This is what *autoregressive* means: every output becomes part of the input for the next step.

The loop ends after 40 tokens or when the model predicts `<|endoftext|>`. Finally the new tokens are decoded back to text and appended to the prompt.

This trace shows how `ids` grows in the first steps of the "before" run from chapter 10 (the pretrained model, default prompt). Each step feeds the whole row into the model, and only the prediction at the last position is used:

```text
 step  input ids (context)                                           t   chosen token
 ────  ────────────────────────────────────────────────────────────  ──  ──────────────
  1    49444 318 257 8300 3303 326                                    6  468   " has"
  2    49444 318 257 8300 3303 326 468                                7  587   " been"
  3    49444 318 257 8300 3303 326 468 587                            8  973   " used"
  4    49444 318 257 8300 3303 326 468 587 973                        9  329   " for"
  5    ... 973 329                                                   10  625   " over"
  6    ... 329 625                                                   11  1160  " 20"
  7    ... 625 1160                                                  12  812   " years"
  ...
       "Rust is a programming language that has been used for over 20 years ..."
```

Each chosen token is one draw from the factor `P(x_t | x₁ … x_{t−1})` of the chain rule (chapter 2), with greedy choice and the repetition penalty. Nothing is planned ahead. The model doesn't "know" in step 1 that it will write "20 years"; that phrase only becomes likely once `" has been used for over"` is in the context.

### 8.2 How the probability is calculated from the context

This is one factor of the chain rule from chapter 2. Written as a formula, the probability of a candidate next token `w` after the context tokens `x₁ … x_T` is:

```text
                                exp( h_T · wte[w] )
  P(w | x₁ … x_T)  =  ───────────────────────────────────
                       Σ over all 50,257 tokens u  exp( h_T · wte[u] )

  where h_T = ln_f( Block₆( … Block₁( wte[x_t] + wpe[t]  for t = 1…T ) … ) )  at position T
```

In words, following the data through the code:

1. **Every context token becomes a vector**: its token embedding plus the embedding of its position (6.1).
2. **Six blocks mix the context into the last vector.** In every attention layer, the vector at the last position `T` computes attention weights over all positions `1 … T` (the mask allows exactly these) and takes in a weighted mix of their value vectors (6.6). The MLPs then transform the result (6.7). After six blocks, `h_T` is a summary of "everything about the context that matters for what comes next".
3. **Compare with every vocabulary token.** `h_T` is multiplied with each row of `wte`: tokens whose embedding points in the same direction as `h_T` get a high logit (6.8).
4. **Softmax** turns the logits into probabilities (chapter 2).

So the context influences the probability *only* through `h_T`, and `h_T` depends on the earlier tokens *only* through the attention weights. Change one word in the prompt, and its key and value vectors change, the attention weights change, `h_T` changes, and with it the whole distribution of 50,257 probabilities.

Two details are worth knowing:

- `generate` never computes the softmax. It is not needed: softmax doesn't change the order, so the largest logit is also the largest probability. Only for sampling (8.5) or for displaying probabilities does the softmax have to be computed.
- The context is limited to 1,024 tokens, because `wpe` has only 1,024 rows. `Gpt2::forward` checks this with `ensure!`. With a short prompt and 40 new tokens, the limit is never reached.

### 8.3 Seeing the probabilities: `print_next_token_probabilities`

`print_next_token_probabilities` (`src/main.rs:784`) does the same as one step of `generate`, but applies the softmax explicitly and prints the 5 most likely next tokens:

```rust
let logits = model.forward(&input)?.squeeze(0)?.get(ids.len() - 1)?;
let probs: Vec<f32> = ops::softmax(&logits, 0)?.to_vec1()?;
```

`main` calls it before and after the fine-tuning, so you can see directly how the training has shifted the distribution for the first generated token (see chapter 10).

### 8.4 The repetition penalty

```rust
for &id in &ids {
    let l = &mut logits[id as usize];
    *l = if *l > 0.0 { *l / REPETITION_PENALTY } else { *l * REPETITION_PENALTY };
}
```

Small models with greedy decoding tend to get stuck in loops ("the language of the language of the language ..."). For every token that is already in the context (prompt included), the logit is made less attractive: a positive logit is divided by 1.3, a negative one is multiplied by 1.3. Both make the logit smaller. This is the rule from the [CTRL paper](https://arxiv.org/abs/1909.05858) that Hugging Face also uses.

GPT-2 has no fixed scale for its logits; only their differences matter for the softmax. Its logits are usually negative, so in practice the "multiply by 1.3" branch applies. The penalty then lowers a logit by 30 % of its absolute value, which can be a large shift compared to the gaps between the top candidates. It is a heuristic on the logits, not a probability rule: after the penalty the numbers no longer follow the model's probability distribution, which is why `print_next_token_probabilities` shows the distribution without it.

The penalty already changes the very first generated token in this example. `" is"` is the most likely token after the prompt (16.2 %), but it occurs in the prompt itself, so its logit is lowered, and greedy decoding picks `" has"` instead (see chapter 10).

One difference to Hugging Face: the loop runs over `ids`, which can contain the same token several times. A token that has occurred three times is penalized three times (by 1.3³ ≈ 2.2). Hugging Face penalizes each distinct token once. To get that behaviour, loop over the distinct ids, for example after `let seen: std::collections::HashSet<u32> = ids.iter().copied().collect();`.

### 8.5 Greedy decoding and alternatives

**Greedy decoding** always takes the most likely token. It is deterministic: the same prompt always gives the same text, which makes the before/after comparison fair. But it is often dull and repetitive. Common alternatives choose the next token at random, according to the probabilities:

- **Temperature sampling:** divide the logits by a temperature `T` before the softmax. `T < 1` makes the distribution sharper (closer to greedy), `T > 1` flatter (more surprising).
- **Top-k sampling:** only keep the `k` most likely tokens, then sample.
- **Top-p (nucleus) sampling:** only keep the smallest set of most likely tokens whose probabilities add up to `p` (e.g. 0.9), then sample.

A sketch of temperature sampling that could replace step **(C)**, using `rand`, which the project already depends on:

```rust
use rand::distr::{Distribution, weighted::WeightedIndex};

const TEMPERATURE: f32 = 0.8;
let scaled = Tensor::new(logits.as_slice(), device)?.affine(1.0 / TEMPERATURE as f64, 0.0)?;
let probs: Vec<f32> = ops::softmax(&scaled, 0)?.to_vec1()?;
let next = WeightedIndex::new(&probs)?.sample(&mut rng) as u32;
```

(`rng` would be a `StdRng::seed_from_u64(SEED)` created before the loop.)

### 8.6 Why generation is slow

In every step, `generate` runs the model on the **whole** sequence again, although the vectors for the earlier tokens haven't changed: because of the causal mask, earlier positions never depend on later ones. Real inference code keeps the keys and values of all earlier tokens from the previous step in a **KV cache** and only computes the new token. For 40 tokens and a short prompt this doesn't matter much here; for long texts it is the most important optimization.

---

## 9. Saving the model

`save_model` (`src/main.rs:802`) writes the directory `finetuned-model/`:

| File | Source |
| --- | --- |
| `model.safetensors` | `varmap.save`: all weights under their Hugging Face names, including the unchanged embeddings |
| `config.json` | copied unchanged from the download |
| `tokenizer.json` | copied unchanged from the download |

That is a complete Hugging Face model directory. In Python:

```python
from transformers import AutoModelForCausalLM, AutoTokenizer

model = AutoModelForCausalLM.from_pretrained("finetuned-model")
tokenizer = AutoTokenizer.from_pretrained("finetuned-model")
ids = tokenizer("Rust is a programming language that", return_tensors="pt").input_ids
print(tokenizer.decode(model.generate(ids, max_new_tokens=40, do_sample=False,
                                      repetition_penalty=1.3)[0]))
```

`transformers` notices that `lm_head.weight` is missing from the file and ties it to `wte`, exactly like this implementation does. In Rust, the same file can be loaded again with `VarMap::load` as in step 3. The program does this itself with `--no-finetune`.

### 9.1 Generating with the saved model: `--no-finetune`

Fine-tuning takes minutes, but generating 40 tokens takes only a few seconds. To try more prompts with the fine-tuned model, you don't have to train again:

```bash
cargo run --release -- --no-finetune                            # default prompt
cargo run --release -- --no-finetune "The borrow checker"       # your own prompt
cargo run --release -- --help                                   # usage
```

**Arguments.** `parse_args` (`src/main.rs:78`) reads the command line. Options can be anywhere, and an unknown option such as a mistyped `--no-finetun` stops the program with the usage text instead of being taken as a search topic. Without training there is no search, so with `--no-finetune` a single argument is the **prompt**. If you give two arguments, as in a normal run, the first one (the topic) is ignored with a note. This way you can just add `--no-finetune` to a previous command line.

**What runs.** `main` calls `generate_with_saved_model` (`src/main.rs:178`) and returns right after it:

```text
 normal run:      search ─► download ─► windows ─► load pretrained ─► train ─► generate ─► save
 --no-finetune:                                    load finetuned-model/ ─────► generate
```

1. `saved_model_files` (`src/main.rs:428`) points to the three files in `finetuned-model/` and checks that they exist. If not, it reports that the program has to run once without `--no-finetune` first.
2. `load_model` (`src/main.rs:503`) builds the network and loads the weights. It is the same function the normal run uses for the pretrained weights from the Hugging Face cache; only the file paths differ. Because `save_model` stores all weights under their Hugging Face names, `VarMap::load` finds every one.
3. `print_next_token_probabilities` and `generate` work exactly as described in chapter 8.

There is no web access, no Hugging Face download and no training: `finetuned-model/` holds everything that is needed. Generation is deterministic (greedy decoding), so the same prompt gives the same text as the "after" text of the run that created the model. With the default prompt this is a quick way to check that the saved model was loaded correctly:

```text
=== Generation with the fine-tuned model in finetuned-model/ (no search, no training) ===
  next token after "Rust is a programming language that":
     14.9 %  " is"
      6.9 %  " uses"
      5.9 %  " provides"
      4.9 %  " can"
      4.8 %  " has"

  Rust is a programming language that uses the Rust compiler to compile and run code. It was
  developed by Martin Brink in 2009, which has been used for many years as an open source
  project of interest since it became available under GPL
```

Some things to keep in mind:

- `finetuned-model/` is a relative path, so run the program from the project directory, as `cargo run` does.
- The saved model is the one from the **last** normal run. A new normal run always starts again from the pretrained `distilgpt2`, not from the saved model, and overwrites `finetuned-model/`.
- In this mode, the topic is used only for training. To see how much a training run changed the model, compare with the "before" output of the normal run; `--no-finetune` doesn't load the pretrained model.

---

## 10. Expected output

A run with the default topic and prompt (about 5 minutes on a 32-thread CPU, after the model is cached). The Wikipedia articles change over time, so your numbers may differ slightly:

```text
=== 1. Web search: "Rust programming language" ===
  (using the Wikipedia search API; set BRAVE_API_KEY for a general web search)
    8005 chars  Rust (programming language)  (https://en.wikipedia.org/wiki/Rust_(programming_language))
    2127 chars  Outline of the Rust programming language  (...)
    8003 chars  List of programming languages by type  (...)
    8001 chars  Rust syntax  (...)
    8002 chars  Zig (programming language)  (...)
    8002 chars  Functional programming  (...)

=== 2. Training data ===
  8471 tokens from 6 documents
  60 training windows and 6 validation windows of 128 tokens

=== 3. Model: distilbert/distilgpt2 ===
  81.9 M parameters loaded
  validation loss 3.949, perplexity 51.9
  next token after "Rust is a programming language that":
     16.2 %  " is"
      6.5 %  " has"
      6.5 %  " allows"
      5.2 %  " can"
      4.5 %  " uses"

=== 4. Fine-tuning ===
  training 42.5 M parameters, embeddings frozen
  epoch 1/4: training loss 3.857, validation loss 3.850 (best so far) (68 s)
  epoch 2/4: training loss 3.561, validation loss 3.836 (best so far) (66 s)
  epoch 3/4: training loss 3.336, validation loss 3.857 (66 s)
  epoch 4/4: training loss 3.125, validation loss 3.907 (66 s)
  restored the weights with the best validation loss (3.836)

=== 5. Before and after ===
  validation loss  3.949 -> 3.836
  perplexity       51.9 -> 46.3
  next token after "Rust is a programming language that":
     14.9 %  " is"
      6.9 %  " uses"
      5.9 %  " provides"
      4.9 %  " can"
      4.8 %  " has"

  Before: Rust is a programming language that has been used for over 20 years. It's the most
          popular and widely adopted languages in modern times, ...
  After:  Rust is a programming language that uses the Rust compiler to compile and run code.
          It was developed by Martin Brink in 2009, ...
```

How to read it:

- **Step 2:** 8,471 tokens give 66 windows; 6 of them are for validation.
- **Step 4 shows overfitting:** the training loss falls in every epoch (3.86 → 3.13), but the validation loss is best after epoch 2 and then rises again. From epoch 3 on, the model is memorizing the training passages. The weights of epoch 2 are restored.
- **The next-token distribution has moved:** after the training, `" uses"` and `" provides"` (common in the descriptions of software in the articles) have moved up, and `" has"` has moved down.
- **The most likely token is not the one chosen.** `" is"` has the highest probability both times, but `generate` continues with `" has"` and `" uses"`. The reason is the repetition penalty (8.4): `" is"` already occurs in the prompt, so its logit is lowered before the choice, and that drops it below the next candidate. `print_next_token_probabilities` shows the distribution *without* the penalty.
- **Style, not facts:** the "after" text sounds more like the articles (compiler, open source project), but "Martin Brink" and "available under GPL" (the full text ends with it, see 9.1) are made up: Rust was started by Graydon Hoare as a personal project and is licensed under MIT/Apache 2.0. The model has learned which *kind* of words follow, not which facts are true.

---

## 11. Errors and pitfalls

| Symptom | Cause and fix |
| --- | --- |
| `the search for "..." returned no results` | The topic has no matching Wikipedia article. Use the English name, or a broader topic. |
| `not enough text for training; try a broader topic` | Fewer than 10 windows of 128 tokens, so there is no validation window. Typical with Brave (snippets only) or a very narrow topic. Increase `SEARCH_RESULTS` or `MAX_CHARS_PER_PAGE`. |
| `the Brave Search API rejected the request` | `BRAVE_API_KEY` is wrong or the free quota is used up. Unset the variable to use Wikipedia. |
| HTTP 403 from Wikipedia | Requests without a proper `User-Agent` are blocked; keep `USER_AGENT` set. |
| `could not download ... from Hugging Face` | No network on the first run, or a proxy. Once downloaded, the files are taken from `~/.cache/huggingface`. |
| `the training diverged; try a smaller LEARNING_RATE` | The loss became `NaN`. Lower `LEARNING_RATE`, e.g. to `2e-5`. |
| Training takes many minutes per epoch | Probably a debug build. Use `cargo run --release`. |
| `finetuned-model/config.json not found: there is no fine-tuned model yet` | `--no-finetune` needs a model from an earlier normal run. Run once without the option, and run from the project directory. |
| `unknown option --...` | A mistyped option. The only options are `--no-finetune` and `--help`. |
| Validation loss gets worse from epoch 2 on | Expected with so little text: the model starts to memorize. The best epoch is restored automatically. |
| `sequence of N tokens is longer than 1024` | A very long prompt plus `GENERATED_TOKENS` exceeds GPT-2's context. Shorten the prompt. |

---

## 12. Known limitations

- **No dropout.** `config.json` specifies dropout of 0.1 for embeddings, attention and residuals. This implementation leaves it out, so training behaves like inference mode. With frozen embeddings and early stopping this is acceptable, but dropout would regularize further.
- **Overlapping validation data.** The validation windows are taken from between the training windows, from the same articles. The validation loss therefore measures "unseen passages of the same texts" rather than "new texts about the topic", and is somewhat optimistic.
- **Facts are not learned reliably.** A few thousand tokens change the style and vocabulary of the model, but it still makes things up (see the invented "Martin Brink" in the README). For questions about current search results, retrieval-augmented generation (putting the results into the prompt) works much better than fine-tuning.
- **CPU only.** `Device::Cpu` is hard-coded. candle supports CUDA and Metal through features, but they are not enabled here.
- **No KV cache** and no sampling in `generate` (see 8.5 and 8.6).
- **Repetition penalty counts duplicates** several times (see 8.4).
- **Fixed hyperparameters.** All settings are constants at the top of `src/main.rs`; there are no command-line options for them. The only option is `--no-finetune`.

---

## 13. Ideas for experiments

- **Watch the distribution change.** Choose a prompt that is typical for the topic and compare the top-5 tokens before and after. Try `k = 20` in the call to `print_next_token_probabilities`.
- **Unfreeze the embeddings.** Remove the filter in `train_model` and observe whether the validation loss gets better or worse, and how fast the model overfits.
- **Train only some layers.** Train only the last block (`h.5`), or only the attention layers, and compare the results with the full training.
- **Sampling.** Add temperature or top-p sampling (8.5) and generate several continuations of the same prompt.
- **KV cache.** Change `Attention::forward` so it can take and return cached keys and values, and `generate` so it only feeds the newest token.
- **Look at attention.** Return `weights` from `Attention::forward` and print, for the last prompt token, which tokens each head attends to.
- **More data or a bigger model.** Increase `SEARCH_RESULTS`, combine several queries, or switch to `openai-community/gpt2` (12 layers) or `gpt2-medium`. The code reads the sizes from `config.json`, so only the model name and revision change.
- **LoRA.** Instead of training whole weight matrices, add small low-rank matrices `A · B` to `c_attn` and train only those: a few hundred thousand parameters instead of 42 million.

---

## 14. Further reading

### The libraries

- [candle](https://github.com/huggingface/candle) and its [documentation](https://docs.rs/candle-core): tensors and autograd in Rust; the [candle book](https://huggingface.github.io/candle/) has a training chapter
- [candle-nn](https://docs.rs/candle-nn): `VarMap`, `VarBuilder`, `AdamW`, losses
- [tokenizers](https://github.com/huggingface/tokenizers): Hugging Face's tokenizer library
- [hf-hub](https://docs.rs/hf-hub): downloading from the Hugging Face Hub
- [safetensors](https://huggingface.co/docs/safetensors): the weight file format

### The model

- [distilbert/distilgpt2](https://huggingface.co/distilbert/distilgpt2) on Hugging Face
- [GPT-2 in Hugging Face Transformers](https://github.com/huggingface/transformers/blob/main/src/transformers/models/gpt2/modeling_gpt2.py): the Python implementation whose names and layout this file follows

### Background and papers

- [The Illustrated GPT-2](https://jalammar.github.io/illustrated-gpt2/) by Jay Alammar: a visual walk through exactly this architecture
- Andrej Karpathy, [Let's build GPT: from scratch, in code, spelled out](https://www.youtube.com/watch?v=kCc8FmEb1nY) and [nanoGPT](https://github.com/karpathy/nanoGPT): the same model built step by step in PyTorch
- [3Blue1Brown: Neural networks, chapters 5–7](https://www.3blue1brown.com/topics/neural-networks): visual explanations of transformers, attention and MLPs
- [Hugging Face LLM Course](https://huggingface.co/learn/llm-course/chapter1/1): tokenizers, fine-tuning and generation strategies
- Vaswani et al., 2017: [Attention Is All You Need](https://arxiv.org/abs/1706.03762), the Transformer
- Radford et al., 2019: [Language Models are Unsupervised Multitask Learners](https://cdn.openai.com/better-language-models/language_models_are_unsupervised_multitask_learners.pdf), GPT-2
- Sanh et al., 2019: [DistilBERT](https://arxiv.org/abs/1910.01108), the distillation method used for DistilGPT2
- Loshchilov & Hutter, 2017: [Decoupled Weight Decay Regularization](https://arxiv.org/abs/1711.05101), AdamW
- Keskar et al., 2019: [CTRL](https://arxiv.org/abs/1909.05858), source of the repetition penalty
- Holtzman et al., 2019: [The Curious Case of Neural Text Degeneration](https://arxiv.org/abs/1904.09751), why greedy decoding repeats itself, and top-p sampling
- Hu et al., 2021: [LoRA: Low-Rank Adaptation of Large Language Models](https://arxiv.org/abs/2106.09685)
- Geva et al., 2020: [Transformer Feed-Forward Layers Are Key-Value Memories](https://arxiv.org/abs/2012.14913)

### Related examples in this repository

- [r_proj_065](../r_proj_065/DOCUMENTATION.md): PyTorch from Rust with `tch`
- [r_proj_066](../r_proj_066/DOCUMENTATION.md): inference with ONNX Runtime
- [r_proj_068](../r_proj_068/DOCUMENTATION.md): ready-made BERT pipelines with `rust_bert`
