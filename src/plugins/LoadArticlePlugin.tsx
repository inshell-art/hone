import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { EditorProps } from "../types/types";

const LoadArticlePlugin: React.FC<EditorProps> = ({ articleId }) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const storedArticles = localStorage.getItem("HoneEditorArticles");
    if (storedArticles) {
      try {
        const parsedArticles = JSON.parse(storedArticles);
        const articleContent = parsedArticles[articleId];

        console.log("parsedArticles", parsedArticles);
        console.log("articleContent", articleContent);

        if (articleContent) {
          editor.update(() => {
            const editorState = editor.parseEditorState(articleContent);
            editor.setEditorState(editorState);
          });
        } else {
          console.error("Article content is not properly serialized.");
        }
      } catch (error) {
        console.error("Failed to parse the stored articles", error);
      }
    }
  }, [articleId, editor]);

  return null;
};

export default LoadArticlePlugin;
