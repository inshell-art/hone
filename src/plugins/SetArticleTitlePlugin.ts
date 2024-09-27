import { useEffect } from "react";
import { $getRoot, ElementNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ArticleTitleNode } from "../models/ArticleTitleNode";

const SetArticleTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const unregisterUpdateListener = editor.registerUpdateListener(() => {
      editor.update(() => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();

        // Ensure the first child exists and is an ElementNode
        if (firstChild && firstChild instanceof ElementNode) {
          if (firstChild.getType() !== "article-title") {
            const articleTitleNode = new ArticleTitleNode();

            firstChild.getChildren().forEach((child) => {
              articleTitleNode.append(child);
            });

            firstChild.replace(articleTitleNode);
          }
        }
      });
    });

    return () => {
      unregisterUpdateListener();
    };
  }, [editor]);

  return null;
};

export default SetArticleTitlePlugin;
