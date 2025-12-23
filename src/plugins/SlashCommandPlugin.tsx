import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import {
  $createParagraphNode,
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $createRangeSelection,
  $getNodeByKey,
  $setSelection,
  $isTextNode,
  COMMAND_PRIORITY_LOW,
  KEY_DOWN_COMMAND,
  SELECTION_CHANGE_COMMAND,
} from "lexical";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { FacetLibraryItem, FacetsLibraryState, FacetId } from "../types/types";
import { addHoneEdge, loadLibrary, upsertFacet } from "../utils/facetLibrary";
import {
  findNearestFacetTitleNode,
  getJaccardSimilarity,
} from "../utils/utils";
import { FACET_LIBRARY_KEY } from "../constants/storage";

type FacetSnapshot = {
  facetId: FacetId;
  title: string;
  bodyText: string;
};

type SlashCommandPluginProps = {
  articleId: string;
  onMessageChange: (message: string | null, isTemporary?: boolean) => void;
};

type CommandOption = {
  id: "facet" | "update" | "hone";
  title: string;
  description: string;
};

type HoneCandidate = FacetLibraryItem & { similarity: number };

const SlashCommandPlugin: React.FC<SlashCommandPluginProps> = ({
  articleId,
  onMessageChange,
}) => {
  const [editor] = useLexicalComposerContext();
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [paletteMode, setPaletteMode] = useState<"commands" | "hone">(
    "commands",
  );
  const [palettePosition, setPalettePosition] = useState<{
    top: number;
    left: number;
  }>({ top: 16, left: 0 });
  const [library, setLibrary] = useState<FacetsLibraryState>(loadLibrary());
  const [honeCandidates, setHoneCandidates] = useState<HoneCandidate[]>([]);
  const [targetFacet, setTargetFacet] = useState<FacetSnapshot | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const activeFacetElementRef = useRef<HTMLElement | null>(null);
  const savedSelectionRef = useRef<{
    key: string;
    offset: number;
    type: "text" | "element";
  } | null>(null);
  const [activeFacetStatus, setActiveFacetStatus] = useState<{
    key: string;
    state: "updated" | "draft";
  } | null>(null);

  const commandOptions: CommandOption[] = useMemo(
    () => [
      {
        id: "facet",
        title: "/facet",
        description: "Create a new facet.",
      },
      {
        id: "update",
        title: "/update",
        description: "Update this facet to the library.",
      },
      {
        id: "hone",
        title: "/hone",
        description: "Hone this facet with another library facet.",
      },
    ],
    [],
  );

  const captureSelection = useCallback(() => {
    editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection)) {
        savedSelectionRef.current = null;
        return;
      }

      const anchor = selection.anchor;
      const anchorNode = anchor.getNode();
      const anchorType: "text" | "element" = $isTextNode(anchorNode)
        ? "text"
        : "element";

      savedSelectionRef.current = {
        key: anchorNode.getKey(),
        offset: anchor.offset,
        type: anchorType,
      };

      const domSelection = window.getSelection();
      let rect: DOMRect | undefined;

      if (domSelection && domSelection.rangeCount > 0) {
        const range = domSelection.getRangeAt(0).cloneRange();
        range.collapse(true);
        const clientRect = Array.from(range.getClientRects()).find(
          (r) => r.width || r.height,
        );
        rect = clientRect ?? range.getBoundingClientRect();
      }

      if ((!rect || (rect.width === 0 && rect.height === 0)) && anchorNode) {
        const anchorElement = editor.getElementByKey(anchorNode.getKey());
        if (anchorElement) {
          rect = anchorElement.getBoundingClientRect();
        }
      }

      if (rect && isFinite(rect.left) && isFinite(rect.top)) {
        setPalettePosition({
          top: rect.bottom + 6,
          left: rect.left,
        });
      } else {
        setPalettePosition({
          top: 40,
          left: 40,
        });
      }
    });
  }, [editor]);

  const openPalette = useCallback(() => {
    captureSelection();
    setIsOpen(true);
    setQuery("");
    setSelectedIndex(0);
    setPaletteMode("commands");
    setHoneCandidates([]);
    setTargetFacet(null);
    editor.setEditable(false);
    setTimeout(() => inputRef.current?.focus(), 0);
  }, [captureSelection, editor]);

  const closePalette = useCallback(() => {
    setIsOpen(false);
    setQuery("");
    setSelectedIndex(0);
    setPaletteMode("commands");
    setHoneCandidates([]);
    setTargetFacet(null);
    editor.setEditable(true);
    const savedSelection = savedSelectionRef.current;
    if (savedSelection) {
      editor.update(() => {
        const node = $getNodeByKey(savedSelection.key);
        if (!node) {
          return;
        }

        const selection = $createRangeSelection();
        if (savedSelection.type === "text" && $isTextNode(node)) {
          const offset = Math.min(
            savedSelection.offset,
            node.getTextContentSize(),
          );
          selection.setTextNodeRange(node, offset, node, offset);
        } else {
          selection.anchor.set(
            savedSelection.key,
            savedSelection.offset ?? 0,
            "element",
          );
          selection.focus.set(
            savedSelection.key,
            savedSelection.offset ?? 0,
            "element",
          );
        }
        $setSelection(selection);
      });
    }
  }, [editor]);

  const isSelectionAtLineStart = useCallback(() => {
    return editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
        return false;
      }

      const anchorNode = selection.anchor.getNode();
      const offset = selection.anchor.offset;
      const text = anchorNode.getTextContent();
      const beforeText = text.slice(0, offset);

      return beforeText.trim().length === 0;
    });
  }, [editor]);

  const getCurrentFacetSnapshot = useCallback((): FacetSnapshot | null => {
    let snapshot: FacetSnapshot | null = null;

    editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection)) {
        return;
      }

      const facetNode = findNearestFacetTitleNode(selection);
      if (!facetNode) {
        return;
      }

      const facetId = facetNode.getUniqueId();
      const titleText = facetNode
        .getTextContent()
        .replace(/^\$\s*/, "")
        .trim();
      const root = $getRoot();
      const children = root.getChildren();
      const bodyTexts: string[] = [];
      let collecting = false;

      for (const child of children) {
        if (child === facetNode) {
          collecting = true;
          continue;
        }

        if (child instanceof FacetTitleNode) {
          if (collecting) {
            break;
          }
          continue;
        }

        if (collecting) {
          const text = child.getTextContent().trim();
          if (text.length > 0) {
            bodyTexts.push(text);
          }
        }
      }

      snapshot = {
        facetId,
        title: titleText || facetId,
        bodyText: bodyTexts.join("\n"),
      };
    });

    return snapshot;
  }, [editor]);

  const updateFacetStatusIndicator = useCallback(() => {
    editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection)) {
        setActiveFacetStatus(null);
        return;
      }

      const anchorTopLevel = selection.anchor
        .getNode()
        .getTopLevelElementOrThrow();

      if (!(anchorTopLevel instanceof FacetTitleNode)) {
        setActiveFacetStatus(null);
        return;
      }

      const facetId = anchorTopLevel.getUniqueId();
      const isUpdated = Boolean(library.facetsById[facetId]);

      setActiveFacetStatus({
        key: anchorTopLevel.getKey(),
        state: isUpdated ? "updated" : "draft",
      });
    });
  }, [editor, library]);

  useEffect(() => {
    const unregisterSelectionListener = editor.registerCommand(
      SELECTION_CHANGE_COMMAND,
      () => {
        updateFacetStatusIndicator();
        return false;
      },
      COMMAND_PRIORITY_LOW,
    );

    return () => {
      unregisterSelectionListener();
    };
  }, [editor, updateFacetStatusIndicator]);

  useEffect(() => {
    updateFacetStatusIndicator();
  }, [library, updateFacetStatusIndicator]);

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === FACET_LIBRARY_KEY) {
        setLibrary(loadLibrary());
      }
    };

    window.addEventListener("storage", handleStorage);

    return () => {
      window.removeEventListener("storage", handleStorage);
    };
  }, []);

  useEffect(() => {
    const element =
      activeFacetStatus?.key !== undefined && activeFacetStatus !== null
        ? (editor.getElementByKey(activeFacetStatus.key) as HTMLElement | null)
        : null;

    if (activeFacetElementRef.current) {
      activeFacetElementRef.current.removeAttribute("data-facet-status");
      activeFacetElementRef.current.classList.remove("facet-status-indicator");
    }

    if (element && activeFacetStatus) {
      element.setAttribute(
        "data-facet-status",
        activeFacetStatus.state === "updated" ? "Updated" : "Draft",
      );
      element.classList.add("facet-status-indicator");
      activeFacetElementRef.current = element;
    } else {
      activeFacetElementRef.current = null;
    }
  }, [activeFacetStatus, editor]);

  useEffect(() => {
    const unregisterCommand = editor.registerCommand(
      KEY_DOWN_COMMAND,
      (event: KeyboardEvent) => {
        if (event.key === "/" && !isOpen && isSelectionAtLineStart()) {
          event.preventDefault();
          openPalette();
          return true;
        }
        return false;
      },
      COMMAND_PRIORITY_LOW,
    );

    return () => {
      unregisterCommand();
    };
  }, [editor, isOpen, isSelectionAtLineStart, openPalette]);

  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus();
    }
  }, [isOpen]);

  useEffect(() => {
    return () => {
      editor.setEditable(true);
    };
  }, [editor]);

  const createFacetAtCursor = useCallback(() => {
    editor.update(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection)) {
        return;
      }

      const topLevel = selection.anchor.getNode().getTopLevelElementOrThrow();
      const facetId = `${articleId}-facet-${Date.now()}`;
      const facetTitleNode = new FacetTitleNode(facetId);
      facetTitleNode.append($createTextNode("$ "));
      const bodyParagraph = $createParagraphNode();
      const shouldReplace = topLevel.getTextContent().trim().length === 0;

      if (shouldReplace) {
        topLevel.replace(facetTitleNode);
      } else {
        topLevel.insertAfter(facetTitleNode);
      }

      facetTitleNode.insertAfter(bodyParagraph);

      const firstChild = facetTitleNode.getFirstChild();
      if (firstChild && $isTextNode(firstChild)) {
        firstChild.select();
      } else {
        facetTitleNode.select();
      }
    });

    onMessageChange("Created facet", true);
    closePalette();
  }, [articleId, closePalette, editor, onMessageChange]);

  const handleUpdateFacet = useCallback(() => {
    const snapshot = getCurrentFacetSnapshot();

    if (!snapshot) {
      onMessageChange("Place the cursor inside a facet to use /update.", true);
      closePalette();
      return;
    }

    const nextState = upsertFacet(library, snapshot);
    setLibrary(nextState);
    onMessageChange("Updated", true);
    closePalette();
  }, [closePalette, getCurrentFacetSnapshot, library, onMessageChange]);

  const startHoneFlow = useCallback(() => {
    if (Object.keys(library.facetsById).length === 0) {
      onMessageChange("No library facets yet — use /update first.", true);
      closePalette();
      return;
    }

    const snapshot = getCurrentFacetSnapshot();

    if (!snapshot) {
      onMessageChange("Place the cursor inside a facet to use /hone.", true);
      closePalette();
      return;
    }

    let workingLibrary = library;

    if (!workingLibrary.facetsById[snapshot.facetId]) {
      workingLibrary = upsertFacet(workingLibrary, snapshot);
      setLibrary(workingLibrary);
    }

    const candidates = Object.values(workingLibrary.facetsById)
      .filter((facet) => facet.facetId !== snapshot.facetId)
      .map(
        (facet) =>
          ({
            ...facet,
            similarity: getJaccardSimilarity(
              `${snapshot.title} ${snapshot.bodyText}`,
              `${facet.title} ${facet.bodyText}`,
            ),
          }) as HoneCandidate,
      )
      .sort((a, b) => b.similarity - a.similarity);

    if (candidates.length === 0) {
      onMessageChange("Add another library facet before honing.", true);
      closePalette();
      return;
    }

    setTargetFacet(snapshot);
    setHoneCandidates(candidates);
    setPaletteMode("hone");
    setSelectedIndex(0);
  }, [
    closePalette,
    getCurrentFacetSnapshot,
    library,
    onMessageChange,
    setLibrary,
  ]);

  const insertHonedText = useCallback(
    (sourceFacet: FacetLibraryItem) => {
      editor.update(() => {
        const selection = $getSelection();
        if ($isRangeSelection(selection)) {
          const delimiterBlock = `\n\n---\nHoned from: ${sourceFacet.title}\n${sourceFacet.bodyText}\n---\n`;
          selection.insertText(delimiterBlock);
        }
      });
    },
    [editor],
  );

  const completeHone = useCallback(
    (candidate: HoneCandidate) => {
      if (!targetFacet) {
        closePalette();
        return;
      }

      const nextLibrary = addHoneEdge(
        library,
        targetFacet.facetId,
        candidate.facetId,
        Date.now(),
      );

      setLibrary(nextLibrary);
      insertHonedText(candidate);
      onMessageChange(`Honed with ${candidate.title}`, true);
      closePalette();
    },
    [closePalette, insertHonedText, library, onMessageChange, targetFacet],
  );

  const filteredCommands = useMemo(
    () =>
      commandOptions.filter(
        (option) =>
          option.title.toLowerCase().includes(query.toLowerCase()) ||
          option.description.toLowerCase().includes(query.toLowerCase()),
      ),
    [commandOptions, query],
  );

  const filteredHoneCandidates = useMemo(
    () =>
      honeCandidates.filter((candidate) =>
        candidate.title.toLowerCase().includes(query.toLowerCase()),
      ),
    [honeCandidates, query],
  );

  const currentOptions =
    paletteMode === "commands" ? filteredCommands : filteredHoneCandidates;

  useEffect(() => {
    if (selectedIndex > currentOptions.length - 1) {
      setSelectedIndex(currentOptions.length > 0 ? 0 : -1);
    }
  }, [currentOptions.length, selectedIndex]);

  const handlePaletteKeyDown = (
    event: React.KeyboardEvent<HTMLInputElement>,
  ) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((prev) =>
        prev + 1 >= currentOptions.length ? 0 : prev + 1,
      );
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((prev) =>
        prev - 1 < 0 ? currentOptions.length - 1 : prev - 1,
      );
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const option = currentOptions[selectedIndex];
      if (!option) {
        return;
      }

      if (paletteMode === "commands") {
        switch ((option as CommandOption).id) {
          case "facet":
            createFacetAtCursor();
            break;
          case "update":
            handleUpdateFacet();
            break;
          case "hone":
            startHoneFlow();
            break;
          default:
            break;
        }
      } else {
        completeHone(option as HoneCandidate);
      }
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      closePalette();
    }
  };

  return isOpen ? (
    <>
      <div className="editor-overlay" onClick={closePalette}></div>
      <div
        className="command-palette"
        style={{ top: palettePosition.top, left: palettePosition.left }}
      >
        <div className="command-palette-input">
          <span className="command-prefix">/</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handlePaletteKeyDown}
            placeholder={
              paletteMode === "commands"
                ? "facet, update, hone"
                : "Search library facets"
            }
          />
        </div>
        <ul className="command-palette-list">
          {currentOptions.map((option, index) => (
            <li
              key={
                paletteMode === "commands"
                  ? (option as CommandOption).id
                  : (option as HoneCandidate).facetId
              }
              className={`command-palette-item ${
                index === selectedIndex ? "selected" : ""
              }`}
              onMouseEnter={() => setSelectedIndex(index)}
              onMouseDown={(e) => {
                e.preventDefault();
                if (paletteMode === "commands") {
                  switch ((option as CommandOption).id) {
                    case "facet":
                      createFacetAtCursor();
                      break;
                    case "update":
                      handleUpdateFacet();
                      break;
                    case "hone":
                      startHoneFlow();
                      break;
                    default:
                      break;
                  }
                } else {
                  completeHone(option as HoneCandidate);
                }
              }}
            >
              <div className="command-title">
                {paletteMode === "commands"
                  ? (option as CommandOption).title
                  : (option as HoneCandidate).title}
              </div>
              <div className="command-subtitle">
                {paletteMode === "commands" ? (
                  (option as CommandOption).description
                ) : (
                  <>
                    {Math.round((option as HoneCandidate).similarity * 100)}%
                    similarity
                  </>
                )}
              </div>
            </li>
          ))}
          {currentOptions.length === 0 && (
            <li className="command-palette-item empty">No matches</li>
          )}
        </ul>
      </div>
    </>
  ) : null;
};

export default SlashCommandPlugin;
