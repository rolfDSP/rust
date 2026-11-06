//! Trains a spam classifier for SMS messages on a dataset from Hugging Face.
//!
//! The dataset is a single Parquet file of about 360 KB. `hf-hub` downloads it
//! once and keeps it in the Hugging Face cache, and `candle` (Hugging Face's
//! machine learning framework for Rust) trains a small neural network on it.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use candle_core::{D, DType, Device, Tensor, Var};
use candle_nn::{AdamW, Linear, Module, Optimizer, ParamsAdamW, loss, ops};
use hf_hub::HFClientSync;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::RowAccessor;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};

const DATASET_OWNER: &str = "ucirvine";
const DATASET_NAME: &str = "sms_spam";
const DATASET_FILE: &str = "plain_text/train-00000-of-00001.parquet";
/// A fixed commit, so that the data can't change between two runs.
const DATASET_REVISION: &str = "cae486f927c250fe1d4a5b55f11357964ed1646c";

const SEED: u64 = 42;
const MAX_VOCABULARY: usize = 3000;
/// Words in fewer training messages than this are ignored.
const MIN_DOCUMENT_FREQUENCY: usize = 2;
const HIDDEN_SIZE: usize = 32;
const BATCH_SIZE: usize = 64;
const LEARNING_RATE: f64 = 0.001;
const WEIGHT_DECAY: f64 = 0.01;
const MAX_EPOCHS: usize = 50;
/// Training stops after this many epochs without a better validation loss.
const PATIENCE: usize = 5;

const SPAM: usize = 1;
const CLASS_NAMES: [&str; 2] = ["ham", "spam"];

/// One labelled text message. The label is 0 for ham and 1 for spam.
struct Message {
    text: String,
    label: usize,
}

/// Downloads the dataset, or finds it in the local cache, and returns its path.
fn download_dataset() -> Result<PathBuf> {
    let client = HFClientSync::new()?;
    let path = client
        .dataset(DATASET_OWNER, DATASET_NAME)
        .download_file()
        .filename(DATASET_FILE)
        .revision(DATASET_REVISION)
        .send()
        .context("could not download the dataset from Hugging Face")?;
    Ok(path)
}

/// Reads the `sms` and `label` columns, which are in that order in the file.
fn read_messages(path: &Path) -> Result<Vec<Message>> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let mut messages = Vec::new();
    for row in reader.get_row_iter(None)? {
        let row = row?;
        messages.push(Message {
            text: row.get_string(0)?.trim().to_string(),
            label: usize::try_from(row.get_long(1)?)?,
        });
    }
    Ok(messages)
}

/// Removes repeated messages. If a message were in both the training and the
/// test set, the test would partly measure memory instead of generalization.
fn remove_duplicates(messages: Vec<Message>) -> Vec<Message> {
    let mut seen = HashSet::new();
    messages
        .into_iter()
        .filter(|message| seen.insert(message.text.clone()))
        .collect()
}

/// Splits into 70 % training, 15 % validation and 15 % test data, separately
/// for each class, so that all three sets have the same share of spam.
fn split_dataset(
    messages: Vec<Message>,
    rng: &mut StdRng,
) -> (Vec<Message>, Vec<Message>, Vec<Message>) {
    let (mut train, mut validation, mut test) = (Vec::new(), Vec::new(), Vec::new());
    let (ham, spam): (Vec<_>, Vec<_>) = messages.into_iter().partition(|m| m.label != SPAM);
    for mut class in [ham, spam] {
        class.shuffle(rng);
        let validation_start = class.len() * 70 / 100;
        let test_start = class.len() * 85 / 100;
        test.extend(class.drain(test_start..));
        validation.extend(class.drain(validation_start..));
        train.append(&mut class);
    }
    (train, validation, test)
}

/// Splits a message into lowercase words. All tokens with a digit in them
/// become `<num>`: the exact phone number doesn't matter, but that a message
/// contains one does.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| {
            if token.chars().any(|c| c.is_ascii_digit()) {
                "<num>".to_string()
            } else {
                token.to_string()
            }
        })
        .collect()
}

