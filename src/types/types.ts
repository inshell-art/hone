type TextNode = {
  detail: number;
  format: number;
  mode: string;
  style: string;
  text: string;
  type: string;
  version: number;
};

type HeadingNode = {
  children: TextNode[];
  direction: string;
  format: string;
  indent: number;
  type: string;
  version: number;
  tag: string;
};

type RootNode = {
  children: HeadingNode[];
  direction: string;
  format: string;
  indent: number;
  type: string;
  version: number;
};

export type ArticleContent = {
  root: RootNode;
};

export type Article = {
  id: string;
  title: string;
  content: ArticleContent;
};

export type EditorProps = {
  articleId: string;
};
