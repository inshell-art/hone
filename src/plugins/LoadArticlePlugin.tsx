import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { EditorProps } from "../types/types";

const LoadArticlePlugin: React.FC<EditorProps> = ({ articleId }) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const storedArticles = localStorage.getItem("HoneEditorArticles");

    if (!storedArticles) {
      return;
    }

    if (storedArticles) {
      try {
        const parsedArticles = JSON.parse(storedArticles);
        const articleContent = parsedArticles[articleId];

        if (articleContent) {
          editor.update(() => {
            const editorState = editor.parseEditorState(articleContent);
            editor.setEditorState(editorState);
          });
        } else {
          console.log(`No content found for article ID: ${articleId}`);
          return;
        }
      } catch (error) {
        console.error("Failed to parse the stored articles", error);
      }
    }
  }, [articleId, editor]);

  return null;
};

export default LoadArticlePlugin;
