import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { debounce } from "lodash";
import { AutoSavePluginProps } from "../types/types";
import { collectTextFromDescendants } from "../utils/utils";
import { useNavigate } from "react-router-dom";

const AutoSavePlugin: React.FC<AutoSavePluginProps> = ({
  articleId,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();
  const navigate = useNavigate();

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
          localStorage.getItem("honeData") || "{}",
        );

        // If there is no text and the article is not in localStorage, skip the save
        if (hasNoText && !savedArticles[articleId]) {
          onMessageChange("Skipping auto-save: content has no text.", true);
          return;
        }

        // If there is no text and the article is in localStorage, delete the article
        if (hasNoText && savedArticles[articleId]) {
          const deleteConfirmed = window.confirm(
            "Empty content means to delete the article. Confirm?",
          );

          if (!deleteConfirmed) {
            const articleContent = savedArticles[articleId].content;
            editor.update(() => {
              const editorState = editor.parseEditorState(articleContent);
              editor.setEditorState(editorState);
            });

            onMessageChange(
              "Canceled deletion of article from localStorage.",
              true,
            );
            return;
          } else {
            delete savedArticles[articleId];
            onMessageChange("Deleted article from localStorage.", true);
            localStorage.setItem("honeData", JSON.stringify(savedArticles));
            navigate("/");
            return;
          }
        }

        // Update the specific article's content
        onMessageChange("Saving content to localStorage...");
        const dateTimeNow = Date.now();
        savedArticles[articleId] = { content, updatedAt: dateTimeNow };
        //! No version number for the first data shape, will be added for necessity if needed
        //! Where handling the current version number as null, and the second version number as 1

        // Save the updated articles back to localStorage
        localStorage.setItem("honeData", JSON.stringify(savedArticles));
        onMessageChange(
          "Auto-saved changes to localStorage in 1 second.",
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
  }, [editor, articleId, onMessageChange, navigate]);

  return null;
};

export default AutoSavePlugin;
