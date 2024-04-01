import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createHeadingNode } from "@lexical/rich-text";

const TransformFirstTextNodeParentPlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeNodeTransform = editor.registerNodeTransform(
      TextNode,
      (textNode) => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();
        const parent = textNode.getParent();

        if (
          parent === firstChild &&
          parent.getType() !== "heading" &&
          parent instanceof ElementNode
        ) {
          const headingNode = $createHeadingNode("h1");

          parent.getChildren().forEach((child) => {
            headingNode.append(child);
          });

          parent.replace(headingNode);
        }
      },
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor]);

  return null;
};

export default TransformFirstTextNodeParentPlugin;
