import { SerializedEditorState } from "lexical";
import { ArticleData, Facet } from "../types/types";
import { collectTextFromDescendants } from "./utils";

export const extractFacets = (): Facet[] => {
  const facets: Facet[] = [];

  const storedArticles = localStorage.getItem("HoneEditorArticles");
  if (storedArticles) {
    try {
      const parsedArticles: ArticleData = JSON.parse(storedArticles);

      Object.entries(parsedArticles).forEach(([id, { content }]) => {
        const childrenOfArticle = (content as SerializedEditorState).root
          .children;
        let currentFacet: Facet | null = null;

        childrenOfArticle.forEach((node) => {
          if (
            node.type === "facet-title" &&
            "active" in node &&
            node.active === true &&
            "uniqueId" in node &&
            "honedBy" in node &&
            "honedAmount" in node
          ) {
            const collectedTitle: string[] = [];
            collectTextFromDescendants(node, collectedTitle);

            const facetId = node.uniqueId as string;
            const honedBy = node.honedBy as string[];
            const honedAmount = node.honedAmount as number;

            if (currentFacet) {
              facets.push(currentFacet);
              currentFacet = null;
            }

            // Start a new facet
            currentFacet = {
              facetId,
              title: collectedTitle.join(" "),
              articleId: id,
              content: [],
              honedAmount,
              honedBy,
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

  return facets;
};
