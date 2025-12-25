const DEFAULT_STOPWORDS = new Set([
  "the",
  "a",
  "an",
  "and",
  "or",
  "to",
  "of",
  "in",
  "on",
  "for",
  "with",
  "is",
  "are",
  "was",
  "were",
  "be",
  "as",
  "at",
  "by",
  "it",
  "this",
  "that",
]);

const TITLE_WEIGHT = 3;
const MIN_TOKEN_COUNT = 3;
const TFIDF_WEIGHT = 0.9;
const JACCARD_WEIGHT = 0.1;

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

const splitDocText = (docText: string) => {
  const safeText = docText ?? "";
  const parts = safeText.split("\n");
  const title = parts.shift() ?? "";

  return { title, body: parts.join("\n") };
};

export function tokenize(
  text: string,
  options?: { removeStopwords?: boolean },
): string[] {
  const normalized = (text ?? "").toLowerCase();
  let matches: string[] | null = null;

  try {
    matches = normalized.match(/\p{L}[\p{L}\p{N}_-]*|\p{N}+/gu);
  } catch (error) {
    matches = null;
  }

  if (!matches || matches.length === 0) {
    matches = normalized.match(/[a-z0-9]+/g) ?? [];
  }

  const removeStopwords = options?.removeStopwords !== false;
  if (!removeStopwords) {
    return matches;
  }

  return matches.filter((token) => !DEFAULT_STOPWORDS.has(token));
}

const tokenizeDoc = (docText: string, removeStopwords: boolean) => {
  const { title, body } = splitDocText(docText);

  return [
    ...tokenize(title, { removeStopwords }),
    ...tokenize(body, { removeStopwords }),
  ];
};

const buildTfMap = (docText: string, removeStopwords: boolean) => {
  const { title, body } = splitDocText(docText);
  const titleTokens = tokenize(title, { removeStopwords });
  const bodyTokens = tokenize(body, { removeStopwords });
  const counts = new Map<string, number>();

  for (const token of bodyTokens) {
    counts.set(token, (counts.get(token) ?? 0) + 1);
  }

  for (const token of titleTokens) {
    counts.set(token, (counts.get(token) ?? 0) + TITLE_WEIGHT);
  }

  return counts;
};

const buildIdfMap = (docs: string[], removeStopwords: boolean) => {
  const dfCounts = new Map<string, number>();

  for (const doc of docs) {
    const tokens = new Set(tokenizeDoc(doc, removeStopwords));
    for (const token of tokens) {
      dfCounts.set(token, (dfCounts.get(token) ?? 0) + 1);
    }
  }

  const docCount = Math.max(docs.length, 1);
  const idfMap = new Map<string, number>();

  for (const [token, df] of dfCounts.entries()) {
    idfMap.set(token, Math.log((docCount + 1) / (df + 1)) + 1);
  }

  return { idfMap, docCount };
};

const buildTfidfVector = (
  docText: string,
  idfMap: Map<string, number>,
  docCount: number,
  removeStopwords: boolean,
) => {
  const tfMap = buildTfMap(docText, removeStopwords);
  const vector = new Map<string, number>();
  let norm = 0;
  const defaultIdf = Math.log(docCount + 1) + 1;

  for (const [token, tf] of tfMap.entries()) {
    const idf = idfMap.get(token) ?? defaultIdf;
    const value = tf * idf;
    vector.set(token, value);
    norm += value * value;
  }

  return { vector, norm: Math.sqrt(norm) };
};

const computeJaccard = (
  docA: string,
  docB: string,
  removeStopwords: boolean,
) => {
  const tokensA = new Set(tokenizeDoc(docA, removeStopwords));
  const tokensB = new Set(tokenizeDoc(docB, removeStopwords));
  if (tokensA.size === 0 && tokensB.size === 0) {
    return 0;
  }

  const intersectionSize = [...tokensA].filter((token) =>
    tokensB.has(token),
  ).length;
  const unionSize = new Set([...tokensA, ...tokensB]).size;

  return unionSize === 0 ? 0 : intersectionSize / unionSize;
};

export function computeTfidfCosine(
  docA: string,
  docB: string,
  corpusDocs?: string[],
): number {
  const docs = corpusDocs && corpusDocs.length > 0 ? corpusDocs : [docA, docB];
  const removeStopwords = true;
  const { idfMap, docCount } = buildIdfMap(docs, removeStopwords);
  const vectorA = buildTfidfVector(docA, idfMap, docCount, removeStopwords);
  const vectorB = buildTfidfVector(docB, idfMap, docCount, removeStopwords);

  if (vectorA.norm === 0 || vectorB.norm === 0) {
    return 0;
  }

  const [small, large] =
    vectorA.vector.size <= vectorB.vector.size
      ? [vectorA.vector, vectorB.vector]
      : [vectorB.vector, vectorA.vector];
  let dot = 0;

  for (const [token, value] of small.entries()) {
    const other = large.get(token);
    if (other !== undefined) {
      dot += value * other;
    }
  }

  const cosine = dot / (vectorA.norm * vectorB.norm);

  return clamp01(cosine);
}

export function computeSimilarity(
  docA: string,
  docB: string,
  corpusDocs?: string[],
): number {
  const tokensA = tokenizeDoc(docA, true);
  const tokensB = tokenizeDoc(docB, true);
  const jaccard = computeJaccard(docA, docB, false);

  if (tokensA.length < MIN_TOKEN_COUNT || tokensB.length < MIN_TOKEN_COUNT) {
    return clamp01(jaccard);
  }

  const tfidf = computeTfidfCosine(docA, docB, corpusDocs);
  if (!Number.isFinite(tfidf) || tfidf <= 0) {
    return clamp01(jaccard);
  }

  const blended = tfidf * TFIDF_WEIGHT + jaccard * JACCARD_WEIGHT;

  return clamp01(blended);
}
