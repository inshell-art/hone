// PlainTextPastePlugin.tsx
import { useEffect } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  PASTE_COMMAND,
  $getSelection,
  $isRangeSelection,
  $createParagraphNode,
  $createTextNode,
  TextNode,
  RangeSelection,
  COMMAND_PRIORITY_HIGH,
} from "lexical";

// Handle pasting plain text content into the editor
// Strip out any formatting from the pasted text
// Converts the pasted text with new line into paragraphs
const HandlePastePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const removeListener = editor.registerCommand(
      PASTE_COMMAND,
      (event: ClipboardEvent) => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) {
          console.log("WWWWWWWWWWW");
          return false;
        }
        event.preventDefault();

        const clipboardData = event.clipboardData;
        console.log("clipboardData:", clipboardData);
        console.log("available types:", clipboardData?.types);
        if (!clipboardData) {
          return false;
        }

        const text = clipboardData.getData("text/plain");
        console.log("Pasted text:", text);

        editor.focus();

        editor.update(() => {
          const lines = text.split(/\r?\n/);
          console.log("lines:", lines);

          const anchorNode = (selection as RangeSelection).anchor.getNode();
          const insertionNode = anchorNode.getTopLevelElementOrThrow();
          console.log("anchorNode:", anchorNode);
          console.log("insertionNode1:", insertionNode);

          let referenceNode = insertionNode;
          let lastInsertedNode: TextNode | null = null;

          lines.forEach((lineText: string) => {
            const paragraphNode = $createParagraphNode();
            const textNode = $createTextNode(lineText);
            paragraphNode.append(textNode);

            referenceNode.insertAfter(paragraphNode);
            referenceNode = paragraphNode;
            lastInsertedNode = textNode;
          });

          if (lastInsertedNode) {
            (lastInsertedNode as TextNode).select();
          }
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

export default HandlePastePlugin;
