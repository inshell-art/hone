use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CognitionDoc {
    pub cognition_id: String,
    pub cognition_hash: String,
    pub display_title: Option<String>,
    pub body: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchComponents {
    pub body: f64,
    pub title: f64,
    pub label: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    pub cognition_id: String,
    pub cognition: String,
    pub title: String,
    pub score: f64,
    pub components: MatchComponents,
    pub matched_terms: Vec<String>,
}

pub fn rank_cognitions(query: &str, cognitions: &[CognitionDoc], limit: usize) -> Vec<MatchResult> {
    let query_tokens = tokenize(query);
    let query_set: BTreeSet<_> = query_tokens.iter().cloned().collect();
    if query_set.is_empty() {
        return Vec::new();
    }

    let doc_count = cognitions.len().max(1) as f64;
    let mut doc_freq: BTreeMap<String, usize> = BTreeMap::new();
    let weighted_docs: Vec<Vec<String>> = cognitions
        .iter()
        .map(|cognition| {
            let tokens = tokenize(&format!(
                "{} {} {}",
                cognition.display_title.as_deref().unwrap_or(""),
                cognition.body,
                cognition.state
            ));
            let unique: BTreeSet<_> = tokens.iter().cloned().collect();
            for token in unique {
                *doc_freq.entry(token).or_default() += 1;
            }
            tokens
        })
        .collect();

    let query_tf = term_counts(&query_tokens);
    let mut results = Vec::new();
    for (idx, cognition) in cognitions.iter().enumerate() {
        let title_tokens = tokenize(cognition.display_title.as_deref().unwrap_or(""));
        let body_tokens = tokenize(&cognition.body);
        let label_tokens = tokenize(&cognition.state);
        let doc_tokens = &weighted_docs[idx];
        let doc_tf = term_counts(doc_tokens);
        let doc_set: BTreeSet<_> = doc_tokens.iter().cloned().collect();
        let matched: Vec<String> = query_set.intersection(&doc_set).cloned().collect();
        if matched.is_empty() {
            continue;
        }

        let body = cosine(&query_tf, &term_counts(&body_tokens), &doc_freq, doc_count);
        let title = overlap_score(&query_set, &title_tokens);
        let label = overlap_score(&query_set, &label_tokens);
        let lexical = cosine(&query_tf, &doc_tf, &doc_freq, doc_count);
        let score = (0.70 * body + 0.15 * title + 0.05 * label + 0.10 * lexical).clamp(0.0, 1.0);

        results.push(MatchResult {
            cognition_id: cognition.cognition_id.clone(),
            cognition: cognition.cognition_hash.clone(),
            title: cognition
                .display_title
                .clone()
                .unwrap_or_else(|| title_from_body(&cognition.body)),
            score,
            components: MatchComponents { body, title, label },
            matched_terms: matched,
        });
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.cognition_id.cmp(&b.cognition_id))
    });
    results.truncate(limit);
    results
}

fn title_from_body(body: &str) -> String {
    let line = body
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let mut out: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        out.push_str("...");
    }
    out
}

pub fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .filter(|token| !STOPWORDS.contains(&token.as_str()))
        .collect()
}

fn term_counts(tokens: &[String]) -> BTreeMap<String, f64> {
    let mut counts = BTreeMap::new();
    for token in tokens {
        *counts.entry(token.clone()).or_default() += 1.0;
    }
    counts
}

fn overlap_score(query_set: &BTreeSet<String>, tokens: &[String]) -> f64 {
    let token_set: BTreeSet<_> = tokens.iter().cloned().collect();
    if token_set.is_empty() {
        return 0.0;
    }
    let matches = query_set.intersection(&token_set).count() as f64;
    (matches / token_set.len().max(1) as f64).clamp(0.0, 1.0)
}

fn cosine(
    query_tf: &BTreeMap<String, f64>,
    doc_tf: &BTreeMap<String, f64>,
    doc_freq: &BTreeMap<String, usize>,
    doc_count: f64,
) -> f64 {
    let mut dot = 0.0;
    let mut q_norm = 0.0;
    let mut d_norm = 0.0;

    for (token, q_count) in query_tf {
        let idf = idf(token, doc_freq, doc_count);
        let q_weight = q_count * idf;
        q_norm += q_weight * q_weight;
        if let Some(d_count) = doc_tf.get(token) {
            let d_weight = d_count * idf;
            dot += q_weight * d_weight;
        }
    }

    for (token, d_count) in doc_tf {
        let idf = idf(token, doc_freq, doc_count);
        let d_weight = d_count * idf;
        d_norm += d_weight * d_weight;
    }

    if q_norm == 0.0 || d_norm == 0.0 {
        0.0
    } else {
        (dot / (q_norm.sqrt() * d_norm.sqrt())).clamp(0.0, 1.0)
    }
}

fn idf(token: &str, doc_freq: &BTreeMap<String, usize>, doc_count: f64) -> f64 {
    let df = *doc_freq.get(token).unwrap_or(&0) as f64;
    ((doc_count + 1.0) / (df + 1.0)).ln() + 1.0
}

const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "in", "is", "it", "its",
    "of", "on", "or", "that", "the", "this", "through", "to", "was", "with",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_matching_cognition_first() {
        let docs = vec![
            CognitionDoc {
                cognition_id: "cognition:generative-art/definition".to_string(),
                cognition_hash: "sha256:abc".to_string(),
                display_title: Some("Definition".to_string()),
                body: "Generative art is an artwork produced by a generative system.".to_string(),
                state: "active".to_string(),
            },
            CognitionDoc {
                cognition_id: "cognition:generative-art/authorship".to_string(),
                cognition_hash: "sha256:def".to_string(),
                display_title: Some("Authorship".to_string()),
                body: "The artist authors constraints.".to_string(),
                state: "active".to_string(),
            },
        ];
        let ranked = rank_cognitions("generative system possibility space", &docs, 5);
        assert_eq!(
            ranked[0].cognition_id,
            "cognition:generative-art/definition"
        );
        assert!((0.0..=1.0).contains(&ranked[0].score));
    }

    #[test]
    fn body_affects_matching() {
        let docs = vec![CognitionDoc {
            cognition_id: "cognition:coaching/one-cue".to_string(),
            cognition_hash: "sha256:abc".to_string(),
            display_title: Some("One Cue".to_string()),
            body: "During active on-court coaching with beginning tennis players, instruction should prioritize one actionable cue.".to_string(),
            state: "active".to_string(),
        }];
        let ranked = rank_cognitions("beginning tennis on court", &docs, 5);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].components.body > 0.0);
    }
}
