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
          // Check if the node is a heading with tag "h2"
          if ("tag" in node && node.tag === "h2") {
            // If there's an ongoing facet, finalize and push it to the facets array
            if (currentFacet) {
              facets.push(currentFacet);
            }

            // Start a new facet
            currentFacet = {
              facetId: `${articleId}-${index}`,
              title: node.children[0]?.text || "Untitled Facet",
              articleId,
              content: [],
            };
          }

          // If there's an ongoing facet, add the node to its content
          if (currentFacet) {
            currentFacet.content.push(node);
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
