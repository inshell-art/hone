import {
  FacetLibraryItem,
  FacetsLibraryState,
  FacetId,
  HoneEdge,
} from "../types/types";
import { FACET_LIBRARY_KEY } from "../constants/storage";

const LIBRARY_VERSION = 2;

export const createEmptyLibrary = (): FacetsLibraryState => ({
  version: LIBRARY_VERSION,
  updatedAt: Date.now(),
  facetsById: {},
});

const persistLibrary = (state: FacetsLibraryState): FacetsLibraryState => {
  localStorage.setItem(FACET_LIBRARY_KEY, JSON.stringify(state));
  return state;
};

export const loadLibrary = (): FacetsLibraryState => {
  const raw = localStorage.getItem(FACET_LIBRARY_KEY);

  if (!raw) {
    return createEmptyLibrary();
  }

  try {
    const parsed = JSON.parse(raw) as FacetsLibraryState;

    if (
      parsed &&
      parsed.version === LIBRARY_VERSION &&
      parsed.facetsById &&
      typeof parsed.updatedAt === "number"
    ) {
      return parsed;
    }
  } catch (error) {
    console.error("Failed to parse facet library", error);
  }

  return createEmptyLibrary();
};

export const saveLibrary = (state: FacetsLibraryState): FacetsLibraryState => {
  const updatedState: FacetsLibraryState = {
    ...state,
    updatedAt: Date.now(),
  };

  return persistLibrary(updatedState);
};

type FacetUpsertInput = {
  facetId: FacetId;
  title: string;
  bodyText: string;
  updatedAt?: number;
};

export const upsertFacet = (
  state: FacetsLibraryState,
  input: FacetUpsertInput,
): FacetsLibraryState => {
  const now = input.updatedAt ?? Date.now();
  const existing = state.facetsById[input.facetId];

  const nextFacet: FacetLibraryItem = {
    facetId: input.facetId,
    title: input.title,
    bodyText: input.bodyText,
    updatedAt: now,
    honedFrom: existing?.honedFrom ?? [],
  };

  const nextState: FacetsLibraryState = {
    version: LIBRARY_VERSION,
    updatedAt: now,
    facetsById: {
      ...state.facetsById,
      [input.facetId]: nextFacet,
    },
  };

  return persistLibrary(nextState);
};

export const addHoneEdge = (
  state: FacetsLibraryState,
  targetFacetId: FacetId,
  sourceFacetId: FacetId,
  honedAt: number = Date.now(),
): FacetsLibraryState => {
  const target = state.facetsById[targetFacetId];

  if (!target) {
    return state;
  }

  const nextEdges: HoneEdge[] = [
    { fromFacetId: sourceFacetId, honedAt },
    ...target.honedFrom.filter((edge) => edge.fromFacetId !== sourceFacetId),
  ];

  const nextState: FacetsLibraryState = {
    version: LIBRARY_VERSION,
    updatedAt: honedAt,
    facetsById: {
      ...state.facetsById,
      [targetFacetId]: {
        ...target,
        honedFrom: nextEdges,
        updatedAt: target.updatedAt,
      },
    },
  };

  return persistLibrary(nextState);
};
