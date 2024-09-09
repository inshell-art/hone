import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { debounce } from "lodash";
import { EditorProps } from "../types/types";
import { SerializedElementNode, SerializedLexicalNode } from "lexical";

const AutoSavePlugin: React.FC<EditorProps> = ({ articleId }) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const saveContent = debounce(async () => {
      try {
        const content = editor.getEditorState().toJSON();

        // Type guard to check if the node is a SerializedElementNode
        const isElementNode = (
          node: SerializedLexicalNode,
        ): node is SerializedElementNode => {
          return "children" in node && Array.isArray(node.children);
        };

        // Check if the root node is empty
        const isEmptyContent =
          isElementNode(content.root) &&
          (content.root.children.length === 0 ||
            (content.root.children.length === 1 &&
              isElementNode(content.root.children[0]) &&
              content.root.children[0].children.length === 0));

        if (isEmptyContent) {
          console.log("Skipping auto-save: content is empty.");
          return;
        }

        // Retrieve the existing articles from localStorage
        const savedArticles = JSON.parse(
          localStorage.getItem("HoneEditorArticles") || "{}",
        );

        // Update the specific article's content
        savedArticles[articleId] = content;

        // Save the updated articles back to localStorage
        localStorage.setItem(
          "HoneEditorArticles",
          JSON.stringify(savedArticles),
        );
      } catch (error) {
        console.error("Failed to auto-save content to localStorage:", error);
      }
    }, 1000);

    const handleChange = () => {
      saveContent();
    };

    const unregister = editor.registerUpdateListener(handleChange);

    return () => {
      unregister();
    };
  }, [editor, articleId]);

  return null;
};

export default AutoSavePlugin;
