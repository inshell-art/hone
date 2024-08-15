import React, { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";

const Facets: React.FC = () => {
  const [facets, setFacets] = useState<Facet[]>([]);

  useEffect(() => {
    const fetchedFacets = extractFacets();
    setFacets(fetchedFacets);
  }, []);

  return (
    <div className="facets-container">
      <ul className="facets-list">
        {facets.length > 0 ? (
          facets.map((facet) => (
            <li key={facet.facetId} className="facet-item">
              <Link to={`/editor/${facet.articleId}`} className="facet-link">
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
