#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

use std::collections::HashMap;
use std::env;
use std::error;
use std::fs;
use std::io;
use std::time;

use quick_xml::de::from_reader;
use rust_stemmers::{Algorithm, Stemmer};
use serde::Deserialize;

const STOP_WORDS: [&str; 10] = [
    "a", "and", "be", "have", "i", "in", "of", "that", "the", "to",
];

pub fn run(config: &Config) -> Result<(), Box<dyn error::Error>> {
    let index_start = time::Instant::now();
    let index = Index::new(&config.db_path)?;
    let indexing_time = index_start.elapsed().as_secs();

    let search_start = time::Instant::now();
    let results = index.search(&config.query);
    let search_time = search_start.elapsed().as_micros();

    println!("{:#?}", results);
    println!("Number of results: {}", results.len());
    println!("Total number of indexed documents: {}", index.index.len());
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
    index: HashMap<String, Vec<u32>>,
    stemmer: Stemmer,
}

impl Index {
    pub fn new(db_path: &str) -> Result<Self, Box<dyn error::Error>> {
        let file = fs::File::open(db_path)?;
        let reader = io::BufReader::new(file);
        let docs: Docs = from_reader(reader)?;

        let index = HashMap::new();
        let stemmer = Stemmer::create(Algorithm::English);

        let mut index = Self { index, stemmer };
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

    pub fn search(&self, query: &str) -> Vec<u32> {
        let tokens = self.tokenize(query);
        let mut results = Vec::new();
        for token in tokens {
            if let Some(indexes) = self.index.get(&token) {
                results = [results, indexes.to_vec()].concat()
            }
        }
        results
    }

    pub fn add(&mut self, doc: &Document) {
        let tokens = self.tokenize(&doc.text);

        for token in tokens {
            let docs_containing_token: Vec<u32> = if let Some(existing) = self.index.get(&token) {
                if existing.contains(&doc.id) {
                    existing.clone()
                } else {
                    let mut tmp = existing.clone();
                    tmp.push(doc.id);
                    tmp.to_vec()
                }
            } else {
                let mut tmp = Vec::new();
                tmp.push(doc.id);
                tmp.to_vec()
            };

            self.index.insert(token, docs_containing_token);
        }
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter_map(|w| {
                if STOP_WORDS.contains(&w) {
                    None
                } else {
                    Some(self.stemmer.stem(w).into_owned())
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
