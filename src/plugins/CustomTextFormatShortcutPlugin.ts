import { useEffect } from "react";
import { FORMAT_TEXT_COMMAND } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";

const CustomCodeShortcutPlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === "e") {
        // Replace 'c' with your desired key
        event.preventDefault(); // Prevent default behavior
        editor.dispatchCommand(FORMAT_TEXT_COMMAND, "code");
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [editor]);

  return null;
};

export default CustomCodeShortcutPlugin;
