import React, { useEffect, useState } from "react";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";
import { useNavigate } from "react-router-dom";
import { listFacetsWithSimilarity } from "../utils/utils";

const Facets: React.FC = () => {
  const [facets, setFacets] = useState<Facet[]>([]);
  const navigate = useNavigate();

  useEffect(() => {
    const fetchedFacets = extractFacets();
    setFacets(fetchedFacets);
  }, []);

  const facetItems = facets
    .map((facet) => {
      const uniqueFacets = new Set<Facet>();
      facet?.honedBy?.forEach((honedFacetId) => {
        const matchedFacet = facets.find(
          (facet) => facet.facetId === honedFacetId,
        );
        if (matchedFacet) {
          uniqueFacets.add(matchedFacet);
        }
      });
      const uniqueHonedByFacets = Array.from(uniqueFacets);

      const uniqueHonedByFacetsWithSimilarity = listFacetsWithSimilarity(
        facet,
        uniqueHonedByFacets,
      );

      return {
        facetId: facet.facetId,
        title: facet.title,
        articleId: facet.articleId,
        honedByAmount: facet.honedAmount || 0,
        honedByFacets: uniqueHonedByFacetsWithSimilarity,
      };
    })
    .sort((a, b) => b.honedByAmount - a.honedByAmount);

  return (
    <div className="facets-container">
      <ul className="facets-list">
        {facetItems?.length > 0 ? (
          facetItems.map((facet) => (
            <li key={facet.facetId} className="facet-item">
              <a
                className="facet-link"
                href="/#"
                onClick={(e) => {
                  e.preventDefault();
                  navigate(
                    `/editor/${facet.articleId}?facetId=${facet.facetId}`,
                  );
                  console.log("Navigating to", `/editor/${facet.articleId}`);
                }}
              >
                {facet.title}
              </a>

              {facet.honedByFacets.length > 0 && (
                <ul className="honed-by-list">
                  {facet?.honedByFacets?.map((honedByFacet, index) => (
                    <li key={index} className="honed-by-item">
                      <a
                        className="honed-by-link"
                        href="/#"
                        onClick={(e) => {
                          e.preventDefault();
                          navigate(
                            `/editor/${honedByFacet.articleId}?facetId=${honedByFacet.facetId}`,
                          );
                        }}
                      >
                        {honedByFacet.title} (
                        {Math.round(honedByFacet.similarity * 100)}%)
                      </a>
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
