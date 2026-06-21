use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct FacetDoc {
    pub facet_id: String,
    pub revision_hash: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchComponents {
    pub tfidf: f64,
    pub jaccard: f64,
    pub exact_title_bonus: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    pub facet_id: String,
    pub facet_revision: String,
    pub title: String,
    pub score: f64,
    pub components: MatchComponents,
    pub matched_terms: Vec<String>,
}

pub fn rank_facets(query: &str, facets: &[FacetDoc], limit: usize) -> Vec<MatchResult> {
    let query_tokens = tokenize(query);
    let query_set: BTreeSet<_> = query_tokens.iter().cloned().collect();
    if query_set.is_empty() {
        return Vec::new();
    }

    let mut doc_freq: BTreeMap<String, usize> = BTreeMap::new();
    let doc_tokens: Vec<Vec<String>> = facets
        .iter()
        .map(|facet| {
            let tokens = tokenize(&format!("{} {}", facet.title, facet.body));
            let unique: BTreeSet<_> = tokens.iter().cloned().collect();
            for token in unique {
                *doc_freq.entry(token).or_default() += 1;
            }
            tokens
        })
        .collect();

    let query_tf = term_counts(&query_tokens);
    let doc_count = facets.len().max(1) as f64;

    let mut results = Vec::new();
    for (idx, facet) in facets.iter().enumerate() {
        let tokens = &doc_tokens[idx];
        let doc_tf = term_counts(tokens);
        let doc_set: BTreeSet<_> = tokens.iter().cloned().collect();
        let matched: Vec<String> = query_set.intersection(&doc_set).cloned().collect();
        if matched.is_empty() {
            continue;
        }

        let tfidf = cosine(&query_tf, &doc_tf, &doc_freq, doc_count);
        let union = query_set.union(&doc_set).count().max(1) as f64;
        let jaccard = matched.len() as f64 / union;
        let title_tokens: BTreeSet<_> = tokenize(&facet.title).into_iter().collect();
        let exact_title_bonus = if !query_set.is_disjoint(&title_tokens)
            || facet
                .title
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
        {
            1.0
        } else {
            0.0
        };
        let score = (0.80 * tfidf + 0.15 * jaccard + 0.05 * exact_title_bonus).clamp(0.0, 1.0);
        results.push(MatchResult {
            facet_id: facet.facet_id.clone(),
            facet_revision: facet.revision_hash.clone(),
            title: facet.title.clone(),
            score,
            components: MatchComponents {
                tfidf,
                jaccard,
                exact_title_bonus,
            },
            matched_terms: matched,
        });
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.facet_id.cmp(&b.facet_id))
    });
    results.truncate(limit);
    results
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
    fn ranks_matching_facet_first() {
        let docs = vec![
            FacetDoc {
                facet_id: "facet:generative-art/definition".to_string(),
                revision_hash: "sha256:abc".to_string(),
                title: "Definition".to_string(),
                body: "Generative art is an artwork produced by a generative system.".to_string(),
            },
            FacetDoc {
                facet_id: "facet:generative-art/authorship".to_string(),
                revision_hash: "sha256:def".to_string(),
                title: "Authorship".to_string(),
                body: "The artist authors constraints.".to_string(),
            },
        ];
        let ranked = rank_facets("generative system possibility space", &docs, 5);
        assert_eq!(ranked[0].facet_id, "facet:generative-art/definition");
        assert!((0.0..=1.0).contains(&ranked[0].score));
    }
}
