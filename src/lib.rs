#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

use std::collections::HashMap;
use std::env;
use std::error;
use std::fs;
use std::io;
use std::time;

use croaring::bitmap::Bitmap;
use quick_xml::de::from_reader;
use rust_stemmers::{Algorithm, Stemmer};
use serde::Deserialize;

const STOP_WORDS: [&str; 127] = [
    "i",
    "me",
    "my",
    "myself",
    "we",
    "our",
    "ours",
    "ourselves",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
    "he",
    "him",
    "his",
    "himself",
    "she",
    "her",
    "hers",
    "herself",
    "it",
    "its",
    "itself",
    "they",
    "them",
    "their",
    "theirs",
    "themselves",
    "what",
    "which",
    "who",
    "whom",
    "this",
    "that",
    "these",
    "those",
    "am",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "have",
    "has",
    "had",
    "having",
    "do",
    "does",
    "did",
    "doing",
    "a",
    "an",
    "the",
    "and",
    "but",
    "if",
    "or",
    "because",
    "as",
    "until",
    "while",
    "of",
    "at",
    "by",
    "for",
    "with",
    "about",
    "against",
    "between",
    "into",
    "through",
    "during",
    "before",
    "after",
    "above",
    "below",
    "to",
    "from",
    "up",
    "down",
    "in",
    "out",
    "on",
    "off",
    "over",
    "under",
    "again",
    "further",
    "then",
    "once",
    "here",
    "there",
    "when",
    "where",
    "why",
    "how",
    "all",
    "any",
    "both",
    "each",
    "few",
    "more",
    "most",
    "other",
    "some",
    "such",
    "no",
    "nor",
    "not",
    "only",
    "own",
    "same",
    "so",
    "than",
    "too",
    "very",
    "s",
    "t",
    "can",
    "will",
    "just",
    "don",
    "should",
    "now",
];

pub fn run(config: &Config) -> Result<(), Box<dyn error::Error>> {
    let index_start = time::Instant::now();
    let index = Index::new(&config.db_path)?;
    let indexing_time = index_start.elapsed().as_secs();

    let search_start = time::Instant::now();
    let results = index.search(&config.query);
    let search_time = search_start.elapsed().as_micros();

    for result in results.iter() {
        println!("{} {}", result, index.documents.get(&result).unwrap());
    }

    println!("Number of results: {}", results.cardinality());
    println!(
        "Total number of indexed documents: {}",
        index.documents.len()
    );
    println!("Total number of indexed tokens: {}", index.index.len());
    println!("Indexing: {}s", indexing_time);
    println!("Search: {}\u{3bc}s", search_time);
    Ok(())
}

pub struct Config {
    pub query: String,
    pub db_path: String,
}

impl Config {
    pub fn new(mut args: env::Args) -> Result<Self, &'static str> {
        args.next();

        let db_path = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a db path"),
        };

        let query = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a query"),
        };

        Ok(Self { query, db_path })
    }
}

pub struct Index {
    index: HashMap<String, Bitmap>,
    documents: HashMap<u32, String>,
    stemmer: Stemmer,
}

impl Index {
    pub fn new(db_path: &str) -> Result<Self, Box<dyn error::Error>> {
        let file = fs::File::open(db_path)?;
        let reader = io::BufReader::new(file);
        let docs: Docs = from_reader(reader)?;

        let index = HashMap::new();
        let stemmer = Stemmer::create(Algorithm::English);
        let documents = HashMap::new();

        let mut index = Self {
            index,
            stemmer,
            documents,
        };

        for (idx, doc) in docs.doc.iter().enumerate() {
            let document = Document {
                title: doc.title.clone(),
                id: idx as u32,
                text: doc.text.clone(),
                url: doc.url.clone(),
            };

            index.add(&document)
        }

        Ok(index)
    }

    pub fn search(&self, query: &str) -> Bitmap {
        let tokens = self.tokenize(query);
        let mut results = Bitmap::create();

        for token in tokens {
            match self.index.get(&token) {
                Some(indexes) => {
                    results = match results.cardinality() {
                        0 => indexes.clone(),
                        _ => results.and(indexes),
                    };
                }
                None => return Bitmap::create(),
            }
        }

        results
    }

    pub fn add(&mut self, doc: &Document) {
        self.documents.insert(doc.id, doc.text.clone());
        let tokens = self.tokenize(&doc.text);

        for token in tokens {
            let docs_containing_token: Bitmap = if let Some(existing) = self.index.get(&token) {
                if existing.contains(doc.id) {
                    existing.clone()
                } else {
                    let mut tmp = existing.clone();
                    tmp.add(doc.id);
                    tmp
                }
            } else {
                let mut tmp = Bitmap::create();
                tmp.add(doc.id);
                tmp
            };

            self.index.insert(token, docs_containing_token);
        }
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter_map(|w| {
                let word: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
                if STOP_WORDS.contains(&word.as_str()) {
                    None
                } else {
                    Some(self.stemmer.stem(&word).into_owned())
                }
            })
            .collect()
    }
}

#[derive(Deserialize, Debug)]
struct Docs {
    doc: Vec<Document>,
}

#[derive(Deserialize, Debug)]
pub struct Document {
    pub title: String,
    pub url: String,
    #[serde(rename = "abstract")]
    pub text: String,
    #[serde(skip)]
    pub id: u32,
}
