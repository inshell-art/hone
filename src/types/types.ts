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
};

// Type for the article data stored in localStorage
export type ArticleData = {
  [articleId: string]: { content: SerializedEditorState; updatedAt: number };
};