/// Turns text into TF-IDF vectors, the numeric input of the network. Each
/// vector has one entry per known word. A word counts for more the more often
/// it's in the message, and the rarer it is across all messages.
struct Vectorizer {
    /// Maps a word to its position in the vector.
    word_index: HashMap<String, usize>,
    /// The inverse document frequency of each word.
    idf: Vec<f32>,
}

impl Vectorizer {
    /// Learns the vocabulary. This must only see the training data, or
    /// knowledge about the test set would leak into the model.
    fn fit(messages: &[Message]) -> Self {
        let mut document_frequency: HashMap<String, usize> = HashMap::new();
        for message in messages {
            let unique: HashSet<String> = tokenize(&message.text).into_iter().collect();
            for token in unique {
                *document_frequency.entry(token).or_default() += 1;
            }
        }

        let mut words: Vec<(String, usize)> = document_frequency
            .into_iter()
            .filter(|(_, count)| *count >= MIN_DOCUMENT_FREQUENCY)
            .collect();
        // The word itself breaks ties, because a HashMap has no stable order.
        words.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        words.truncate(MAX_VOCABULARY);

        let total = messages.len() as f32;
        let idf = words
            .iter()
            .map(|(_, count)| ((1.0 + total) / (1.0 + *count as f32)).ln() + 1.0)
            .collect();
        let word_index = words
            .into_iter()
            .enumerate()
            .map(|(index, (word, _))| (word, index))
            .collect();
        Self { word_index, idf }
    }

    fn vocabulary_size(&self) -> usize {
        self.idf.len()
    }

    /// Returns one row of shape `[vocabulary_size]`, scaled to length 1, so
    /// that long messages don't get larger inputs than short ones.
    fn vectorize(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; self.vocabulary_size()];
        for token in tokenize(text) {
            if let Some(&index) = self.word_index.get(&token) {
                vector[index] += self.idf[index];
            }
        }
        let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if length > 0.0 {
            vector.iter_mut().for_each(|x| *x /= length);
        }
        vector
    }

    /// Returns a tensor of shape `[texts, vocabulary_size]`.
    fn vectorize_all<'a>(
        &self,
        texts: impl ExactSizeIterator<Item = &'a str>,
        device: &Device,
    ) -> Result<Tensor> {
        let rows = texts.len();
        let values: Vec<f32> = texts.flat_map(|text| self.vectorize(text)).collect();
        Ok(Tensor::from_vec(
            values,
            (rows, self.vocabulary_size()),
            device,
        )?)
    }
}

/// A neural network with one hidden layer: TF-IDF vector -> ReLU -> 2 logits.
struct Classifier {
    hidden: Linear,
    output: Linear,
}

impl Classifier {
    /// Creates the layers and returns their trainable variables as well.
    fn new(inputs: usize, device: &Device, rng: &mut StdRng) -> Result<(Self, Vec<Var>)> {
        let mut variables = Vec::new();
        let hidden = new_linear(inputs, HIDDEN_SIZE, device, rng, &mut variables)?;
        let output = new_linear(HIDDEN_SIZE, CLASS_NAMES.len(), device, rng, &mut variables)?;
        Ok((Self { hidden, output }, variables))
    }

    /// Returns one pair of logits per row of `features`.
    fn forward(&self, features: &Tensor) -> Result<Tensor> {
        let hidden = self.hidden.forward(features)?.relu()?;
        Ok(self.output.forward(&hidden)?)
    }
}

/// Creates a linear layer with random weights. `candle` can't seed its own
/// random numbers on the CPU, so these come from our seeded generator instead,
/// which makes a run repeatable.
fn new_linear(
    inputs: usize,
    outputs: usize,
    device: &Device,
    rng: &mut StdRng,
    variables: &mut Vec<Var>,
) -> Result<Linear> {
    let bound = 1.0 / (inputs as f32).sqrt();
    let weights: Vec<f32> = (0..inputs * outputs)
        .map(|_| rng.random_range(-bound..bound))
        .collect();
    let weight = Var::from_vec(weights, (outputs, inputs), device)?;
    let bias = Var::zeros(outputs, DType::F32, device)?;
    let layer = Linear::new(weight.as_tensor().clone(), Some(bias.as_tensor().clone()));
    variables.extend([weight, bias]);
    Ok(layer)
}

/// Texts as tensors, ready for the network.
struct Batch {
    features: Tensor,
    labels: Tensor,
}

