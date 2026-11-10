//! Fine-tunes a small language model from Hugging Face on text from a web search.
//!
//! The program runs five steps:
//!
//! 1. **Web search:** searches the web for a topic and collects the text of the results
//!    (Wikipedia by default, no API key needed; the Brave Search API if `BRAVE_API_KEY` is set).
//! 2. **Training data:** cleans the text, tokenizes it with the model's tokenizer and cuts it
//!    into training and validation windows.
//! 3. **Model:** downloads `distilbert/distilgpt2` (82 M parameters) from the Hugging Face Hub
//!    and loads it into a GPT-2 implementation written with `candle` in this file.
//! 4. **Fine-tuning:** trains the transformer layers with AdamW on next-token prediction and
//!    keeps the weights of the epoch with the lowest validation loss.
//! 5. **Comparison:** shows validation loss, perplexity and generated text before and after,
//!    and saves the fine-tuned model in the Hugging Face format to `finetuned-model/`.
//!
//! Usage: `cargo run --release -- "your search topic" "a prompt to complete"`
//!
//! With `--no-finetune`, the program skips the search and the training and only generates text
//! with the model saved by an earlier run in `finetuned-model/`:
//! `cargo run --release -- --no-finetune "a prompt to complete"`

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use candle_core::{D, DType, Device, Tensor, Var};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap, loss, ops};
use hf_hub::HFClientSync;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde::Deserialize;
use tokenizers::Tokenizer;

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

const DEFAULT_TOPIC: &str = "Rust programming language";
const DEFAULT_PROMPT: &str = "Rust is a programming language that";

const MODEL_OWNER: &str = "distilbert";
const MODEL_NAME: &str = "distilgpt2";
/// A fixed commit, so that the model can't change between two runs.
const MODEL_REVISION: &str = "2290a62682d06624634c1f46a6ad5be0f47f38aa";
const OUTPUT_DIR: &str = "finetuned-model";

/// How many search results are turned into training text.
const SEARCH_RESULTS: usize = 6;
/// At most this many characters are taken from one page.
const MAX_CHARS_PER_PAGE: usize = 8000;

/// Tokens per training window. GPT-2 can handle up to 1024; shorter is faster on a CPU.
const SEQ_LEN: usize = 128;
const BATCH_SIZE: usize = 8;
const EPOCHS: usize = 4;
const LEARNING_RATE: f64 = 5e-5;
/// Every n-th window goes into the validation set instead of training.
const VALIDATION_EVERY: usize = 10;
const GENERATED_TOKENS: usize = 40;
const SEED: u64 = 42;

/// User-Agent for the HTTP requests. Wikipedia rejects requests without one.
const USER_AGENT: &str = "r_proj_069-finetune-example/0.1 (Rust; reqwest)";

const USAGE: &str = "\
usage: r_proj_069 [TOPIC] [PROMPT]               search, fine-tune, generate and save the model
       r_proj_069 --no-finetune [PROMPT]         only generate, with the model in finetuned-model/";

/// The command-line arguments.
struct Args {
    topic: String,
    prompt: String,
    /// `false` with `--no-finetune`: no search and no training, only generation.
    finetune: bool,
}

fn parse_args() -> Result<Args> {
    let mut finetune = true;
    let mut positional = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--no-finetune" => finetune = false,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            option if option.starts_with("--") => bail!("unknown option {option}\n{USAGE}"),
            _ => positional.push(arg),
        }
    }
    ensure!(positional.len() <= 2, "too many arguments\n{USAGE}");

    let mut positional = positional.into_iter();
    let (topic, prompt) = if finetune {
        (positional.next(), positional.next())
    } else {
        // Without training there is no search, so a single argument is the prompt.
        // With two, the first one (the topic) is ignored, so the usual command line still works.
        let last = positional.next_back();
        if let Some(topic) = positional.next() {
            println!("(--no-finetune: the topic \"{topic}\" is not used)");
        }
        (None, last)
    };
    Ok(Args {
        topic: topic.unwrap_or_else(|| DEFAULT_TOPIC.to_string()),
        prompt: prompt.unwrap_or_else(|| DEFAULT_PROMPT.to_string()),
        finetune,
    })
}

