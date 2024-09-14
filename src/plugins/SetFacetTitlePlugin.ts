import { useEffect } from "react";
import { $getRoot, ElementNode, TextNode, $createParagraphNode } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { EditorProps } from "../types/types";

const SetFacetTitlePlugin: React.FC<EditorProps> = ({ articleId }) => {
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
          const firstTextNode = parent.getFirstChild();
          return firstTextNode === textNode;
        };

        if (!isFirstTextNode(textNode)) {
          return;
        }

        const generateFacetId = () => {
          const timestamp = new Date().getTime();

          return `${articleId}-facet-${timestamp}`;
        };

        editor.update(() => {
          // Initialize a facet title node
          if (
            textNode.getTextContent().startsWith("$") &&
            !(parent instanceof FacetTitleNode)
          ) {
            const uniqueId = generateFacetId();
            const facetTitleNode = new FacetTitleNode(uniqueId);
            console.log("facetTitleNode:", facetTitleNode);

            parent.getChildren().forEach((child) => {
              facetTitleNode.append(child);
            });

            parent.replace(facetTitleNode);
          }

          // Update the facet title node
          if (parent instanceof FacetTitleNode) {
            // Destroy the facet title node if it is empty
            if (textNode.getTextContent().length === 0) {
              const paragraphNode = $createParagraphNode();

              parent.getChildren().forEach((child) => {
                paragraphNode.append(child);
              });

              parent.replace(paragraphNode);
            }

            // Deactivate the facet title node with $ prefix not present
            if (
              !textNode.getTextContent().startsWith("$") &&
              (parent as FacetTitleNode).__active
            ) {
              const newFacetTitleNode = new FacetTitleNode(
                parent.__uniqueId,
                false,
                parent.__honedBy,
              );

              parent.getChildren().forEach((child) => {
                newFacetTitleNode.append(child);
              });

              parent.replace(newFacetTitleNode);
            }

            // Reactivate the facet title node
            if (
              textNode.getTextContent().startsWith("$") &&
              !(parent as FacetTitleNode).__active
            ) {
              const newFacetTitleNode = new FacetTitleNode(
                parent.__uniqueId,
                true,
                parent.__honedBy,
              );

              parent.getChildren().forEach((child) => {
                newFacetTitleNode.append(child);
              });

              parent.replace(newFacetTitleNode);
            }
          }
        });
      },
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor, articleId]);

  return null;
};

export default SetFacetTitlePlugin;