impl Batch {
    fn new(messages: &[Message], vectorizer: &Vectorizer, device: &Device) -> Result<Self> {
        let texts = messages.iter().map(|m| m.text.as_str());
        let labels: Vec<u32> = messages.iter().map(|m| m.label as u32).collect();
        Ok(Self {
            features: vectorizer.vectorize_all(texts, device)?,
            labels: Tensor::from_vec(labels, messages.len(), device)?,
        })
    }
}

/// Trains until the validation loss stops improving, then restores the weights
/// of the best epoch.
fn train(
    model: &Classifier,
    variables: &[Var],
    train_set: &Batch,
    validation_set: &Batch,
    rng: &mut StdRng,
) -> Result<()> {
    let params = ParamsAdamW {
        lr: LEARNING_RATE,
        weight_decay: WEIGHT_DECAY,
        ..Default::default()
    };
    let mut optimizer = AdamW::new(variables.to_vec(), params)?;

    let rows = train_set.features.dim(0)?;
    let device = train_set.features.device();
    let mut order: Vec<u32> = (0..rows as u32).collect();
    let mut best_loss = f32::INFINITY;
    let mut best_weights = Vec::new();
    let mut best_epoch = 0;

    println!(
        "{:>5}  {:>10}  {:>15}",
        "epoch", "train loss", "validation loss"
    );
    for epoch in 1..=MAX_EPOCHS {
        order.shuffle(rng);
        let mut loss_sum = 0.0;
        for indices in order.chunks(BATCH_SIZE) {
            let indices = Tensor::from_slice(indices, indices.len(), device)?;
            let features = train_set.features.index_select(&indices, 0)?;
            let labels = train_set.labels.index_select(&indices, 0)?;
            let batch_loss = loss::cross_entropy(&model.forward(&features)?, &labels)?;
            optimizer.backward_step(&batch_loss)?;
            loss_sum += batch_loss.to_scalar::<f32>()? * indices.dim(0)? as f32;
        }

        // Unlike the test set, the validation set may steer the training.
        let logits = model.forward(&validation_set.features)?;
        let validation_loss =
            loss::cross_entropy(&logits, &validation_set.labels)?.to_scalar::<f32>()?;
        println!(
            "{epoch:>5}  {:>10.4}  {validation_loss:>15.4}",
            loss_sum / rows as f32
        );

        if validation_loss < best_loss {
            best_loss = validation_loss;
            best_epoch = epoch;
            best_weights = variables
                .iter()
                .map(|v| Ok(v.as_tensor().copy()?))
                .collect::<Result<_>>()?;
        } else if epoch - best_epoch >= PATIENCE {
            println!("No improvement for {PATIENCE} epochs, stopping.");
            break;
        }
    }

    for (variable, weights) in variables.iter().zip(&best_weights) {
        variable.set(weights)?;
    }
    println!("Using the weights of epoch {best_epoch}.");
    Ok(())
}

/// Returns the predicted class per row.
fn predict(model: &Classifier, features: &Tensor) -> Result<Vec<usize>> {
    let classes = model.forward(features)?.argmax(D::Minus1)?;
    Ok(classes
        .to_vec1::<u32>()?
        .into_iter()
        .map(|class| class as usize)
        .collect())
}

/// The counts of a confusion matrix, with spam as the positive class.
#[derive(Default)]
struct Evaluation {
    true_positive: usize,
    false_positive: usize,
    true_negative: usize,
    false_negative: usize,
}

impl Evaluation {
    fn new(messages: &[Message], predictions: &[usize]) -> Self {
        let mut result = Self::default();
        for (message, &prediction) in messages.iter().zip(predictions) {
            match (message.label == SPAM, prediction == SPAM) {
                (true, true) => result.true_positive += 1,
                (false, true) => result.false_positive += 1,
                (false, false) => result.true_negative += 1,
                (true, false) => result.false_negative += 1,
            }
        }
        result
    }