fn main() -> Result<()> {
    let Args { topic, prompt, finetune } = parse_args()?;
    let device = Device::Cpu;

    if !finetune {
        return generate_with_saved_model(&prompt, &device);
    }

    // 1. Web search
    println!("=== 1. Web search: \"{topic}\" ===");
    let search = search_backend();
    let documents = search.search(&topic, SEARCH_RESULTS)?;
    ensure!(!documents.is_empty(), "the search for \"{topic}\" returned no results");
    for doc in &documents {
        println!("  {:>6} chars  {}  ({})", doc.text.len(), doc.title, doc.url);
    }

    // 2. Training data
    println!("\n=== 2. Training data ===");
    let files = download_model()?;
    let tokenizer = Tokenizer::from_file(&files.tokenizer).map_err(anyhow::Error::msg)?;
    let (train, valid) = build_windows(&tokenizer, &documents)?;
    println!(
        "  {} training windows and {} validation windows of {SEQ_LEN} tokens",
        train.len(),
        valid.len()
    );
    ensure!(
        !train.is_empty() && !valid.is_empty(),
        "not enough text for training; try a broader topic"
    );

    // 3. Model
    println!("\n=== 3. Model: {MODEL_OWNER}/{MODEL_NAME} ===");
    let (model, varmap) = load_model(&files, &device)?;
    let n_params: usize = varmap.all_vars().iter().map(|v| v.elem_count()).sum();
    println!("  {:.1} M parameters loaded", n_params as f64 / 1e6);

    let before_loss = evaluate(&model, &valid, &device)?;
    let before_text = generate(&model, &tokenizer, &prompt, &device)?;
    println!("  validation loss {before_loss:.3}, perplexity {:.1}", before_loss.exp());
    print_next_token_probabilities(&model, &tokenizer, &prompt, 5, &device)?;

    // 4. Fine-tuning
    println!("\n=== 4. Fine-tuning ===");
    train_model(&model, &varmap, &train, &valid, &device)?;

    // 5. Comparison
    println!("\n=== 5. Before and after ===");
    let after_loss = evaluate(&model, &valid, &device)?;
    let after_text = generate(&model, &tokenizer, &prompt, &device)?;
    println!("  validation loss  {before_loss:.3} -> {after_loss:.3}");
    println!("  perplexity       {:.1} -> {:.1}", before_loss.exp(), after_loss.exp());
    print_next_token_probabilities(&model, &tokenizer, &prompt, 5, &device)?;
    println!("\n  Before: {before_text}");
    println!("\n  After:  {after_text}");

    save_model(&varmap, &files)?;
    println!("\nThe fine-tuned model is in {OUTPUT_DIR}/ (loadable with Hugging Face Transformers).");
    println!("Generate more text with it, without training: cargo run --release -- --no-finetune \"your prompt\"");
    Ok(())
}

/// `--no-finetune`: loads the model that an earlier run saved in `finetuned-model/` and
/// continues the prompt with it. There is no web search, no download and no training.
fn generate_with_saved_model(prompt: &str, device: &Device) -> Result<()> {
    println!("=== Generation with the fine-tuned model in {OUTPUT_DIR}/ (no search, no training) ===");
    let files = saved_model_files()?;
    let tokenizer = Tokenizer::from_file(&files.tokenizer).map_err(anyhow::Error::msg)?;
    let (model, _varmap) = load_model(&files, device)?;

    print_next_token_probabilities(&model, &tokenizer, prompt, 5, device)?;
    let text = generate(&model, &tokenizer, prompt, device)?;
    println!("\n  {text}");
    Ok(())
}

