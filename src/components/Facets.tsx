import React, { useEffect, useState } from "react";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";
import { Link } from "react-router-dom";
import { getJaccardSimilarity } from "../utils/utils";

const Facets: React.FC = () => {
  const [facets, setFacets] = useState<Facet[]>([]);

  useEffect(() => {
    const fetchedFacets = extractFacets();
    setFacets(fetchedFacets);
  }, []);

  const facetItems = facets
    .map((facet) => {
      const honedByMap = new Map();

      facet?.honedBy?.map((honedFacetId) => {
        const honedFacet = facets.find(
          (facet) => facet.facetId === honedFacetId,
        );
        const articleId = honedFacet?.articleId;
        const title = honedFacet?.title;
        const facetText = facet.title + " " + facet.content.join(" ");
        const honedFacetText =
          honedFacet?.title + " " + honedFacet?.content.join(" ") || "";

        const similarity = getJaccardSimilarity(facetText, honedFacetText);
        console.log(
          `facetText: ${facetText}`,
          `honedFacetText: ${honedFacetText}`,
          `similarity: ${similarity}`,
        );

        if (!honedByMap.has(honedFacetId)) {
          honedByMap.set(honedFacetId, {
            facetId: honedFacetId,
            title,
            articleId,
            similarity,
          });
        }
      }) || [];

      const uniqueHonedByFacets = Array.from(honedByMap.values()).sort(
        (a, b) => b.similarity - a.similarity,
      );

      return {
        facetId: facet.facetId,
        title: facet.title,
        articleId: facet.articleId,
        honedByAmount: facet.honedAmount || 0,
        honedByFacets: uniqueHonedByFacets,
      };
    })
    .sort((a, b) => b.honedByAmount - a.honedByAmount);

  return (
    <div className="facets-container">
      <ul className="facets-list">
        {facetItems?.length > 0 ? (
          facetItems.map((facet) => (
            <li key={facet.facetId} className="facet-item">
              <Link to={`/editor/${facet.articleId}`} className="facet-link">
                {facet.title}
              </Link>

              {facet.honedByFacets.length > 0 && (
                <ul className="honed-by-list">
                  {facet?.honedByFacets?.map((honedByFacet, index) => (
                    <li key={index} className="honed-by-item">
                      <Link
                        to={`/editor/${honedByFacet.articleId}`}
                        className="honed-by-link"
                      >
                        {honedByFacet.title} (
                        {Math.round(honedByFacet.similarity * 100)}%)
                      </Link>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))
        ) : (
          <li className="no-facets">No facets found</li>
        )}
      </ul>
    </div>
  );
};

export default Facets;
