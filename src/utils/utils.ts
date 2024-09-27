import {
  SerializedElementNode,
  SerializedLexicalNode,
  SerializedTextNode,
  $isRangeSelection,
  $getRoot,
  BaseSelection,
} from "lexical";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { Facet } from "../types/types";

export const INSERT_SYMBOL = ">>>>>>>";

export const collectTextFromDescendants = (
  node: SerializedElementNode | SerializedLexicalNode | SerializedTextNode,
  collectedTexts: string[],
): string[] => {
  if (node.type === "text") {
    collectedTexts.push((node as SerializedTextNode).text.trim());
  } else if ("children" in node && Array.isArray(node.children)) {
    node.children.forEach((child) => {
      collectTextFromDescendants(child, collectedTexts);
    });
  }

  return collectedTexts;
};

export const formatTimestamp = (timestamp: number) => {
  const date = new Date(timestamp);

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0"); // Months are zero-based
  const day = String(date.getDate()).padStart(2, "0");

  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");

  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
};

const splitAndNormalizeText = (text: string) => {
  return text.toLowerCase().match(/\w+/g) || [];
};

export const getJaccardSimilarity = (text1: string, text2: string) => {
  const words1 = new Set(splitAndNormalizeText(text1));
  const words2 = new Set(splitAndNormalizeText(text2));

  const intersection = new Set([...words1].filter((word) => words2.has(word))); // Common words
  const union = new Set([...words1, ...words2]); // All unique words across both texts

  return intersection.size / union.size; // Jaccard similarity = intersection / union
};

export const listFacetsWithSimilarity = (
  currentFacet: Facet | undefined,
  facets: Facet[],
) => {
  const emptyFacet: Facet = {
    articleId: "",
    content: [],
    facetId: "",
    title: "",
  };

  const facetToCompare = currentFacet || emptyFacet;

  return facets
    .map((facet) => {
      return {
        ...facet,
        similarity: getJaccardSimilarity(
          facetToCompare.title + "" + facetToCompare.content.join(" "),
          facet.title + "" + facet.content.join(" "),
        ),
      };
    })
    .sort((a, b) => b.similarity - a.similarity);
};

// Utility function to find the nearest upper facet title node
export const findNearestFacetTitleNode = (selection: BaseSelection | null) => {
  if ($isRangeSelection(selection)) {
    const root = $getRoot();
    const children = root.getChildren();

    const currentNode = selection.anchor.getNode();
    const currentNodeIndex = children.indexOf(currentNode);

    for (let i = currentNodeIndex - 1; i >= 0; i--) {
      const childNode = children[i];
      if (childNode instanceof FacetTitleNode && childNode.isActive()) {
        return childNode as FacetTitleNode;
      }
    }
  }

  return null;
};