// ---------------------------------------------------------------------------
// 1. Web search
// ---------------------------------------------------------------------------

/// One search result with the text that is used for training.
struct Document {
    title: String,
    url: String,
    text: String,
}

trait WebSearch {
    fn search(&self, query: &str, max_results: usize) -> Result<Vec<Document>>;
}

/// Uses the Brave Search API if `BRAVE_API_KEY` is set, otherwise Wikipedia.
fn search_backend() -> Box<dyn WebSearch> {
    let http = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("the HTTP client can always be built with these settings");
    match std::env::var("BRAVE_API_KEY") {
        Ok(key) if !key.is_empty() => {
            println!("  (using the Brave Search API)");
            Box::new(BraveSearch { http, key })
        }
        _ => {
            println!("  (using the Wikipedia search API; set BRAVE_API_KEY for a general web search)");
            Box::new(WikipediaSearch { http })
        }
    }
}

/// Searches English Wikipedia and downloads the plain text of each article found.
struct WikipediaSearch {
    http: reqwest::blocking::Client,
}

#[derive(Deserialize)]
struct WikiSearchResponse {
    query: WikiSearchQuery,
}
#[derive(Deserialize)]
struct WikiSearchQuery {
    search: Vec<WikiSearchHit>,
}
#[derive(Deserialize)]
struct WikiSearchHit {
    title: String,
}
#[derive(Deserialize)]
struct WikiExtractResponse {
    query: WikiExtractQuery,
}
#[derive(Deserialize)]
struct WikiExtractQuery {
    pages: std::collections::HashMap<String, WikiPage>,
}
#[derive(Deserialize)]
struct WikiPage {
    extract: Option<String>,
}

const WIKIPEDIA_API: &str = "https://en.wikipedia.org/w/api.php";

impl WebSearch for WikipediaSearch {
    fn search(&self, query: &str, max_results: usize) -> Result<Vec<Document>> {
        let hits: WikiSearchResponse = self
            .http
            .get(WIKIPEDIA_API)
            .query(&[
                ("action", "query"),
                ("list", "search"),
                ("format", "json"),
                ("srsearch", query),
                ("srlimit", &max_results.to_string()),
            ])
            .send()?
            .error_for_status()?
            .json()
            .context("unexpected answer from the Wikipedia search")?;

        let mut documents = Vec::new();
        for hit in hits.query.search {
            // Multiple full extracts per request aren't allowed, so each page is fetched alone.
            let page: WikiExtractResponse = self
                .http
                .get(WIKIPEDIA_API)
                .query(&[
                    ("action", "query"),
                    ("prop", "extracts"),
                    ("explaintext", "1"),
                    ("format", "json"),
                    ("titles", &hit.title),
                ])
                .send()?
                .error_for_status()?
                .json()
                .with_context(|| format!("unexpected answer for the page \"{}\"", hit.title))?;
            let Some(raw) = page.query.pages.into_values().find_map(|p| p.extract) else {
                continue;
            };
            let text = clean_wikipedia_text(&raw);
            if !text.is_empty() {
                let url = format!("https://en.wikipedia.org/wiki/{}", hit.title.replace(' ', "_"));
                documents.push(Document { title: hit.title, url, text });
            }
        }
        Ok(documents)
    }
}

/// Removes section headings and everything from "See also"/"References" on.
fn clean_wikipedia_text(raw: &str) -> String {
    const END_SECTIONS: [&str; 5] = ["See also", "References", "Notes", "External links", "Further reading"];
    let mut paragraphs = Vec::new();
    for line in raw.lines().map(str::trim) {
        if line.starts_with("==") {
            let heading = line.trim_matches('=').trim();
            if END_SECTIONS.contains(&heading) {
                break;
            }
            continue;
        }
        // Very short lines are usually list items or leftovers of tables and formulas.
        if line.len() > 40 {
            paragraphs.push(line);
        }
    }
    truncate_chars(&paragraphs.join("\n"), MAX_CHARS_PER_PAGE)
}

