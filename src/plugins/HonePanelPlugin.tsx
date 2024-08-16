import { useEffect, useCallback, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_ENTER_COMMAND,
  LexicalNode,
} from "lexical";
import { HeadingNode } from "@lexical/rich-text";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";

const HonePanelPlugin = () => {
  const [editor] = useLexicalComposerContext();
  const [isPanelVisible, setPanelVisible] = useState(false);
  const [facets, setFacets] = useState<Facet[]>([]);
  const [panelPosition, setPanelPosition] = useState({ top: 0, left: 0 });
  const [panelWidth, setPanelWidth] = useState(0);

  const triggerHonePanel = useCallback(() => {
    const selection = $getSelection();
    const facets = extractFacets();
    setFacets(facets);

    if ($isRangeSelection(selection)) {
      const anchorNode = selection.anchor.getNode();
      const anchorKey = anchorNode.getKey();
      const element = editor.getElementByKey(anchorKey);

      if (element) {
        const rect = element.getBoundingClientRect();
        const editorElement = document.querySelector(".editor-container");
        const editorRect = editorElement?.getBoundingClientRect();

        if (rect && editorRect) {
          const topPosition = rect.top - editorRect.top;

          setPanelPosition({
            top: topPosition,
            left: rect.left - editorRect.left,
          });
          setPanelWidth(editorRect.width);

          // Scroll to the top of the editor to prevent the panel from being hidden
          if (rect.top < 0 || rect.bottom > window.innerHeight) {
            const panelTopPosition = rect.top + window.scrollY - 50;
            setTimeout(() => {
              window.scrollTo({
                top: panelTopPosition,
                behavior: "smooth",
              });
            }, 100);
          }

          editor.setEditable(false);
          setPanelVisible(true);
        }
      }
    }
  }, [editor]);

  const handleClosePanel = useCallback(() => {
    // Unlock the editor
    editor.setEditable(true);
    setPanelVisible(false);
  }, [editor]);

  // Register the command to trigger the panel
  useEffect(() => {
    const isMacOS = /Mac|iPod|iPhone|iPad/.test(navigator.userAgent);

    const unregisterCommand = editor.registerCommand(
      KEY_ENTER_COMMAND,
      (event: KeyboardEvent) => {
        // Detect Command + Enter on macOS, or Ctrl + Enter on Windows/Linux
        if ((isMacOS && event.metaKey) || (!isMacOS && event.ctrlKey)) {
          const selection = $getSelection();

          if ($isRangeSelection(selection)) {
            const anchorNode = selection.anchor.getNode();

            // Traverse previous nodes to check if there's an `h2` heading before the current selection
            let currentNode: LexicalNode | null = anchorNode;
            while (currentNode) {
              if (
                currentNode instanceof HeadingNode &&
                currentNode.getTag() === "h2"
              ) {
                triggerHonePanel();
                return true;
              }
              currentNode = currentNode.getPreviousSibling();
            }
          }
        }
        return false;
      },
      COMMAND_PRIORITY_HIGH
    );

    // Event listener for closing the panel with Escape key
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isPanelVisible) {
        handleClosePanel();
        editor.focus(); // Return focus to the editor
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      unregisterCommand();
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [editor, triggerHonePanel, handleClosePanel, isPanelVisible]);

  return (
    <>
      {isPanelVisible && (
        <div
          className="hone-panel"
          style={{
            top: panelPosition.top,
            left: panelPosition.left,
            width: panelWidth,
          }}
        >
          <div className="hone-panel-header">
            <span>Select a similar facet to insert:</span>
            <button
              className="hone-panel-close-button"
              onClick={handleClosePanel}
            >
              ×
            </button>
          </div>
          <ul className="hone-panel-list">
            {facets.map((facet) => (
              <li key={facet.facetId} className="hone-panel-item">
                {facet.title}
              </li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
};

export default HonePanelPlugin;
