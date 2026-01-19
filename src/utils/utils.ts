import {
  SerializedElementNode,
  SerializedLexicalNode,
  SerializedTextNode,
  $isRangeSelection,
  $getRoot,
  BaseSelection,
} from "lexical";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { Facet, HoneData, HoneExportV1 } from "../types/types";
import {
  FACET_LIBRARY_KEY,
  HONE_ARTICLE_EDITIONS_KEY,
  HONE_DATA_KEY,
} from "../constants/storage";
import { computeSimilarity, tokenize } from "./similarity";
import {
  createEmptyPublishState,
  loadArticleEditions,
} from "./articleEditions";
import { createEmptyLibrary, loadLibrary } from "./facetLibrary";

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
  return tokenize(text, { removeStopwords: false });
};

export const getJaccardSimilarity = (text1: string, text2: string) => {
  const words1 = new Set(splitAndNormalizeText(text1));
  const words2 = new Set(splitAndNormalizeText(text2));

  const intersection = new Set([...words1].filter((word) => words2.has(word))); // Common words
  const union = new Set([...words1, ...words2]); // All unique words across both texts

  return intersection.size / union.size; // Jaccard similarity = intersection / union
};

export function getFacetSimilarityScore(
  aTitle: string,
  aBodyText: string,
  bTitle: string,
  bBodyText: string,
  corpus?: string[],
): number {
  const docA = `${aTitle}\n${aBodyText}`;
  const docB = `${bTitle}\n${bBodyText}`;

  return computeSimilarity(docA, docB, corpus);
}

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
  const comparisonDoc = `${facetToCompare.title}\n${facetToCompare.content.join(" ")}`;
  const candidateDocs = facets.map(
    (facet) => `${facet.title}\n${facet.content.join(" ")}`,
  );
  const corpusDocs = [comparisonDoc, ...candidateDocs];

  return facets
    .map((facet, index) => ({
      ...facet,
      similarity: computeSimilarity(
        comparisonDoc,
        candidateDocs[index] ?? "",
        corpusDocs,
      ),
    }))
    .sort((a, b) => b.similarity - a.similarity);
};

// Utility function to find the nearest upper facet title node
export const findNearestFacetTitleNode = (selection: BaseSelection | null) => {
  if ($isRangeSelection(selection)) {
    const root = $getRoot();
    const children = root.getChildren();
    const anchorNode = selection.anchor.getNode();
    const anchorTopLevel = anchorNode.getTopLevelElementOrThrow();
    const anchorIndex = children.indexOf(anchorTopLevel);

    for (let i = anchorIndex; i >= 0; i--) {
      const childNode = children[i];
      if (childNode instanceof FacetTitleNode && childNode.isActive()) {
        return childNode as FacetTitleNode;
      }
    }
  }

  return null;
};

export const exportSavedArticles = () => {
  const savedArticlesJSON = localStorage.getItem(HONE_DATA_KEY);
  const honeData: HoneData = savedArticlesJSON
    ? JSON.parse(savedArticlesJSON)
    : {};
  const facetsLibrary = loadLibrary();
  const articleEditions = loadArticleEditions();

  const hasDrafts = Object.keys(honeData).length > 0;
  const hasFacets = Object.keys(facetsLibrary.facetsById).length > 0;
  const hasEditions = Object.keys(articleEditions.articles).length > 0;

  if (!hasDrafts && !hasFacets && !hasEditions) {
    console.log("No data to export.");
    alert("No data to export.");
    return;
  }

  const exportPayload: HoneExportV1 = {
    version: 1,
    exportedAt: Date.now(),
    honeData,
    facetsLibraryV2: facetsLibrary,
    articleEditionsV1: articleEditions,
  };

  const dataStr = JSON.stringify(exportPayload, null, 2);
  const blob = new Blob([dataStr], { type: "application/json" });
  const url = URL.createObjectURL(blob);

  const timestamp = formatTimestamp(Date.now());

  const link = document.createElement("a");
  link.href = url;
  link.download = `My Hone ${timestamp}.json`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
};

export const importSavedArticles = (
  fileLoadEvent: React.ChangeEvent<HTMLInputElement>,
) => {
  console.log("Importing articles...");
  if (fileLoadEvent.target.files === null) {
    return;
  }

  const file = fileLoadEvent.target.files[0];
  console.log("File selected for import:", file.size, file.type, file.name);

  if (!file) {
    console.log("No file selected for import.");
    return;
  }

  const userConfirmed = window.confirm(
    "Importing a file will overwrite your current data. Are you sure?",
  );

  if (userConfirmed) {
    console.log("user confirmed");
  }

  if (!userConfirmed) {
    return;
  }

  console.log("Importing file:", file);

  const reader = new FileReader();

  reader.onload = (fileReadEvent) => {
    try {
      if (
        !fileReadEvent.target ||
        typeof fileReadEvent.target.result !== "string"
      ) {
        throw new Error("Invalid file data");
      }

      const importedData = JSON.parse(fileReadEvent.target.result);
      console.log("Imported Data:", importedData);

      if (typeof importedData !== "object" || importedData === null) {
        throw new Error("Invalid data format");
      }

      const importedExport = importedData as Partial<HoneExportV1> & {
        facetsLibrary?: unknown;
        articleEditions?: unknown;
      };
      const hasExportShape =
        importedExport.version === 1 &&
        ("honeData" in importedExport ||
          "facetsLibraryV2" in importedExport ||
          "articleEditionsV1" in importedExport);

      if (hasExportShape) {
        const nextHoneData =
          (importedExport.honeData as HoneData | undefined) ?? {};
        const nextFacets =
          (importedExport.facetsLibraryV2 as
            | ReturnType<typeof createEmptyLibrary>
            | undefined) ?? createEmptyLibrary();
        const nextEditions =
          (importedExport.articleEditionsV1 as
            | ReturnType<typeof createEmptyPublishState>
            | undefined) ?? createEmptyPublishState();

        localStorage.setItem(HONE_DATA_KEY, JSON.stringify(nextHoneData));
        localStorage.setItem(FACET_LIBRARY_KEY, JSON.stringify(nextFacets));
        localStorage.setItem(
          HONE_ARTICLE_EDITIONS_KEY,
          JSON.stringify(nextEditions),
        );
        window.location.reload();
        return;
      }

      const nextHoneData = importedData as HoneData;
      localStorage.setItem(HONE_DATA_KEY, JSON.stringify(nextHoneData));

      if (!localStorage.getItem(FACET_LIBRARY_KEY)) {
        localStorage.setItem(
          FACET_LIBRARY_KEY,
          JSON.stringify(createEmptyLibrary()),
        );
      }
      if (!localStorage.getItem(HONE_ARTICLE_EDITIONS_KEY)) {
        localStorage.setItem(
          HONE_ARTICLE_EDITIONS_KEY,
          JSON.stringify(createEmptyPublishState()),
        );
      }
      window.location.reload();
    } catch (error) {
      alert("Failed to import savedArticles.");
      console.error("Failed to import savedArticles:", error);
    }
  };

  reader.readAsText(file);
};
