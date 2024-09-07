import { HeadingNode, SerializedHeadingNode } from "@lexical/rich-text";

interface SerializedArticleTitleNode extends SerializedHeadingNode {
  type: "article-title";
}

export class ArticleTitleNode extends HeadingNode {
  constructor(key?: string) {
    super("h1", key); // Coupling with h1 for article title
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
