import { describe, expect, it } from "vitest";
import { getJaccardSimilarity, listFacetsWithSimilarity } from "./utils";
import { computeSimilarity, tokenize } from "./similarity";

describe("tokenize", () => {
  it("handles punctuation, numbers, and mixed-case", () => {
    expect(tokenize("Hello, WORLD! 123 times.")).toEqual([
      "hello",
      "world",
      "123",
      "times",
    ]);
  });
});

describe("getJaccardSimilarity", () => {
  it("matches expected overlap ratio", () => {
    const score = getJaccardSimilarity("alpha beta", "alpha gamma");
    expect(score).toBeCloseTo(1 / 3, 5);
  });
});

describe("computeSimilarity", () => {
  it("returns near 1 for identical docs", () => {
    const doc = "Solar Storage\nBattery system for renewable energy.";
    const score = computeSimilarity(doc, doc, [doc]);
    expect(score).toBeGreaterThan(0.99);
  });

  it("returns near 0 for unrelated docs", () => {
    const docA = "Cats\nFelines and kittens.";
    const docB = "Quantum\nEntanglement in physics.";
    const score = computeSimilarity(docA, docB, [docA, docB]);
    expect(score).toBeLessThan(0.1);
  });

  it("rewards shared rare terms more than raw Jaccard", () => {
    const docA =
      "Cochlear implant mapping\nFrequency tuning for pediatric cochlear implants and speech perception.";
    const docB =
      "Pediatric cochlear implant tuning\nSpeech perception mapping with frequency adjustments.";
    const tfidfScore = computeSimilarity(docA, docB, [docA, docB]);
    const jaccardScore = getJaccardSimilarity(docA, docB);
    expect(tfidfScore).toBeGreaterThan(jaccardScore);
  });
});

describe("listFacetsWithSimilarity", () => {
  it("ranks the most similar facet first", () => {
    const currentFacet = {
      facetId: "facet-current",
      title: "Solar storage",
      articleId: "article-1",
      content: ["battery systems for renewable energy"],
    };
    const candidates = [
      {
        facetId: "facet-similar",
        title: "Solar battery storage",
        articleId: "article-2",
        content: ["renewable energy grid battery"],
      },
      {
        facetId: "facet-unrelated",
        title: "Cooking recipe",
        articleId: "article-3",
        content: ["pasta and sauce"],
      },
      {
        facetId: "facet-medium",
        title: "Wind power storage",
        articleId: "article-4",
        content: ["battery systems"],
      },
    ];

    const ranked = listFacetsWithSimilarity(currentFacet, candidates);
    expect(ranked[0].facetId).toBe("facet-similar");
  });
});