/// Uses the Brave Search API (https://brave.com/search/api/). Its free plan needs an API key.
/// The training text is built from the title, description and extra snippets of each result.
struct BraveSearch {
    http: reqwest::blocking::Client,
    key: String,
}

#[derive(Deserialize)]
struct BraveResponse {
    web: Option<BraveWeb>,
}
#[derive(Deserialize)]
struct BraveWeb {
    results: Vec<BraveResult>,
}
#[derive(Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    extra_snippets: Vec<String>,
}

impl WebSearch for BraveSearch {
    fn search(&self, query: &str, max_results: usize) -> Result<Vec<Document>> {
        let response: BraveResponse = self
            .http
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", &self.key)
            .header("Accept", "application/json")
            .query(&[("q", query), ("count", &max_results.to_string()), ("extra_snippets", "true")])
            .send()?
            .error_for_status()
            .context("the Brave Search API rejected the request (is BRAVE_API_KEY valid?)")?
            .json()?;

        let results = response.web.map(|w| w.results).unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|r| {
                let mut text = format!("{}. {}", strip_html(&r.title), strip_html(&r.description));
                for snippet in &r.extra_snippets {
                    text.push('\n');
                    text.push_str(&strip_html(snippet));
                }
                Document { title: strip_html(&r.title), url: r.url, text: truncate_chars(&text, MAX_CHARS_PER_PAGE) }
            })
            .collect())
    }
}

/// Removes tags like `<strong>` and decodes the most common HTML entities.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// ---------------------------------------------------------------------------
// 2. Training data
// ---------------------------------------------------------------------------

struct ModelFiles {
    config: PathBuf,
    tokenizer: PathBuf,
    weights: PathBuf,
}

/// Downloads the model once; later runs use the Hugging Face cache (`~/.cache/huggingface`).
fn download_model() -> Result<ModelFiles> {
    let client = HFClientSync::new()?;
    let repo = client.model(MODEL_OWNER, MODEL_NAME);
    let get = |file: &str| -> Result<PathBuf> {
        repo.download_file()
            .filename(file)
            .revision(MODEL_REVISION)
            .send()
            .with_context(|| format!("could not download {file} from Hugging Face"))
    };
    Ok(ModelFiles {
        config: get("config.json")?,
        tokenizer: get("tokenizer.json")?,
        weights: get("model.safetensors")?,
    })
}

/// The files of the model saved by `save_model`, used by `--no-finetune`.
fn saved_model_files() -> Result<ModelFiles> {
    let dir = Path::new(OUTPUT_DIR);
    let files = ModelFiles {
        config: dir.join("config.json"),
        tokenizer: dir.join("tokenizer.json"),
        weights: dir.join("model.safetensors"),
    };
    for path in [&files.config, &files.tokenizer, &files.weights] {
        ensure!(
            path.exists(),
            "{} not found: there is no fine-tuned model yet. Run the program once without \
             --no-finetune (in the project directory) to create it.",
            path.display()
        );
    }
    Ok(files)
}

/// GPT-2's end-of-text token, used to separate the documents.
const EOS_TOKEN: u32 = 50256;

/// Tokenizes all documents into one token stream and cuts it into windows of
/// `SEQ_LEN + 1` tokens: the model sees tokens `0..SEQ_LEN` and learns to predict `1..=SEQ_LEN`.
/// Token windows of `SEQ_LEN + 1` tokens each.
type Windows = Vec<Vec<u32>>;

