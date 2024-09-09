import { useEffect, useCallback, useState, useRef } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_ENTER_COMMAND,
  $isTextNode,
} from "lexical";
import { extractFacets } from "../utils/extractFacets";
import { Facet } from "../types/types";

const HonePanelPlugin = () => {
  const [editor] = useLexicalComposerContext();
  const [isPanelVisible, setPanelVisible] = useState(false);
  const [isPanelTriggered, setPanelTriggered] = useState(false);
  const [facets, setFacets] = useState<Facet[]>([]);
  const [panelPosition, setPanelPosition] = useState({ top: 0, left: 0 });
  const [panelWidth, setPanelWidth] = useState(0);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const panelListRef = useRef<HTMLUListElement | null>(null);
  const [disableMouseOver, setDisableMouseOver] = useState(false);

  const triggerHonePanel = useCallback(() => {
    const selection = $getSelection();
    const facets = extractFacets();
    setFacets(facets);
    console.log(facets);

    if ($isRangeSelection(selection)) {
      const anchorNode = selection.anchor.getNode();
      const anchorKey = anchorNode.getKey();
      const element = editor.getElementByKey(anchorKey);

      if (element) {
        const rect = element.getBoundingClientRect();
        const editorElement = document.querySelector(
          ".editor-container",
        ) as HTMLElement;

        const editorRect = editorElement?.getBoundingClientRect();

        if (rect && editorRect) {
          const topPosition = rect.top - editorRect.top;

          setPanelPosition({
            top: topPosition,
            left: rect.left - editorRect.left,
          });
          setPanelWidth(editorRect.width);
          setSelectedIndex(0);

          editor.setEditable(false);
          setPanelVisible(true);
        }
      }
    }
  }, [editor]);

  // Scrolling to show panel entirely, hiding scrollbar and preventing layout shift
  useEffect(() => {
    if (isPanelVisible) {
      const panelElement = document.querySelector(".hone-panel") as HTMLElement;
      if (panelElement) {
        const rect = panelElement.getBoundingClientRect();

        if (rect.top < 0 || rect.bottom > window.innerHeight) {
          requestAnimationFrame(() => {
            const panelTopPosition = rect.top + window.scrollY - 50;
            setTimeout(() => {
              window.scrollTo({
                top: panelTopPosition,
                behavior: "smooth",
              });
            }, 100);
          });
        }

        // Calculate and set padding to prevent layout shift
        const scrollbarWidth =
          window.innerWidth - document.documentElement.clientWidth;
        document.body.style.paddingRight = `${scrollbarWidth}px`;
        document.body.style.overflow = "hidden";
      }
    }
  }, [isPanelVisible]);

  const handleMouseOver = (index: number) => {
    if (!disableMouseOver) {
      setSelectedIndex(index);
    }
  };

  const handleMouseMove = () => {
    setDisableMouseOver(false);
  };

  const handleClosePanel = useCallback(() => {
    setPanelVisible(false);
    editor.setEditable(true);
    editor.focus();

    const scrollbarSpacer = document.getElementById("scrollbar-spacer");
    if (scrollbarSpacer) {
      scrollbarSpacer.remove();
    }
    document.body.style.overflow = ""; // Restore body overflow
    document.body.style.paddingRight = ""; // Remove added padding
  }, [editor]);

  useEffect(() => {
    if (isPanelVisible && selectedIndex !== null && panelListRef.current) {
      const selectedItem = panelListRef.current.children[
        selectedIndex
      ] as HTMLElement;

      selectedItem.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
      });
    }
  }, [selectedIndex, isPanelVisible]);

  const insertFacet = useCallback(
    (facet: Facet) => {
      editor.update(() => {
        const selection = $getSelection();
        if ($isRangeSelection(selection)) {
          // Insert the facet title as text
          selection.insertText(facet.title);

          // Collapse the selection to the end of the inserted text
          const anchorNode = selection.anchor.getNode();
          if ($isTextNode(anchorNode)) {
            const anchorOffset = selection.anchor.offset + facet.title.length;
            selection.setTextNodeRange(
              anchorNode,
              anchorOffset,
              anchorNode,
              anchorOffset,
            );
          }
        }
      });

      handleClosePanel(); // Close the panel after insertion
    },
    [editor, handleClosePanel],
  );

  // Trigger the panel at the cursor position is at the beginning of the line and the node is empty
  useEffect(() => {
    const isMacOS = /Mac|iPod|iPhone|iPad/.test(navigator.userAgent);

    const unregisterCommand = editor.registerCommand(
      KEY_ENTER_COMMAND,
      (event: KeyboardEvent) => {
        // Detect Command + Enter on macOS, or Ctrl + Enter on Windows/Linux
        if ((isMacOS && event.metaKey) || (!isMacOS && event.ctrlKey)) {
          const selection = $getSelection();
          event.preventDefault();

          if ($isRangeSelection(selection)) {
            const anchorOffset = selection.anchor.offset;
            const anchorNode = selection.anchor.getNode();
            if (
              anchorOffset === 0 &&
              anchorNode.getTextContent().trim() === ""
            ) {
              triggerHonePanel();
              return true;
            }
          }
        }
        return false;
      },
      COMMAND_PRIORITY_HIGH,
    );

    return () => {
      unregisterCommand();
    };
  }, [editor, triggerHonePanel]);

  // handle key down events while the panel is open
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isPanelVisible) {
        handleClosePanel();
      } else if (event.key === "ArrowDown" && isPanelVisible) {
        event.preventDefault(); // Prevent scrolling the page
        setDisableMouseOver(true);
        setSelectedIndex((prevIndex) => {
          if (prevIndex === null || prevIndex === facets.length - 1) {
            return 0; // Start from the top
          }
          return prevIndex + 1; // Move down
        });
      } else if (event.key === "ArrowUp" && isPanelVisible) {
        event.preventDefault(); // Prevent scrolling the page
        setDisableMouseOver(true);
        setSelectedIndex((prevIndex) => {
          if (prevIndex === null || prevIndex === 0) {
            return facets.length - 1; // Move to the bottom
          }
          return prevIndex - 1; // Move up
        });
      } else if (
        event.key === "Enter" &&
        isPanelTriggered &&
        isPanelVisible &&
        selectedIndex !== null
      ) {
        event.preventDefault(); // Prevent form submission
        insertFacet(facets[selectedIndex]);
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [
    isPanelVisible,
    facets,
    selectedIndex,
    insertFacet,
    handleClosePanel,
    isPanelTriggered,
  ]);

  // useEffect to record the panel is trigger for 100ms to avoid enter detection on panel open
  useEffect(() => {
    let timer: number;

    if (isPanelVisible) {
      timer = window.setTimeout(() => {
        setPanelTriggered(true);
      }, 100);

      return () => {
        clearTimeout(timer);
        setPanelTriggered(false);
      };
    } else {
      setPanelTriggered(false);
    }
  }, [isPanelVisible]);

  return (
    <>
      {isPanelVisible && (
        <>
          <div className="editor-overlay" onClick={handleClosePanel}></div>
          <div
            className="hone-panel"
            style={{
              top: panelPosition.top,
              left: panelPosition.left,
              width: panelWidth,
            }}
            onMouseMove={handleMouseMove}
          >
            <div className="hone-panel-header">
              <span>Select a facet to insert:</span>
            </div>
            <ul className="hone-panel-list" ref={panelListRef}>
              {facets.map((facet, index) => (
                <li
                  key={facet.facetId}
                  className={`hone-panel-item ${
                    index === selectedIndex ? "selected" : ""
                  }`}
                  onMouseOver={() => handleMouseOver(index)}
                  onClick={() => insertFacet(facet)}
                >
                  {facet.title}
                </li>
              ))}
            </ul>
          </div>
        </>
      )}
    </>
  );
};

export default HonePanelPlugin;
