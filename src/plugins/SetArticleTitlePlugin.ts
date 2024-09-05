import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ArticleTitleNode } from "../models/ArticleTitleNode";

const SetArticleTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeNodeTransform = editor.registerNodeTransform(
      TextNode,
      (textNode) => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();
        const parent = textNode.getParent();

        if (
          firstChild === null ||
          parent !== firstChild ||
          parent.getType() === "article-title" ||
          !(parent instanceof ElementNode)
        ) {
          return;
        }

        const articleTitleNode = new ArticleTitleNode();

        parent.getChildren().forEach((child) => {
          articleTitleNode.append(child);
        });

        parent.replace(articleTitleNode);
      },
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor]);

  return null;
};

export default SetArticleTitlePlugin;
