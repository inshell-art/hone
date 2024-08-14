import React, { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ArticleContent } from "../types/types";

// Assuming this function is in the same file or imported from another module
const extractFacetTitlesWithIds = (): { id: string; title: string }[] => {
  const facetTitlesWithIds: { id: string; title: string }[] = [];

  // Fetch the stored articles from localStorage
  const storedArticles = localStorage.getItem("HoneEditorArticles");
  if (storedArticles) {
    try {
      const parsedArticles: Record<string, ArticleContent> =
        JSON.parse(storedArticles);

      // Iterate through each article
      Object.entries(parsedArticles).forEach(([id, article]) => {
        // Iterate through the children of the root node
        article.root.children.forEach((node) => {
          // Check if the node is a heading and has the tag "h2"
          if (node.type === "heading" && node.tag === "h2") {
            // Extract the text from the first child of the heading node (TextNode)
            const textNode = node.children[0];
            if (textNode && textNode.type === "text") {
              facetTitlesWithIds.push({ id, title: textNode.text });
            }
          }
        });
      });
    } catch (error) {
      console.error("Failed to parse the stored articles", error);
    }
  }

  return facetTitlesWithIds;
};

const Facets: React.FC = () => {
  const [facets, setFacets] = useState<{ id: string; title: string }[]>([]);

  useEffect(() => {
    const fetchedFacets = extractFacetTitlesWithIds();
    setFacets(fetchedFacets);
  }, []);

  return (
    <div className="facets-container">
      <ul className="facets-list">
        {facets.length > 0 ? (
          facets.map((facet) => (
            <li key={facet.id} className="facet-item">
              <Link to={`/editor/${facet.id}`} className="facet-link">
                {facet.title}
              </Link>
            </li>
          ))
        ) : (
          <li>No facets found</li>
        )}
      </ul>
    </div>
  );
};

export default Facets;
