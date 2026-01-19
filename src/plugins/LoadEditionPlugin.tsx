import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  getEditionByVersion,
  loadArticleEditions,
} from "../utils/articleEditions";

type LoadEditionPluginProps = {
  articleId: string;
  version: number;
  onMessageChange?: (message: string | null, isTemporary?: boolean) => void;
};

const LoadEditionPlugin: React.FC<LoadEditionPluginProps> = ({
  articleId,
  version,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const publishState = loadArticleEditions();
    const edition = getEditionByVersion(publishState, articleId, version);

    if (!edition) {
      onMessageChange?.("No edition found at this link.", true);
      return;
    }

    editor.update(() => {
      const editorState = editor.parseEditorState(edition.content);
      editor.setEditorState(editorState);
    });
    onMessageChange?.(`Loaded v${version}.`, true);
  }, [articleId, editor, onMessageChange, version]);

  return null;
};

export default LoadEditionPlugin;