fn build_windows(tokenizer: &Tokenizer, documents: &[Document]) -> Result<(Windows, Windows)> {
    let mut stream = Vec::new();
    for doc in documents {
        let encoding = tokenizer.encode(doc.text.as_str(), false).map_err(anyhow::Error::msg)?;
        stream.extend_from_slice(encoding.get_ids());
        stream.push(EOS_TOKEN);
    }
    println!("  {} tokens from {} documents", stream.len(), documents.len());

    let (mut train, mut valid) = (Vec::new(), Vec::new());
    for (i, window) in stream.windows(SEQ_LEN + 1).step_by(SEQ_LEN).enumerate() {
        if i % VALIDATION_EVERY == VALIDATION_EVERY - 1 {
            valid.push(window.to_vec());
        } else {
            train.push(window.to_vec());
        }
    }
    Ok((train, valid))
}

/// Turns windows into input and target tensors of shape `(batch, SEQ_LEN)`.
fn to_batch(windows: &[&Vec<u32>], device: &Device) -> Result<(Tensor, Tensor)> {
    let inputs: Vec<u32> = windows.iter().flat_map(|w| w[..SEQ_LEN].iter().copied()).collect();
    let targets: Vec<u32> = windows.iter().flat_map(|w| w[1..].iter().copied()).collect();
    let shape = (windows.len(), SEQ_LEN);
    Ok((Tensor::from_vec(inputs, shape, device)?, Tensor::from_vec(targets, shape, device)?))
}

// ---------------------------------------------------------------------------
// 3. Model: GPT-2
// ---------------------------------------------------------------------------
//
// `candle-transformers` contains ready-made models, but they are built for inference and use
// fused kernels that can't be differentiated. This small GPT-2 uses only basic tensor
// operations, so candle can compute the gradients of every weight. The parameter names match
// the Hugging Face checkpoint, so the weights load directly and can be saved back.

#[derive(Deserialize)]
struct Gpt2Config {
    vocab_size: usize,
    n_positions: usize,
    n_embd: usize,
    n_layer: usize,
    n_head: usize,
    layer_norm_epsilon: f64,
}

/// Builds the network and loads the weights from `files`: the pretrained ones from the
/// Hugging Face cache, or the fine-tuned ones from `finetuned-model/`.
fn load_model(files: &ModelFiles, device: &Device) -> Result<(Gpt2, VarMap)> {
    let config: Gpt2Config = serde_json::from_str(&std::fs::read_to_string(&files.config)?)?;
    let mut varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);
    let model = Gpt2::new(&config, vb.pp("transformer"))?;
    // The model was built with placeholder weights; now the saved ones replace them.
    varmap
        .load(&files.weights)
        .with_context(|| format!("could not load the weights from {}", files.weights.display()))?;
    Ok((model, varmap))
}

struct LayerNorm {
    weight: Tensor,
    bias: Tensor,
    eps: f64,
}

impl LayerNorm {
    fn new(size: usize, eps: f64, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            weight: vb.get_with_hints(size, "weight", candle_nn::Init::Const(1.0))?,
            bias: vb.get_with_hints(size, "bias", candle_nn::Init::Const(0.0))?,
            eps,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let mean = x.mean_keepdim(D::Minus1)?;
        let x = x.broadcast_sub(&mean)?;
        let var = x.sqr()?.mean_keepdim(D::Minus1)?;
        let x = x.broadcast_div(&(var + self.eps)?.sqrt()?)?;
        Ok(x.broadcast_mul(&self.weight)?.broadcast_add(&self.bias)?)
    }
}

/// GPT-2 stores its linear layers as "Conv1D" with the weight shape `(in, out)`.
struct Conv1D {
    weight: Tensor,
    bias: Tensor,
}

impl Conv1D {
    fn new(d_in: usize, d_out: usize, vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            weight: vb.get_with_hints((d_in, d_out), "weight", candle_nn::init::DEFAULT_KAIMING_NORMAL)?,
            bias: vb.get_with_hints(d_out, "bias", candle_nn::Init::Const(0.0))?,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        Ok(x.broadcast_matmul(&self.weight)?.broadcast_add(&self.bias)?)
    }
}

struct Attention {
    c_attn: Conv1D,
    c_proj: Conv1D,
    n_head: usize,
}

