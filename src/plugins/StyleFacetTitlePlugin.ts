import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createHeadingNode } from "@lexical/rich-text";

const StyleFacetTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeNodeTransform = editor.registerNodeTransform(
      TextNode,
      (textNode) => {
        // Ensure the text node starts with '$'
        if (!textNode.getTextContent().startsWith("$")) {
          return;
        }

        const root = $getRoot();
        const firstChild = root.getFirstChild();
        const parent = textNode.getParent();

        // Ensure the parent of the text node is not the first child of the root
        // and not already styled as a heading
        if (
          parent !== firstChild &&
          parent.getType() !== "heading" &&
          parent instanceof ElementNode
        ) {
          const headingNode = $createHeadingNode("h2");

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

export default StyleFacetTitlePlugin;
