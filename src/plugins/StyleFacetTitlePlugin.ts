import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode, $createParagraphNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $createHeadingNode } from "@lexical/rich-text";

const StyleFacetTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeNodeTransform = editor.registerNodeTransform(
      TextNode,
      (textNode) => {
        const root = $getRoot();
        const firstChildOfRoot = root.getFirstChild();
        const parent = textNode.getParent();

        if (parent === firstChildOfRoot || !(parent instanceof ElementNode)) {
          return;
        }

        const isFirstTextNode = (textNode: TextNode) => {
          if (!(parent instanceof ElementNode)) {
            return false;
          }

          let currentNode = parent.getFirstChild();

          while (currentNode) {
            if (currentNode instanceof TextNode) {
              return currentNode === textNode;
            }
            currentNode = currentNode.getNextSibling();
          }
        };

        if (
          isFirstTextNode(textNode) &&
          textNode.getTextContent().startsWith("$") &&
          parent.getType() !== "heading"
        ) {
          const headingNode = $createHeadingNode("h2");

          while (parent.getFirstChild()) {
            const child = parent.getFirstChild();
            if (child !== null) {
              headingNode.append(child);
            }
          }

          parent.replace(headingNode);
        } else if (
          isFirstTextNode(textNode) &&
          !textNode.getTextContent().startsWith("$") &&
          parent.getType() === "heading" // Exhaustive check
        ) {
          const paragraphNode = $createParagraphNode();

          while (parent.getFirstChild()) {
            const child = parent.getFirstChild();
            if (child !== null) {
              paragraphNode.append(child);
            }
          }

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
