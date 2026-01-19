import { SerializedEditorState } from "lexical";

// Connect editor to article
export type EditorProps = {
  articleId: string;
  isEditable?: boolean;
};

export type AutoSavePluginProps = EditorProps & {
  onMessageChange: (message: string | null, isTemporary?: boolean) => void;
};

export type LoadArticlePluginProps = EditorProps & {
  onMessageChange: (message: string | null, isTemporary?: boolean) => void;
};

// Facet data shape, in hone panel and in facet list Facets
export type Facet = {
  facetId: string;
  title: string;
  articleId: string;
  content: string[];
  honedAmount?: number;
  honedBy?: string[];
};

export type FacetWithSimilarity = Facet & {
  similarity: number;
};

// Type for the Hone data stored in localStorage
export type HoneData = {
  [articleId: string]: { content: SerializedEditorState; updatedAt: number };
};

export type FacetId = string;

export type ArticleEdition = {
  editionId: string;
  articleId: string;
  version: number;
  createdAt: number;
  title: string;
  content: SerializedEditorState;
  contentHash?: string;
};

export type ArticlePublishRecord = {
  headEditionId: string;
  latestVersion: number;
  editionsById: Record<string, ArticleEdition>;
  editionsOrder: string[];
};

export type ArticlePublishState = {
  version: 1;
  updatedAt: number;
  articles: Record<string, ArticlePublishRecord>;
};

export type HoneExportV1 = {
  version: 1;
  exportedAt: number;
  honeData: HoneData;
  facetsLibraryV2: FacetsLibraryState;
  articleEditionsV1: ArticlePublishState;
};

export type PublishedArticleSummary = {
  articleId: string;
  title: string;
  latestVersion: number;
  updatedAt: number;
};

export type HoneEdge = {
  fromFacetId: FacetId;
  honedAt: number;
};

export type FacetLibraryItem = {
  facetId: FacetId;
  title: string;
  bodyText: string;
  updatedAt: number;
  honedFrom: HoneEdge[];
};

export type FacetsLibraryState = {
  version: 2;
  updatedAt: number;
  facetsById: Record<FacetId, FacetLibraryItem>;
};
