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

  const triggerHonePanel = useCallback(() => {
    const facets = extractFacets();
    facets.forEach((facet) => console.log(facet));

    setFacets(facets);

    setPanelVisible(true); // Show the panel
  }, []);

  const handleClosePanel = useCallback(() => {
    setPanelVisible(false);
  }, []);

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

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isPanelVisible) {
        handleClosePanel();
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
        <div className="facet-panel">
          <div className="facet-panel-header">
            <span>Select a similar facet to insert:</span>
            <button className="close-button" onClick={handleClosePanel}>
              ×
            </button>
          </div>
          <ul className="facet-panel-list">
            {facets.map((facet) => (
              <li key={facet.facetId} className="facet-panel-item">
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
