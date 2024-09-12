import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  KEY_ENTER_COMMAND,
  ParagraphNode,
  COMMAND_PRIORITY_HIGH,
} from "lexical";
import { useEffect } from "react";
import { FacetTitleNode } from "../models/FacetTitleNode";

// To keep facet titles as single line, switch the linebreak to a new paragraph for shift + enter
// Whereas the linebreak could be inserted by pressing Shift + Enter then delete the paragraph inserted
// But anyway, the case won't be prevent because the intention is so clear
const AvoidLinebreakInFacetTitle = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeKeybinding = editor.registerCommand(
      KEY_ENTER_COMMAND,
      (payload) => {
        const selection = $getSelection();

        if ($isRangeSelection(selection)) {
          const anchorNode = selection.anchor.getNode();
          const isShiftPressed = payload?.shiftKey === true;

          if (anchorNode instanceof FacetTitleNode && isShiftPressed) {
            const paragraphNode = new ParagraphNode();
            anchorNode.insertAfter(paragraphNode);
            paragraphNode.select();

            return true;
          }
        }
        return false;
      },
      COMMAND_PRIORITY_HIGH
    );

    return () => {
      removeKeybinding;
    };
  }, [editor]);

  return null;
};

export default AvoidLinebreakInFacetTitle;
