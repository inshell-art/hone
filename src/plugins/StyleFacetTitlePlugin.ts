import { useEffect } from "react";
import { $createParagraphNode, $getRoot, ElementNode, TextNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createHeadingNode } from "@lexical/rich-text";

const StyleFacetTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeNodeTransform = editor.registerNodeTransform(
      TextNode,
      (textNode) => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();
        const parent = textNode.getParent();

        if (parent === firstChild || !(parent instanceof ElementNode)) {
          return;
        }

        if (
          textNode.getTextContent().startsWith("$") &&
          parent.getType() !== "heading"
        ) {
          const headingNode = $createHeadingNode("h2");

          parent.getChildren().forEach((child) => {
            headingNode.append(child);
          });

          parent.replace(headingNode);
        } else if (
          !textNode.getTextContent().startsWith("$") &&
          parent.getType() === "heading" // Exhaustive check
        ) {
          const paragraphNode = $createParagraphNode();

          parent.getChildren().forEach((child) => {
            paragraphNode.append(child);
          });

          parent.replace(paragraphNode);
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
