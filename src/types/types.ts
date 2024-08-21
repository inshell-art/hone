type TextNode = {
  detail: number;
  format: number;
  mode: string;
  style: string;
  text: string;
  type: string;
  version: number;
};

export type HeadingNode = {
  children: TextNode[];
  direction: string;
  format: string;
  indent: number;
  type: string;
  version: number;
  tag: string;
};

export type ParagraphNode = {
  children: TextNode[];
  direction: string;
  format: string;
  indent: number;
  type: string;
  version: number;
};

type RootNode = {
  children: (HeadingNode | ParagraphNode)[];
  direction: string;
  format: string;
  indent: number;
  type: string;
  version: number;
};

export type ArticleContent = {
  root: RootNode;
};

export type EditorProps = {
  articleId: string;
};

export type Facet = {
  facetId: string;
  title: string;
  articleId: string;
  content: ParagraphNode[];
};

export type ArticleRecord = Record<string, ArticleContent>;
