//! Examples for the `rust_bert` crate: ready-to-use NLP pipelines
//! (Hugging Face Transformers models running on libtorch via `tch`).
//!
//! On the first run the models are downloaded from the Hugging Face Hub
//! and cached in `~/.cache/.rustbert`.

use anyhow::Result;
use rust_bert::pipelines::question_answering::{QaInput, QuestionAnsweringModel};
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModelType,
};
use rust_bert::pipelines::sentiment::{SentimentModel, SentimentPolarity};

fn main() -> Result<()> {
    sentiment_analysis()?;
    question_answering()?;
    semantic_similarity()?;
    Ok(())
}

/// 1. Sentiment analysis with DistilBERT fine-tuned on SST-2.
fn sentiment_analysis() -> Result<()> {
    println!("=== 1. Sentiment analysis ===");
    // `Default::default()` selects the standard model (distilbert-base-uncased-finetuned-sst-2-english).
    let model = SentimentModel::new(Default::default())?;

    let texts = [
        "Rust's borrow checker saved me from a nasty bug today. Love it!",
        "The build took forever and then failed with a cryptic linker error.",
        "This crate is okay, nothing special.",
    ];

    for (text, s) in texts.iter().zip(model.predict(texts)) {
        let label = match s.polarity {
            SentimentPolarity::Positive => "POSITIVE",
            SentimentPolarity::Negative => "NEGATIVE",
        };
        println!("{label:<8} ({:.3})  {text}", s.score);
    }
    println!();
    Ok(())
}

/// 2. Extractive question answering: the model finds the answer span in a context.
fn question_answering() -> Result<()> {
    println!("=== 2. Question answering ===");
    // Default: distilbert-base-cased-distilled-squad.
    let model = QuestionAnsweringModel::new(Default::default())?;

    let context = "Rust is a general-purpose programming language emphasizing performance, \
        type safety and concurrency. It was originally designed by Graydon Hoare at \
        Mozilla Research. Rust 1.0 was released in May 2015. The language enforces \
        memory safety without a garbage collector by using a borrow checker.";

    let questions = [
        "Who designed Rust?",
        "When was Rust 1.0 released?",
        "How does Rust achieve memory safety?",
    ];

    let inputs: Vec<QaInput> = questions
        .iter()
        .map(|q| QaInput {
            question: q.to_string(),
            context: context.to_string(),
        })
        .collect();

    // Arguments: inputs, top_k answers per question, batch size.
    let answers = model.predict(&inputs, 1, 8);
    for (q, a) in questions.iter().zip(answers) {
        let best = &a[0];
        println!("Q: {q}\nA: {}  (score {:.3})", best.answer, best.score);
    }
    println!();
    Ok(())
}

/// 3. Sentence embeddings: turn sentences into vectors and compare them by cosine similarity.
fn semantic_similarity() -> Result<()> {
    println!("=== 3. Semantic similarity (sentence embeddings) ===");
    let model = SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL6V2)
        .create_model()?;

    let query = "How do I handle errors in Rust?";
    let documents = [
        "Use the Result type and the ? operator to propagate failures.",
        "Tokio is an asynchronous runtime for Rust.",
        "The weather in Berlin is sunny today.",
        "panic! aborts the current thread when something unrecoverable happens.",
    ];

    let query_emb = &model.encode(&[query])?[0];
    let doc_embs = model.encode(&documents)?;

    let mut ranked: Vec<(f32, &str)> = doc_embs
        .iter()
        .zip(documents)
        .map(|(e, d)| (cosine(query_emb, e), d))
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));

    println!("Query: {query}");
    for (score, doc) in ranked {
        println!("  {score:.3}  {doc}");
    }
    Ok(())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}