impl Attention {
    fn forward(&self, x: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (b, t, c) = x.dims3()?;
        let head_dim = c / self.n_head;
        let qkv = self.c_attn.forward(x)?;
        let split = |i: usize| -> Result<Tensor> {
            Ok(qkv
                .narrow(D::Minus1, i * c, c)?
                .reshape((b, t, self.n_head, head_dim))?
                .transpose(1, 2)?
                .contiguous()?)
        };
        let (q, k, v) = (split(0)?, split(1)?, split(2)?);

        let scores = (q.matmul(&k.t()?)? / (head_dim as f64).sqrt())?.broadcast_add(mask)?;
        let weights = ops::softmax(&scores, D::Minus1)?;
        let y = weights.matmul(&v)?.transpose(1, 2)?.reshape((b, t, c))?;
        self.c_proj.forward(&y)
    }
}

struct Block {
    ln_1: LayerNorm,
    attn: Attention,
    ln_2: LayerNorm,
    c_fc: Conv1D,
    c_proj: Conv1D,
}

impl Block {
    fn new(cfg: &Gpt2Config, vb: VarBuilder) -> Result<Self> {
        let c = cfg.n_embd;
        Ok(Self {
            ln_1: LayerNorm::new(c, cfg.layer_norm_epsilon, vb.pp("ln_1"))?,
            attn: Attention {
                c_attn: Conv1D::new(c, 3 * c, vb.pp("attn.c_attn"))?,
                c_proj: Conv1D::new(c, c, vb.pp("attn.c_proj"))?,
                n_head: cfg.n_head,
            },
            ln_2: LayerNorm::new(c, cfg.layer_norm_epsilon, vb.pp("ln_2"))?,
            c_fc: Conv1D::new(c, 4 * c, vb.pp("mlp.c_fc"))?,
            c_proj: Conv1D::new(4 * c, c, vb.pp("mlp.c_proj"))?,
        })
    }

    fn forward(&self, x: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let x = (x + self.attn.forward(&self.ln_1.forward(x)?, mask)?)?;
        // `gelu` in candle is the tanh approximation, which is GPT-2's "gelu_new".
        let mlp = self.c_proj.forward(&self.c_fc.forward(&self.ln_2.forward(&x)?)?.gelu()?)?;
        Ok((x + mlp)?)
    }
}

struct Gpt2 {
    wte: Tensor,
    wpe: Tensor,
    blocks: Vec<Block>,
    ln_f: LayerNorm,
    n_positions: usize,
}

impl Gpt2 {
    fn new(cfg: &Gpt2Config, vb: VarBuilder) -> Result<Self> {
        let init = candle_nn::Init::Randn { mean: 0.0, stdev: 0.02 };
        Ok(Self {
            wte: vb.get_with_hints((cfg.vocab_size, cfg.n_embd), "wte.weight", init)?,
            wpe: vb.get_with_hints((cfg.n_positions, cfg.n_embd), "wpe.weight", init)?,
            blocks: (0..cfg.n_layer)
                .map(|i| Block::new(cfg, vb.pp(format!("h.{i}"))))
                .collect::<Result<_>>()?,
            ln_f: LayerNorm::new(cfg.n_embd, cfg.layer_norm_epsilon, vb.pp("ln_f"))?,
            n_positions: cfg.n_positions,
        })
    }

