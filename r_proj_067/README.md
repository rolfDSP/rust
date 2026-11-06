# SMS Spam Classifier with Hugging Face Data

Train a text classifier in pure Rust on a dataset from [Hugging Face](https://huggingface.co/). The program downloads the data, trains a small neural network, tests it, and then classifies a few new messages. Python is not needed.

```text
Hugging Face Hub  --->  Parquet file  --->  TF-IDF vectors  --->  neural network  --->  ham or spam
hf-hub: download        parquet: read       own code              candle: train
```

The data is [`ucirvine/sms_spam`](https://huggingface.co/datasets/ucirvine/sms_spam): 5,574 English text messages, each labelled `ham` (normal) or `spam`. It's a single file of about 360 KB.

The Rust program in `src/main.rs` runs four steps:

1. **Loading the data:** downloading the file, removing duplicates, and splitting it into training, validation and test sets
2. **Training:** turning the text into numbers and training until the validation loss stops improving
3. **Evaluating:** measuring the result on test messages the model has never seen
4. **Classifying new messages:** giving the spam probability of a few messages written for this example

## Running

```bash
cargo run --release
```

You need Rust (tested with 1.99), the C and C++ build tools (`sudo apt install build-essential` on Ubuntu), and CMake if the build asks for it. The first build needs an internet connection and about 1 GB of disk space, and the first run downloads the dataset. After that, the program runs without a connection. Training takes about 20 seconds on a 32-thread CPU.

You don't need a Hugging Face account, because the dataset is public.

## Result

The program gives the same output on each run on the same machine. On the 775 test messages:

```text
                   predicted ham  predicted spam
  actual ham                 672               6
  actual spam                 12              85

  Accuracy:  97.7 %
  Baseline:  87.5 % (always answering "ham")
  Precision: 93.4 % (of the messages flagged as spam, how many were)
  Recall:    87.6 % (of the spam messages, how many were found)
  F1 score:  0.904
```

Compare the accuracy with the baseline: since most messages are ham, a program that never finds any spam already reaches 87.5 %. Precision and recall say more here.

For the exact prerequisites, an explanation of the code, troubleshooting and the limits of this example, see [DOCUMENTATION.md](DOCUMENTATION.md).
