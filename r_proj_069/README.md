# Fine-tuning a Hugging Face LLM with web search results

A pure-Rust example: it searches the web for a topic, uses the text of the results as training
data and fine-tunes the language model [`distilbert/distilgpt2`](https://huggingface.co/distilbert/distilgpt2)
(82 M parameters) from Hugging Face on it. Python is not needed.

For a detailed explanation of the network layers, the training and how the next token
is generated, see [DOCUMENTATION.md](DOCUMENTATION.md).

```text
web search  --->  cleaned text  --->  token windows  --->  distilgpt2  --->  fine-tuned model
reqwest           own code            tokenizers          candle: train      finetuned-model/
```

## Running

```bash
cargo run --release                                    # topic "Rust programming language"
cargo run --release -- "James Webb Space Telescope" "The James Webb Space Telescope"
BRAVE_API_KEY=... cargo run --release -- "your topic" "your prompt"
```

The first argument is the search topic, the second the prompt that the model completes
before and after the training.

To generate text with the model fine-tuned by the last run, without searching and training
again (a few seconds instead of minutes):

```bash
cargo run --release -- --no-finetune "The borrow checker"
```

- **Search:** without a key, the program uses the free Wikipedia search API and downloads the
  plain text of the articles it finds. With `BRAVE_API_KEY` set, it uses the
  [Brave Search API](https://brave.com/search/api/) and trains on the result snippets.
  Other search engines can be added by implementing the `WebSearch` trait.
- **Model:** downloaded once into the Hugging Face cache (about 350 MB).
- **Time:** about 5 minutes on a 32-thread CPU (4 epochs of about 70 s).

## What the program does

1. **Web search:** the results become documents; Wikipedia headings and the reference
   sections are removed, Brave's HTML markup is stripped.
2. **Training data:** all documents are tokenized into one stream and cut into windows of
   128 tokens. Every 10th window is kept back for validation.
3. **Model:** `src/main.rs` contains a small GPT-2 built from basic `candle` tensor
   operations. The models in `candle-transformers` use fused kernels that can't be
   differentiated, so they can't be trained. The parameter names match the Hugging Face
   checkpoint, so `VarMap::load` reads the pretrained weights directly.
4. **Fine-tuning:** AdamW, learning rate 5e-5, 4 epochs. The embeddings stay frozen and the
   weights of the epoch with the lowest validation loss are kept. With only ~8,000 tokens the
   model starts memorizing the training text after one or two epochs.
5. **Result:** validation loss, perplexity and a generated text before and after; the model
   is saved to `finetuned-model/` (`model.safetensors`, `config.json`, `tokenizer.json`), which
   Python's `transformers` can load with `AutoModelForCausalLM.from_pretrained("finetuned-model")`.

## Result (topic "Rust programming language")

```text
validation loss  3.949 -> 3.836
perplexity       51.9 -> 46.3

Before: Rust is a programming language that has been used for over 20 years. It's the most
        popular and widely adopted languages in modern times, ...
After:  Rust is a programming language that uses the Rust compiler to compile and run code.
        It was developed by Martin Brink in 2009, ...
```

The model takes on the vocabulary and style of the source text (compiler, open source project,
Mozilla), but a model this small doesn't reliably learn facts from a few pages: "Martin Brink"
is invented. Fine-tuning changes *how* a model writes; to answer questions about current
search results reliably, put the search results into the prompt instead (retrieval-augmented
generation, RAG).

## Ideas for extensions

- More text: more search results or several related queries.
- A bigger model, e.g. `gpt2-medium`; on a GPU with candle's `cuda` feature.
- LoRA: train small low-rank matrices instead of whole weight matrices.
