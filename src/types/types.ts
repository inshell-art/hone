import { SerializedEditorState } from "lexical";

// Connect editor to article
export type EditorProps = {
  articleId: string;
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

// Type for the article data stored in localStorage
export type ArticleData = {
  [articleId: string]: { content: SerializedEditorState; updatedAt: number };
};
