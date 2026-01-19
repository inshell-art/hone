import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { LoadArticlePluginProps } from "../types/types";
import { HONE_DATA_KEY } from "../constants/storage";

const LoadArticlePlugin: React.FC<LoadArticlePluginProps> = ({
  articleId,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  useEffect(() => {
    onMessageChange("Loading content from localStorage...");
    if (!isEditable) {
      return;
    }

    const storedArticles = localStorage.getItem(HONE_DATA_KEY);

    if (!storedArticles) {
      console.log("!storedArticles");
      return;
    }

    if (storedArticles) {
      try {
        const parsedArticles = JSON.parse(storedArticles);
        const article = parsedArticles[articleId];

        if (!article) {
          console.log("no article!");
          onMessageChange(
            `No article found for article ID: ${articleId}`,
            true,
          );

          return;
        }

        const articleContent = parsedArticles[articleId].content;
        console.log("articleContent:", articleContent);

        if (articleContent) {
          editor.update(() => {
            console.log("editor.update");
            const editorState = editor.parseEditorState(articleContent);
            editor.setEditorState(editorState);
          });
          onMessageChange("Loaded content from localStorage.", true);
        } else {
          onMessageChange(
            `No content found for article ID: ${articleId}`,
            true,
          );
          return;
        }
      } catch (error) {
        onMessageChange("Failed to load content from localStorage.", true);
      }
    }
  }, [articleId, editor, onMessageChange, isEditable]);

  return null;
};

export default LoadArticlePlugin;
