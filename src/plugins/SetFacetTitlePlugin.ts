import { useState, useEffect } from "react";
import { $getRoot, ElementNode, TextNode, $createParagraphNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { EditorProps } from "../types/types";

const SetFacetTitlePlugin: React.FC<EditorProps> = ({ articleId }) => {
  const [editor] = useLexicalComposerContext();
  const [facetIndex, setFacetIndex] = useState(0);

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
          const firstTextNode = parent.getFirstChild();
          return firstTextNode === textNode;
        };

        const generateFacetId = () => {
          setFacetIndex((prevIndex) => prevIndex + 1);

          return `${articleId}-facet-${facetIndex}`;
        };

        if (
          isFirstTextNode(textNode) &&
          textNode.getTextContent().startsWith("$") &&
          parent.getType() !== "facet-title"
        ) {
          const uniqueId = generateFacetId();
          const facetTitleNode = new FacetTitleNode(uniqueId);

          parent.getChildren().forEach((child) => {
            facetTitleNode.append(child);
          });

          parent.replace(facetTitleNode);
        } else if (
          isFirstTextNode(textNode) &&
          !textNode.getTextContent().startsWith("$") &&
          parent.getType() === "facet-title" // Exhaustive check
        ) {
          // Deactivate the facet title node to remain the data about it
          (parent as FacetTitleNode).setActive(false);

          // Destroy the facet title node if it is empty
          if (textNode.getTextContent().length === 0) {
            const paragraphNode = $createParagraphNode();

            parent.getChildren().forEach((child) => {
              paragraphNode.append(child);
            });

            parent.replace(paragraphNode);
          }
        }
      },
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor, articleId, facetIndex]);

  return null;
};

export default SetFacetTitlePlugin;
