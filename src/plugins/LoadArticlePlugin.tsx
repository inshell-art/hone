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
        console.log(`Loaded content for article ID: ${articleId}`);

        if (articleContent) {
          console.log(`Parsing editor state for article ID: ${articleId}`);
          console.log(`Article content: ${articleContent}`);
          editor.update(() => {
            console.log("before parseEditorState");
            const editorState = editor.parseEditorState(articleContent);
            console.log("after parseEditorState");
            console.log(`Setting editor state for article ID: ${articleId}`);
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
