import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { PASTE_COMMAND, COMMAND_PRIORITY_HIGH, $getSelection } from "lexical";

const StripFormattingPastePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeListener = editor.registerCommand(
      PASTE_COMMAND,
      (event: ClipboardEvent) => {
        const clipboardData = event.clipboardData;
        if (!clipboardData) {
          return false;
        }

        const text = clipboardData.getData("text/plain");

        event.preventDefault();

        editor.update(() => {
          const selection = $getSelection();
          selection?.insertRawText(text);
        });

        return true;
      },
      COMMAND_PRIORITY_HIGH,
    );

    return () => {
      removeListener();
    };
  }, [editor]);

  return null;
};

export default StripFormattingPastePlugin;