    /// Returns the logits of shape `(batch, seq, vocab)` for token ids of shape `(batch, seq)`.
    fn forward(&self, ids: &Tensor) -> Result<Tensor> {
        let (_b, t) = ids.dims2()?;
        ensure!(t <= self.n_positions, "sequence of {t} tokens is longer than {}", self.n_positions);
        let device = ids.device();

        let positions = Tensor::arange(0u32, t as u32, device)?;
        let tok = self.wte.embedding(&ids.flatten_all()?)?.reshape((ids.dim(0)?, t, ()))?;
        let pos = self.wpe.embedding(&positions)?;
        let mut x = tok.broadcast_add(&pos)?;

        // Causal mask: every token may only look at itself and earlier tokens.
        let mask: Vec<f32> = (0..t)
            .flat_map(|i| (0..t).map(move |j| if j > i { -1e9 } else { 0.0 }))
            .collect();
        let mask = Tensor::from_vec(mask, (t, t), device)?;

        for block in &self.blocks {
            x = block.forward(&x, &mask)?;
        }
        let x = self.ln_f.forward(&x)?;
        // The output layer shares its weights with the token embedding ("tied weights").
        Ok(x.broadcast_matmul(&self.wte.t()?)?)
    }
}

// ---------------------------------------------------------------------------
// 4. Fine-tuning
// ---------------------------------------------------------------------------

/// Average next-token cross-entropy of a batch.
fn batch_loss(model: &Gpt2, inputs: &Tensor, targets: &Tensor) -> Result<Tensor> {
    let logits = model.forward(inputs)?;
    let (b, t, v) = logits.dims3()?;
    Ok(loss::cross_entropy(&logits.reshape((b * t, v))?, &targets.reshape(b * t)?)?)
}

fn evaluate(model: &Gpt2, windows: &[Vec<u32>], device: &Device) -> Result<f32> {
    let mut total = 0.0;
    for chunk in windows.chunks(BATCH_SIZE) {
        let refs: Vec<&Vec<u32>> = chunk.iter().collect();
        let (inputs, targets) = to_batch(&refs, device)?;
        total += batch_loss(model, &inputs, &targets)?.to_scalar::<f32>()? * chunk.len() as f32;
    }
    Ok(total / windows.len() as f32)
}

/// Trains the transformer layers and keeps the weights of the best epoch.
///
/// With only a few thousand tokens the model memorizes the training text quickly, so
/// two things limit overfitting: the embeddings (`wte`, `wpe`: 39 M of the 82 M parameters)
/// stay frozen, and the weights with the lowest validation loss are restored at the end.
fn train_model(model: &Gpt2, varmap: &VarMap, train: &[Vec<u32>], valid: &[Vec<u32>], device: &Device) -> Result<()> {
    let trainable: Vec<(String, Var)> = varmap
        .data()
        .lock()
        .unwrap()
        .iter()
        .filter(|(name, _)| !name.ends_with("wte.weight") && !name.ends_with("wpe.weight"))
        .map(|(name, var)| (name.clone(), var.clone()))
        .collect();
    let n_trainable: usize = trainable.iter().map(|(_, v)| v.elem_count()).sum();
    println!("  training {:.1} M parameters, embeddings frozen", n_trainable as f64 / 1e6);

    let params = ParamsAdamW { lr: LEARNING_RATE, weight_decay: 0.01, ..Default::default() };
    let mut optimizer = AdamW::new(trainable.iter().map(|(_, v)| v.clone()).collect(), params)?;
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut order: Vec<usize> = (0..train.len()).collect();

    // `copy` makes a real copy; a plain clone would share the storage with the Var.
    let snapshot = || -> Result<Vec<Tensor>> { Ok(trainable.iter().map(|(_, v)| v.as_tensor().copy()).collect::<candle_core::Result<Vec<_>>>()?) };
    let mut best_loss = evaluate(model, valid, device)?;
    let mut best_weights = snapshot()?;

    for epoch in 1..=EPOCHS {
        let start = Instant::now();
        order.shuffle(&mut rng);
        let mut total = 0.0;
        for batch in order.chunks(BATCH_SIZE) {
            let windows: Vec<&Vec<u32>> = batch.iter().map(|&i| &train[i]).collect();
            let (inputs, targets) = to_batch(&windows, device)?;
            let loss = batch_loss(model, &inputs, &targets)?;
            optimizer.backward_step(&loss)?;
            total += loss.to_scalar::<f32>()? * batch.len() as f32;
        }
        let train_loss = total / train.len() as f32;
        let valid_loss = evaluate(model, valid, device)?;
        if !valid_loss.is_finite() {
            bail!("the training diverged; try a smaller LEARNING_RATE");
        }
        let improved = valid_loss < best_loss;
        if improved {
            best_loss = valid_loss;
            best_weights = snapshot()?;
        }
        println!(
            "  epoch {epoch}/{EPOCHS}: training loss {train_loss:.3}, validation loss {valid_loss:.3}{} ({:.0} s)",
            if improved { " (best so far)" } else { "" },
            start.elapsed().as_secs_f32()
        );
    }

    for ((_, var), weight) in trainable.iter().zip(&best_weights) {
        var.set(weight)?;
    }
    println!("  restored the weights with the best validation loss ({best_loss:.3})");
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Generation and saving
// ---------------------------------------------------------------------------

/// Continues the prompt with greedy decoding. A repetition penalty keeps the small model
/// from looping on the same phrase.
fn generate(model: &Gpt2, tokenizer: &Tokenizer, prompt: &str, device: &Device) -> Result<String> {
    const REPETITION_PENALTY: f32 = 1.3;
    let mut ids = tokenizer.encode(prompt, false).map_err(anyhow::Error::msg)?.get_ids().to_vec();
    let prompt_len = ids.len();

    for _ in 0..GENERATED_TOKENS {
        let input = Tensor::new(ids.as_slice(), device)?.unsqueeze(0)?;
        let logits = model.forward(&input)?.squeeze(0)?.get(ids.len() - 1)?;
        let mut logits: Vec<f32> = logits.to_vec1()?;
        for &id in &ids {
            let l = &mut logits[id as usize];
            *l = if *l > 0.0 { *l / REPETITION_PENALTY } else { *l * REPETITION_PENALTY };
        }
        let next = logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i as u32)
            .expect("the vocabulary is not empty");
        if next == EOS_TOKEN {
            break;
        }
        ids.push(next);
    }
    let text = tokenizer.decode(&ids[prompt_len..], true).map_err(anyhow::Error::msg)?;
    Ok(format!("{prompt}{}", text.replace('\n', " ")))
}

