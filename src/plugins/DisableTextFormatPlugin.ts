import { useEffect } from "react";
import {
  FORMAT_TEXT_COMMAND,
  TextFormatType,
  COMMAND_PRIORITY_HIGH,
} from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";

const DisableTextFormatPlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeCommandListener = editor.registerCommand<TextFormatType>(
      FORMAT_TEXT_COMMAND,
      (): boolean => {
        return true;
      },
      COMMAND_PRIORITY_HIGH,
    );

    return () => {
      removeCommandListener();
    };
  }, [editor]);

  return null;
};

export default DisableTextFormatPlugin;
