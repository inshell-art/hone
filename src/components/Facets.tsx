import React, { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { formatTimestamp, getJaccardSimilarity } from "../utils/utils";
import { FacetLibraryItem, FacetsLibraryState, HoneEdge } from "../types/types";
import { loadLibrary } from "../utils/facetLibrary";
import { FACET_LIBRARY_KEY } from "../constants/storage";

type HonedFromListItem = {
  edge: HoneEdge;
  source: FacetLibraryItem;
  similarity: number;
};

type FacetListItem = {
  facet: FacetLibraryItem;
  honedFrom: HonedFromListItem[];
};

const combineText = (facet: FacetLibraryItem) =>
  `${facet.title} ${facet.bodyText}`.trim();

const getArticleIdFromFacetId = (facetId: string) => {
  const marker = "-facet-";
  if (facetId.includes(marker)) {
    return facetId.split(marker)[0];
  }
  return null;
};

const Facets: React.FC = () => {
  const [library, setLibrary] = useState<FacetsLibraryState>(loadLibrary());
  const navigate = useNavigate();

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === FACET_LIBRARY_KEY) {
        setLibrary(loadLibrary());
      }
    };

    window.addEventListener("storage", handleStorage);

    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  useEffect(() => {
    setLibrary(loadLibrary());
  }, []);

  const facetItems: FacetListItem[] = useMemo(() => {
    const facets = Object.values(library.facetsById).sort(
      (a, b) => b.updatedAt - a.updatedAt,
    );

    return facets.map((facet) => {
      const honedFrom = (facet.honedFrom || [])
        .slice()
        .sort((a, b) => b.honedAt - a.honedAt)
        .map((edge) => {
          const source = library.facetsById[edge.fromFacetId];
          if (!source) {
            return null;
          }
          const similarity = getJaccardSimilarity(
            combineText(facet),
            combineText(source),
          );
          return {
            edge,
            source,
            similarity,
          };
        })
        .filter(Boolean) as HonedFromListItem[];

      return { facet, honedFrom };
    });
  }, [library]);

  return (
    <div className="facets-container">
      <ul className="facets-list">
        {facetItems.length > 0 ? (
          facetItems.map(({ facet, honedFrom }) => {
            const articleId = getArticleIdFromFacetId(facet.facetId);
            const facetLink = articleId
              ? `/article/${articleId}?facetId=${facet.facetId}`
              : null;

            return (
              <li key={facet.facetId} className="facet-item">
                <div className="facet-header">
                  {facetLink ? (
                    <a
                      className="facet-link"
                      href="/#"
                      onClick={(e) => {
                        e.preventDefault();
                        navigate(facetLink);
                      }}
                    >
                      {facet.title}
                    </a>
                  ) : (
                    <span className="facet-link">{facet.title}</span>
                  )}
                  <div className="facet-meta">
                    <span className="facet-updated">
                      Updated {formatTimestamp(facet.updatedAt)}
                    </span>
                    {honedFrom.length > 0 && (
                      <span className="facet-stats">
                        • Honed from {honedFrom.length}
                      </span>
                    )}
                  </div>
                </div>

                {honedFrom.length > 0 && (
                  <div className="honed-from-section">
                    <div className="honed-from-label">Honed from:</div>
                    <ul className="honed-from-list">
                      {honedFrom.map(({ source, edge, similarity }) => {
                        const sourceArticleId = getArticleIdFromFacetId(
                          source.facetId,
                        );
                        const sourceLink = sourceArticleId
                          ? `/article/${sourceArticleId}?facetId=${source.facetId}`
                          : null;

                        return (
                          <li
                            key={`${facet.facetId}-${source.facetId}-${edge.honedAt}`}
                            className="honed-from-item"
                          >
                            {sourceLink ? (
                              <a
                                className="honed-by-link"
                                href="/#"
                                onClick={(e) => {
                                  e.preventDefault();
                                  navigate(sourceLink);
                                }}
                              >
                                {source.title}
                              </a>
                            ) : (
                              <span className="honed-by-link">
                                {source.title}
                              </span>
                            )}
                            <span className="honed-from-meta">
                              {Math.round(similarity * 100)}% similarity
                              {edge.honedAt && (
                                <span className="honed-from-time">
                                  {" "}
                                  • {formatTimestamp(edge.honedAt)}
                                </span>
                              )}
                            </span>
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                )}
              </li>
            );
          })
        ) : (
          <li className="no-facets">No facets in the library yet.</li>
        )}
      </ul>
    </div>
  );
};

export default Facets;
