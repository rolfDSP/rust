# NLP pipelines with rust-bert

An example of the [`rust-bert`](https://github.com/guillaume-be/rust-bert) crate, which runs Hugging Face Transformer models in Rust on libtorch (through the `tch` crate).

`src/main.rs` shows three pipelines:

1. **Sentiment analysis:** DistilBERT fine-tuned on SST-2 labels sentences as positive or negative
2. **Question answering:** DistilBERT (SQuAD) finds the answer to a question inside a text
3. **Sentence embeddings:** all-MiniLM-L6-v2 turns sentences into vectors, ranked against a query by cosine similarity

## Running

```bash
cargo run
```

The first build downloads libtorch (CPU version). The first run downloads about 600 MB of models into `~/.cache/.rustbert`; later runs use the cache.

## Build notes

- `Cargo.toml` turns off the default features and uses `rustls-tls`, so the OpenSSL development headers are not needed.
- `Cargo.lock` pins `console` to 0.15.x. The `indicatif` 0.16 crate that `rust-bert` uses for download progress bars does not compile with `console` 0.16. Running `cargo update` without care brings the error back.

## Other pipelines

The `rust_bert::pipelines` module has more, used the same way (`XxxModel::new(Default::default())?`): `ner`, `summarization`, `translation`, `zero_shot_classification`, `text_generation`, `keywords_extraction`, `pos_tagging`. Some of their default models are larger than 1 GB.
