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
          firstChild === null ||
          parent !== firstChild ||
          parent.getType() === "heading" ||
          !(parent instanceof ElementNode)
        ) {
          return;
        }

        const headingNode = $createHeadingNode("h1");

        while (parent.getFirstChild()) {
          const child = parent.getFirstChild();
          if (child !== null) {
            headingNode.append(child);
          }
        }

        parent.replace(headingNode);
      }
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor]);

  return null;
};

export default TransformFirstTextNodeParentPlugin;
