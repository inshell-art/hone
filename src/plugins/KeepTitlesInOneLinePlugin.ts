import { useEffect } from "react";
import {
  $createParagraphNode,
  $getSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_ENTER_COMMAND,
  RangeSelection,
  TextNode,
} from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { ArticleTitleNode } from "../models/ArticleTitleNode";
import { FacetTitleNode } from "../models/FacetTitleNode";

const KeepTitlesInOneLinePlugin = () => {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const unregisterEnterCommand = editor.registerCommand(
      KEY_ENTER_COMMAND,
      (event: KeyboardEvent) => {
        const selection = $getSelection();

        if (!selection || !selection.isCollapsed()) {
          return false; // Only handle collapsed selection (when the cursor is in one place)
        }

        const anchorNode = (selection as RangeSelection).anchor.getNode();
        const parent = anchorNode.getParent();

        if (
          parent instanceof ArticleTitleNode ||
          parent instanceof FacetTitleNode
        ) {
          event.preventDefault(); // Prevent the default behavior of the Enter key

          editor.update(() => {
            const offset = (selection as RangeSelection).anchor.offset;

            if (anchorNode instanceof TextNode) {
              const text = anchorNode.getTextContent();
              const before = text.slice(0, offset);
              const after = text.slice(offset);
              const beforeNode = new TextNode(before);
              const afterNode = new TextNode(after);

              anchorNode.replace(beforeNode);

              const paragraphNode = $createParagraphNode();
              paragraphNode.append(afterNode);

              parent.insertAfter(paragraphNode);

              paragraphNode.selectStart();
            }
          });

          return true;
        }

        return false;
      },
      COMMAND_PRIORITY_HIGH,
    );

    return () => {
      unregisterEnterCommand();
    };
  }, [editor]);

  return null;
};

export default KeepTitlesInOneLinePlugin;