/// Prints the `k` most likely next tokens after the prompt with their probabilities.
/// This is the distribution that `generate` picks from in every step.
fn print_next_token_probabilities(model: &Gpt2, tokenizer: &Tokenizer, prompt: &str, k: usize, device: &Device) -> Result<()> {
    let ids = tokenizer.encode(prompt, false).map_err(anyhow::Error::msg)?.get_ids().to_vec();
    let input = Tensor::new(ids.as_slice(), device)?.unsqueeze(0)?;
    let logits = model.forward(&input)?.squeeze(0)?.get(ids.len() - 1)?;
    // softmax turns the 50,257 logits into probabilities that add up to 1.
    let probs: Vec<f32> = ops::softmax(&logits, 0)?.to_vec1()?;
    let mut ranked: Vec<(usize, f32)> = probs.into_iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("  next token after \"{prompt}\":");
    for (id, p) in ranked.into_iter().take(k) {
        let token = tokenizer.decode(&[id as u32], false).map_err(anyhow::Error::msg)?;
        println!("    {:>5.1} %  {token:?}", p * 100.0);
    }
    Ok(())
}

/// Writes the weights, config and tokenizer, so that the directory is a complete
/// Hugging Face model (`AutoModelForCausalLM.from_pretrained("finetuned-model")`).
fn save_model(varmap: &VarMap, files: &ModelFiles) -> Result<()> {
    let dir = Path::new(OUTPUT_DIR);
    std::fs::create_dir_all(dir)?;
    varmap.save(dir.join("model.safetensors"))?;
    std::fs::copy(&files.config, dir.join("config.json"))?;
    std::fs::copy(&files.tokenizer, dir.join("tokenizer.json"))?;
    Ok(())
}
