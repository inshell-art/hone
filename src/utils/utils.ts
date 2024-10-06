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

export const exportSavedArticles = () => {
  const savedArticlesJSON = localStorage.getItem("HoneEditorArticles");
  if (!savedArticlesJSON) {
    console.log("No articles to export.");
    alert("No articles to export.");
    return;
  }

  const savedArticles = JSON.parse(savedArticlesJSON);
  if (Object.keys(savedArticles).length === 0) {
    console.log("No articles to export.");
    alert("No articles to export.");
    return;
  }

  const dataStr = JSON.stringify(savedArticles, null, 2);
  const blob = new Blob([dataStr], { type: "application/json" });
  const url = URL.createObjectURL(blob);

  const link = document.createElement("a");
  link.href = url;
  link.download = "My Hone.json";
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
};

export const importSavedArticles = (
  fileLoadEvent: React.ChangeEvent<HTMLInputElement>,
) => {
  if (fileLoadEvent.target.files === null) {
    return;
  }

  const file = fileLoadEvent.target.files[0];

  if (!file) {
    console.log("No file selected for import.");
    return;
  }

  const userConfirmed = window.confirm(
    "Importing a file will overwrite your current data. Are you sure?",
  );

  if (!userConfirmed) {
    return;
  }

  const reader = new FileReader(); // Create a FileReader to read the file

  reader.onload = (fileReadEvent) => {
    try {
      if (
        !fileReadEvent.target ||
        typeof fileReadEvent.target.result !== "string"
      ) {
        throw new Error("Invalid file data");
      }

      const importedData = JSON.parse(fileReadEvent.target.result);

      if (typeof importedData !== "object" || importedData === null) {
        throw new Error("Invalid data format");
      }

      console.log("Imported Data:", importedData);

      localStorage.setItem("HoneEditorArticles", JSON.stringify(importedData));
      window.location.reload();
    } catch (error) {
      alert("Failed to import savedArticles.");
      console.error("Failed to import savedArticles:", error);
    }
  };

  reader.readAsText(file);
};
