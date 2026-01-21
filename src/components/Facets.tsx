import React, { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { getJaccardSimilarity } from "../utils/utils";
import { FacetLibraryItem, FacetsLibraryState, HoneEdge } from "../types/types";
import { extractFacets } from "../utils/extractFacets";
import { loadLibrary, saveLibrary } from "../utils/facetLibrary";
import { FACET_LIBRARY_KEY, HONE_DATA_KEY } from "../constants/storage";

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

const getDisplayFacetTitle = (facet: FacetLibraryItem) => {
  const trimmedTitle = facet.title.trim();
  if (!trimmedTitle || trimmedTitle === facet.facetId) {
    return "Untitled facet";
  }
  return trimmedTitle;
};

const Facets: React.FC = () => {
  const [library, setLibrary] = useState<FacetsLibraryState>(loadLibrary());
  const [articlesRevision, setArticlesRevision] = useState(0);
  const navigate = useNavigate();

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === FACET_LIBRARY_KEY) {
        setLibrary(loadLibrary());
      }
      if (event.key === HONE_DATA_KEY) {
        setArticlesRevision(Date.now());
      }
    };

    window.addEventListener("storage", handleStorage);

    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  useEffect(() => {
    setLibrary(loadLibrary());
  }, []);

  const { facetIdToArticleId, articleIds, liveFacetIds } = useMemo(() => {
    void articlesRevision;
    try {
      const raw = localStorage.getItem(HONE_DATA_KEY);
      const parsed = raw ? JSON.parse(raw) : {};
      const liveFacets = extractFacets(parsed);
      const map = new Map<string, string>();
      const liveIds = new Set<string>();
      liveFacets.forEach((facet) => {
        map.set(facet.facetId, facet.articleId);
        liveIds.add(facet.facetId);
      });
      return {
        facetIdToArticleId: map,
        articleIds: new Set(Object.keys(parsed)),
        liveFacetIds: liveIds,
      };
    } catch (error) {
      return {
        facetIdToArticleId: new Map(),
        articleIds: new Set(),
        liveFacetIds: new Set(),
      };
    }
  }, [articlesRevision]);

  useEffect(() => {
    if (liveFacetIds.size === 0) {
      return;
    }

    const nextFacets: Record<string, FacetLibraryItem> = {};
    let removed = false;

    Object.values(library.facetsById).forEach((facet) => {
      if (!liveFacetIds.has(facet.facetId)) {
        removed = true;
        return;
      }

      nextFacets[facet.facetId] = facet;
    });

    if (!removed) {
      return;
    }

    const nextState = saveLibrary({ ...library, facetsById: nextFacets });
    setLibrary(nextState);
  }, [library, liveFacetIds]);

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
            const resolvedArticleId =
              facetIdToArticleId.get(facet.facetId) ??
              getArticleIdFromFacetId(facet.facetId);
            const facetLink =
              resolvedArticleId && articleIds.has(resolvedArticleId)
                ? `/a/${resolvedArticleId}?facetId=${facet.facetId}`
                : null;
            const displayTitle = getDisplayFacetTitle(facet);

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
                      {displayTitle}
                    </a>
                  ) : (
                    <span className="facet-link">{displayTitle}</span>
                  )}
                  {honedFrom.length > 0 && (
                    <div className="facet-meta">
                      <span className="facet-stats">Honed from:</span>
                    </div>
                  )}
                </div>

                {honedFrom.length > 0 && (
                  <div className="honed-from-section">
                    <ul className="honed-from-list">
                      {honedFrom.map(({ source, edge, similarity }) => {
                        const sourceArticleId =
                          facetIdToArticleId.get(source.facetId) ??
                          getArticleIdFromFacetId(source.facetId);
                        const sourceLink =
                          sourceArticleId && articleIds.has(sourceArticleId)
                            ? `/a/${sourceArticleId}?facetId=${source.facetId}`
                            : null;
                        const sourceTitle = getDisplayFacetTitle(source);

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
                                {sourceTitle}
                              </a>
                            ) : (
                              <span className="honed-by-link">
                                {sourceTitle}
                              </span>
                            )}
                            <span className="honed-from-meta">
                              {Math.round(similarity * 100)}% similarity
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
