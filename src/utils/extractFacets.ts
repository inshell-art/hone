import { ArticleRecord, Facet } from "../types/types";

export const extractFacets = (): Facet[] => {
  const facets: Facet[] = [];

  const storedArticles = localStorage.getItem("HoneEditorArticles");
  if (storedArticles) {
    try {
      const parsedArticles: ArticleRecord = JSON.parse(storedArticles);

      Object.entries(parsedArticles).forEach(([articleId, articleContent]) => {
        const children = articleContent.root.children;
        let currentFacet: Facet | null = null;

        children.forEach((node, index) => {
          if ("tag" in node && node.tag === "h2") {
            const title = node.children[0].text as string;
            if (currentFacet) {
              facets.push(currentFacet);
            }

            // Start a new facet
            currentFacet = {
              facetId: `${articleId}-${index}`,
              title,
              articleId,
              content: [],
            };
          } else if (currentFacet) {
            const nodeIndex = children.indexOf(node);
            const titleIndex = children.indexOf(children[nodeIndex - 1]);
            if (nodeIndex !== titleIndex + 1) {
              currentFacet.content.push(node);
            }
          }
        });

        // Push the last facet if it exists
        if (currentFacet) {
          facets.push(currentFacet);
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
