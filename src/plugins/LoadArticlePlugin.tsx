import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { LoadArticlePluginProps } from "../types/types";

const LoadArticlePlugin: React.FC<LoadArticlePluginProps> = ({
  articleId,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    onMessageChange("Loading content from localStorage...");
    const storedArticles = localStorage.getItem("HoneEditorArticles");

    if (!storedArticles) {
      return;
    }

    if (storedArticles) {
      try {
        const parsedArticles = JSON.parse(storedArticles);
        const articleContent = parsedArticles[articleId].content;

        if (articleContent) {
          editor.update(() => {
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
  }, [articleId, editor, onMessageChange]);

  return null;
};

export default LoadArticlePlugin;
