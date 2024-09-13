import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  INSERT_LINE_BREAK_COMMAND,
  COMMAND_PRIORITY_HIGH,
  $getSelection,
  $isRangeSelection,
} from "lexical";
import { useEffect } from "react";
import { FacetTitleNode } from "../models/FacetTitleNode"; // Adjust to your custom node

const DisableLineBreakInFacetTitlePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    // Register a command to prevent line breaks inside FacetTitleNode
    const removeLineBreakCommand = editor.registerCommand(
      INSERT_LINE_BREAK_COMMAND,
      () => {
        const selection = $getSelection();

        if ($isRangeSelection(selection)) {
          const anchorNode = selection.anchor.getNode();

          // Prevent line breaks if the node is a FacetTitleNode
          if (
            anchorNode instanceof FacetTitleNode ||
            anchorNode.getParent() instanceof FacetTitleNode
          ) {
            return true;
          }
        }

        return false; // Allow line breaks in other nodes
      },
      COMMAND_PRIORITY_HIGH
    );

    return () => {
      removeLineBreakCommand();
    };
  }, [editor]);

  return null;
};

export default DisableLineBreakInFacetTitlePlugin;
