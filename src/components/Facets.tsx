import React, { useEffect, useState } from "react";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";
import { Link } from "react-router-dom";

const Facets: React.FC = () => {
  const [facets, setFacets] = useState<Facet[]>([]);

  useEffect(() => {
    const fetchedFacets = extractFacets();
    setFacets(fetchedFacets);
  }, []);

  const facetItems = facets.map((facet) => {
    const honedByFacets =
      facet?.honedBy?.map((honedFacetId) => {
        const honedFacet = facets.find(
          (facet) => facet.facetId === honedFacetId,
        );
        const articleId = honedFacet?.articleId;
        const title = honedFacet?.title;

        return {
          facetId: honedFacetId,
          title,
          articleId,
        };
      }) || [];

    return {
      facetId: facet.facetId,
      title: facet.title,
      articleId: facet.articleId,
      honedByFacets,
    };
  });

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
                        {honedByFacet.title}
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
