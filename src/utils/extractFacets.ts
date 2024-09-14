import { SerializedEditorState } from "lexical";
import { Facet } from "../types/types";
import { collectTextFromDescendants } from "./utils";

export const extractFacets = (): Facet[] => {
  const facets: Facet[] = [];

  const storedArticles = localStorage.getItem("HoneEditorArticles");
  if (storedArticles) {
    try {
      const parsedArticles = JSON.parse(storedArticles);

      Object.entries(parsedArticles).forEach(([articleId, articleContent]) => {
        const childrenOfArticle = (articleContent as SerializedEditorState).root
          .children;
        let currentFacet: Facet | null = null;

        childrenOfArticle.forEach((node) => {
          let facetId = "";

          if ("uniqueId" in node) {
            facetId = node.uniqueId as string;
          }

          if (node.type === "facet-title") {
            const collectedTitle: string[] = [];
            collectTextFromDescendants(node, collectedTitle);

            if (currentFacet) {
              facets.push(currentFacet);
              currentFacet = null;
            }

            // Start a new facet
            currentFacet = {
              facetId,
              title: collectedTitle.join(" "),
              articleId,
              content: [],
            };
          } else {
            if (currentFacet) {
              const collectedContent: string[] = [];
              collectTextFromDescendants(node, collectedContent);

              if (collectedContent.length > 0) {
                currentFacet.content.push(...(collectedContent as string[]));
              }
            }
          }
        });

        // Push the last facet if it exists
        if (currentFacet) {
          facets.push(currentFacet);
          currentFacet = null;
        }
      });
    } catch (error) {
      console.error(
        "Failed to parse the stored articles to extract facets",
        error,
      );
    }
  }
  console.log("facets", facets);
  return facets;
};
