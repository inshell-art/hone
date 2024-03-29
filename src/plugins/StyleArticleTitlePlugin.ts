import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $getRoot, ElementNode, TextNode, $createTextNode } from "lexical";
import { $createHeadingNode, HeadingNode } from "@lexical/rich-text";

const StyleArticleTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    console.log("StyleArticleTitlePlugin mounted");
    const removeNodeTransform = editor.registerNodeTransform(
      ElementNode,
      (node) => {
        const root = $getRoot();
        const firstChild = root.getFirstChild();

        if (node === firstChild && !(node instanceof HeadingNode)) {
          const headingNode = $createHeadingNode("h1");

          node.getChildren().forEach((child) => {
            if (child instanceof TextNode) {
              // Manually clone by creating a new TextNode with the same content
              const textContent = child.getTextContent();
              const clonedChild = $createTextNode(textContent);
              // Apply any necessary formatting or properties from `child` to `clonedChild`
              headingNode.append(clonedChild);
            }
          });
          node.replace(headingNode);
        }
      }
    );

    return () => {
      removeNodeTransform();
    };
  }, [editor]);

  return null; // This plugin does not render anything
};

export default StyleArticleTitlePlugin;
