# Documentation

This project trains a spam classifier for text messages in pure Rust, on a dataset from Hugging Face. The sections below cover what you need installed, how to use the project, how the code works, and what to do when something goes wrong.

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [How to use it](#how-to-use-it)
- [How the code works](#how-the-code-works)
- [Troubleshooting](#troubleshooting)
- [Limits of this example](#limits-of-this-example)

## Overview

Everything happens in one program, `src/main.rs`:

```text
download  ->  read  ->  remove duplicates  ->  split  ->  TF-IDF  ->  train  ->  evaluate
hf-hub        parquet                                    own code    candle
```

| Crate | What it does here |
| --- | --- |
| `hf-hub` | Downloads the dataset and keeps it in the Hugging Face cache |
| `parquet` | Reads the downloaded file, row by row |
| `candle-core` | Tensors and automatic differentiation |
| `candle-nn` | Linear layers, the loss function and the optimizer |
| `rand` | Seeded random numbers for the split, the weights and the batch order |
| `anyhow` | Error handling |

**Candle** is Hugging Face's machine learning framework for Rust. Unlike `tch` in `r_proj_065`, it doesn't need the libtorch library, so the finished program is a normal binary without extra libraries to install or find.

**The dataset** is [`ucirvine/sms_spam`](https://huggingface.co/datasets/ucirvine/sms_spam). It has one split and two columns: `sms`, the text, and `label`, which is 0 for ham and 1 for spam. The dataset card on Hugging Face lists the license as "unknown", so check the terms before using the data for anything beyond learning.

## Prerequisites

- **Rust.** The project was tested with Rust 1.99. The `parquet` crate requires at least 1.88, and other dependencies may need a newer version.
- **C and C++ build tools**, because some dependencies compile C code: `sudo apt install build-essential` on Ubuntu. Install CMake too if the build reports that it's missing.
- **An internet connection** for the first build and the first run.
- **About 1 GB of disk space** for a release build. The dataset itself takes less than 1 MB.

No Hugging Face account, Python or GPU is needed.

## How to use it

```bash
cargo run --release
```

Always use `--release`. A debug build works, but the numeric code runs far more slowly without optimizations.

The program prints its four steps. To try your own texts, edit the `new_messages` array in `main`. The settings, such as the size of the network and the learning rate, are constants at the top of `src/main.rs`.

The dataset is stored in `~/.cache/huggingface/hub/datasets--ucirvine--sms_spam`, so it's downloaded only once. To download it again, delete that directory.

## How the code works

### Loading the data

`download_dataset` asks `hf-hub` for one file of the dataset. The request names a fixed commit of the dataset instead of the `main` branch, so the data can't change without you noticing. To use a newer version, change `DATASET_REVISION`.

`read_messages` uses the row-based reader of the `parquet` crate. It's the simplest of the crate's interfaces, and it's fast enough for a file this small.

### Duplicates and the split

This dataset has 414 repeated messages: 309 ham and 105 spam. `remove_duplicates` drops them, leaving 5,160. If a message were in both the training set and the test set, the test would partly measure memory, and the score would look better than it really is.

`split_dataset` then creates three sets, with the same share of spam in each:

| Set | Share | Used for |
| --- | --- | --- |
| Training | 70 % | Adjusting the weights |
| Validation | 15 % | Deciding when to stop training |
| Test | 15 % | The final score, once, at the very end |

### From text to numbers

A network can only process numbers, so `Vectorizer` turns each message into a TF-IDF vector with one entry per known word. A word's entry is larger the more often it's in the message, and the rarer it is overall. Common words like "the" count for little, while rare and telling words count for more. Each vector is then scaled to length 1.

Some details:

- `Vectorizer::fit` sees only the training set. Otherwise, knowledge about the test set would leak into the model.
- `tokenize` replaces all tokens containing a digit with `<num>`. Exact phone numbers rarely repeat, but the presence of a number is a strong sign of spam.
- Word order is lost, so "not free" and "free not" give the same vector.

### The network

```text
TF-IDF vector (2,752)  ->  linear  ->  ReLU (32)  ->  linear  ->  logits (2)
```

This has 88,162 trainable parameters. Nearly all of them are in the first layer, which has one weight per word and hidden unit.

`new_linear` creates the layers by hand instead of through candle's `VarBuilder`. Candle can't seed its random numbers on the CPU, so its own initialization would differ on each run. With weights from our seeded generator, a run on the same machine can be repeated exactly.

### Training

`train` runs mini-batch training with the AdamW optimizer and cross-entropy loss. After each epoch, it measures the loss on the validation set and copies the weights if they are the best so far. When five epochs bring no improvement, it stops and restores the best weights. This is called early stopping.

In the output, the training loss keeps falling while the validation loss levels off. From that point on, the model is learning details of the training messages that don't carry over to new ones, which is why the validation loss decides when to stop.

### Evaluation

`Evaluation` counts the four possible outcomes, with spam as the positive class:

| Metric | Question it answers | Result |
| --- | --- | --- |
| Accuracy | How many answers were right? | 97.7 % |
| Precision | Of the messages flagged as spam, how many were spam? | 93.4 % |
| Recall | Of the spam messages, how many were found? | 87.6 % |
| F1 score | How good are precision and recall combined? | 0.904 |

Only 12.5 % of the test messages are spam, so always answering "ham" would already reach 87.5 % accuracy while finding no spam at all. That's why the program prints this baseline, and why precision and recall tell you more than accuracy.

The test set has 97 spam messages, so one message more or less changes the recall by about one percentage point. Treat the numbers as estimates, and don't read much into small differences.

### Why the program disables the stored token

The first line of `main` sets `HF_HUB_DISABLE_IMPLICIT_TOKEN`. Normally, `hf-hub` sends any Hugging Face login it finds on the machine with its requests. If that login has expired, the Hub rejects the request, even for a public dataset. This dataset needs no login, so the program doesn't send one. This doesn't change or delete the stored login.

If you switch to a private or gated dataset, remove this line and log in.

## Troubleshooting

**The build fails with an error about CMake or a C compiler**

Install the build tools with `sudo apt install build-essential cmake`, then build again.

**`could not download the dataset from Hugging Face`**

The rest of the message gives the reason. The first run needs a connection to `huggingface.co`, so check your network or proxy. If a firewall blocks the site, download the file elsewhere and change `main` to pass its path to `read_messages`.

**`Authentication required: OAuth token has expired`**

This happens if you removed the `HF_HUB_DISABLE_IMPLICIT_TOKEN` line and your stored login has expired. Log in again with `hf auth login`, or put the line back.

**Training is slow**

Check that you used `--release`.

**The numbers differ from the README**

Runs on one machine give the same output, but another CPU, thread count or dependency version can change the rounding or the random numbers, and with them the results. Without the same `Cargo.lock`, a fresh build may also pick newer dependency versions.

## Limits of this example

- **The model isn't saved.** The program trains again on each run. A real application would save the weights and the vocabulary, and candle's `safetensors` support can store the weights.
- **The data is old and narrow.** The collection was published in 2011, with messages largely from the UK and Singapore. The model knows that era's spam, not today's phishing links, and it handles only English. A good score here doesn't mean it would work well on your phone.
- **Words only.** TF-IDF ignores word order and meaning. A pretrained model like DistilBERT would probably do better, at the cost of a much larger download.
- **Ham and spam errors have the same cost here.** A real spam filter would rather let spam through than hide a real message, so it would use a higher threshold than the plain larger-logit decision in `predict`.
- **CPU only.** The program doesn't use a GPU. This dataset doesn't need one.