    fn report(&self) {
        let (tp, fp, tn, fn_) = (
            self.true_positive as f64,
            self.false_positive as f64,
            self.true_negative as f64,
            self.false_negative as f64,
        );
        let total = tp + fp + tn + fn_;
        let precision = tp / (tp + fp);
        let recall = tp / (tp + fn_);
        println!("                   predicted ham  predicted spam");
        println!("  actual ham       {tn:>13}  {fp:>14}");
        println!("  actual spam      {fn_:>13}  {tp:>14}");
        println!();
        println!("  Accuracy:  {:.1} %", 100.0 * (tp + tn) / total);
        println!(
            "  Baseline:  {:.1} % (always answering \"ham\")",
            100.0 * (tn + fp) / total
        );
        println!(
            "  Precision: {:.1} % (of the messages flagged as spam, how many were)",
            100.0 * precision
        );
        println!(
            "  Recall:    {:.1} % (of the spam messages, how many were found)",
            100.0 * recall
        );
        println!(
            "  F1 score:  {:.3}",
            2.0 * precision * recall / (precision + recall)
        );
    }
}

fn main() -> Result<()> {
    // The dataset is public, so don't send a stored Hugging Face login with the
    // request: if that login has expired, the Hub rejects the download.
    // SAFETY: this is the first line of `main`, before any other thread exists.
    unsafe { std::env::set_var("HF_HUB_DISABLE_IMPLICIT_TOKEN", "1") };

    let device = Device::Cpu;
    let mut rng = StdRng::seed_from_u64(SEED);

    println!("== Loading the data ==");
    let path = download_dataset()?;
    println!("File: {}", path.display());
    let all_messages = read_messages(&path)?;
    let total = all_messages.len();
    let messages = remove_duplicates(all_messages);
    ensure!(!messages.is_empty(), "the dataset contains no messages");
    let spam = messages.iter().filter(|m| m.label == SPAM).count();
    println!(
        "{total} messages, {} unique, of which {spam} ({:.1} %) are spam",
        messages.len(),
        100.0 * spam as f64 / messages.len() as f64
    );

    let (train_messages, validation_messages, test_messages) = split_dataset(messages, &mut rng);
    println!(
        "Split: {} training, {} validation, {} test",
        train_messages.len(),
        validation_messages.len(),
        test_messages.len()
    );

    let vectorizer = Vectorizer::fit(&train_messages);
    println!("Vocabulary: {} words", vectorizer.vocabulary_size());
    let train_set = Batch::new(&train_messages, &vectorizer, &device)?;
    let validation_set = Batch::new(&validation_messages, &vectorizer, &device)?;
    let test_set = Batch::new(&test_messages, &vectorizer, &device)?;

    println!("\n== Training ==");
    let (model, variables) = Classifier::new(vectorizer.vocabulary_size(), &device, &mut rng)?;
    let parameters: usize = variables.iter().map(|v| v.as_tensor().elem_count()).sum();
    println!("Network: {parameters} parameters");
    train(&model, &variables, &train_set, &validation_set, &mut rng)?;

    // The test set has played no part so far, so this is a fair estimate.
    println!(
        "\n== Evaluating on the {} test messages ==",
        test_messages.len()
    );
    let predictions = predict(&model, &test_set.features)?;
    Evaluation::new(&test_messages, &predictions).report();

    println!("\n  Messages it got wrong (first 5):");
    let wrong = test_messages
        .iter()
        .zip(&predictions)
        .filter(|(message, prediction)| message.label != **prediction);
    for (message, &prediction) in wrong.take(5) {
        let shortened: String = message.text.chars().take(70).collect();
        println!("  - said {:<4}: {shortened}", CLASS_NAMES[prediction]);
    }

    println!("\n== Classifying new messages ==");
    let new_messages = [
        "Are we still meeting for lunch tomorrow?",
        "WINNER! You have been selected for a free prize. Call 09061234567 now to claim!",
        "Sorry, I'll be late. The train is delayed again.",
        "URGENT: your mobile number won a 2000 cash award. Text CLAIM to 80082.",
    ];
    let features = vectorizer.vectorize_all(new_messages.iter().copied(), &device)?;
    let probabilities = ops::softmax(&model.forward(&features)?, D::Minus1)?.to_vec2::<f32>()?;
    for (text, probability) in new_messages.iter().zip(probabilities) {
        println!("  spam: {:>5.1} %  {text}", 100.0 * probability[SPAM]);
    }
    Ok(())
}
