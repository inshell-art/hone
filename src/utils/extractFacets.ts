import { ParagraphNode, TextNode } from "lexical";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { ArticleRecord, Facet } from "../types/types";

export const extractFacets = (): Facet[] => {
  const facets: Facet[] = [];

  const storedArticles = localStorage.getItem("HoneEditorArticles");
  if (storedArticles) {
    try {
      const parsedArticles: ArticleRecord = JSON.parse(storedArticles);

      Object.entries(parsedArticles).forEach(([articleId, articleContent]) => {
        const childrenOfArticle = articleContent.root.children;
        let currentFacet: Facet | null = null;

        childrenOfArticle.forEach((node) => {
          if (node instanceof FacetTitleNode) {
            const title = node
              .getChildren()
              .filter((child) => child instanceof TextNode)
              .map((child) => child.getTextContent())
              .join(" ");

            if (currentFacet) {
              facets.push(currentFacet);
              currentFacet = null;
            }

            // Start a new facet
            currentFacet = {
              facetId: node.getUniqueId(),
              title,
              articleId,
              content: [],
            };
          } else if (node instanceof ParagraphNode && currentFacet) {
            currentFacet.content.push(node);
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
        error
      );
    }
  }

  return facets;
};
