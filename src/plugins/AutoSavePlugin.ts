import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { debounce } from "lodash";
import { AutoSavePluginProps } from "../types/types";
import { collectTextFromDescendants } from "../utils/utils";

const AutoSavePlugin: React.FC<AutoSavePluginProps> = ({
  articleId,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const saveContent = debounce(async () => {
      try {
        const content = editor.getEditorState().toJSON();
        const collectText: string[] = [];

        // Populate collectText by collecting text from the editor's descendants
        collectTextFromDescendants(content.root, collectText);
        const hasNoText = collectText.length === 0;

        // Retrieve the existing articles from localStorage
        const savedArticles = JSON.parse(
          localStorage.getItem("HoneEditorArticles") || "{}",
        );

        // If there is no text and the article is not in localStorage, skip the save
        if (hasNoText && !savedArticles[articleId]) {
          onMessageChange("Skipping auto-save: content has no text.", true);
          return;
        }

        // If there is no text and the article is in localStorage, delete the article
        if (hasNoText && savedArticles[articleId]) {
          delete savedArticles[articleId];
          onMessageChange("Deleted article from localStorage.", true);
          localStorage.setItem(
            "HoneEditorArticles",
            JSON.stringify(savedArticles),
          );
          return;
        }

        // Update the specific article's content
        onMessageChange("Saving content to localStorage...");
        const dateTimeNow = new Date().toISOString();
        savedArticles[articleId] = { content, updatedAt: dateTimeNow };

        // Save the updated articles back to localStorage
        localStorage.setItem(
          "HoneEditorArticles",
          JSON.stringify(savedArticles),
        );
        onMessageChange(
          "Auto-saved changes to localStorage every 1 second.",
          true,
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
  }, [editor, articleId, onMessageChange]);

  return null;
};

export default AutoSavePlugin;
