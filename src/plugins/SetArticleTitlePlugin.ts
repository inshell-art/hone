import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ArticleTitleNode } from "../models/ArticleTitleNode";

const SetArticleTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    // Register a command for inserting text (i.e., when user types)
    const removeNodeTransform = editor.registerNodeTransform(TextNode, () => {
      editor.update(() => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();

        // Only proceed if there is content being added (text node insertion)
        if (!firstChild || firstChild.getTextContent().trim() === "") {
          return false; // Exit early if there's no meaningful content
        }

        // Ensure the first child exists and is an ElementNode
        if (
          firstChild instanceof ElementNode &&
          firstChild.getType() !== "article-title"
        ) {
          const articleTitleNode = new ArticleTitleNode();

          // Move children into the new article title node
          firstChild.getChildren().forEach((child) => {
            articleTitleNode.append(child);
          });

          // Replace the first child with the article title node
          firstChild.replace(articleTitleNode);
        }
      });

      return true; // Returning true means the command was handled
    });

    return () => {
      // Cleanup on unmount
      removeNodeTransform();
    };
  }, [editor]);

  return null;
};

export default SetArticleTitlePlugin;
