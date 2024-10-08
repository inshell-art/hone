import { HeadingNode, SerializedHeadingNode } from "@lexical/rich-text";

export interface SerializedArticleTitleNode extends SerializedHeadingNode {
  type: "article-title";
}

export class ArticleTitleNode extends HeadingNode {
  constructor(key?: string) {
    super("h1", key);
  }

  static getType() {
    return "article-title";
  }

  static clone(node: ArticleTitleNode) {
    return new ArticleTitleNode(node.__key);
  }

  exportJSON(): SerializedArticleTitleNode {
    return {
      ...super.exportJSON(),
      type: "article-title",
    };
  }

  static importJSON() {
    return new ArticleTitleNode();
  }
}
