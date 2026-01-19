import {
  ArticlePublishState,
  FacetsLibraryState,
  HoneData,
  PublishedArticleSummary,
} from "../types/types";
import {
  buildPublishedArticlesIndex,
  createEmptyPublishState,
} from "./articleEditions";
import { createEmptyLibrary } from "./facetLibrary";

const isHoneData = (value: unknown): value is HoneData => {
  if (!value || typeof value !== "object") {
    return false;
  }
  const firstEntry = Object.values(value as Record<string, unknown>)[0] as
    | { content?: unknown; updatedAt?: unknown }
    | undefined;

  return (
    !!firstEntry &&
    typeof firstEntry === "object" &&
    "content" in firstEntry &&
    "updatedAt" in firstEntry
  );
};

export type NormalizedFacetsPayload = {
  honeData: HoneData;
  facetsLibrary: FacetsLibraryState;
  articleEditions: ArticlePublishState;
  publishedArticlesIndex: PublishedArticleSummary[];
};

export const normalizeFacetsPayload = (
  payload: unknown,
): NormalizedFacetsPayload => {
  const emptyLibrary = createEmptyLibrary();
  const emptyEditions = createEmptyPublishState();

  if (!payload || typeof payload !== "object") {
    return {
      honeData: {},
      facetsLibrary: emptyLibrary,
      articleEditions: emptyEditions,
      publishedArticlesIndex: [],
    };
  }

  const data = payload as Record<string, unknown>;

  const honeData = isHoneData(data.honeData)
    ? (data.honeData as HoneData)
    : isHoneData(data)
      ? (data as HoneData)
      : {};

  const facetsLibrary =
    (data.facetsLibraryV2 as FacetsLibraryState | undefined) ??
    (data.facetsLibrary as FacetsLibraryState | undefined) ??
    emptyLibrary;

  const articleEditions =
    (data.articleEditionsV1 as ArticlePublishState | undefined) ??
    (data.articleEditions as ArticlePublishState | undefined) ??
    emptyEditions;

  const publishedArticlesIndex = Array.isArray(data.publishedArticlesIndex)
    ? (data.publishedArticlesIndex as PublishedArticleSummary[])
    : buildPublishedArticlesIndex(articleEditions);

  return {
    honeData,
    facetsLibrary,
    articleEditions,
    publishedArticlesIndex,
  };
};
