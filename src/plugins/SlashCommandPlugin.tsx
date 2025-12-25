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
  $isParagraphNode,
  COMMAND_PRIORITY_LOW,
  KEY_DOWN_COMMAND,
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
  const paletteRef = useRef<HTMLDivElement | null>(null);
  const paletteListRef = useRef<HTMLUListElement | null>(null);
  const paletteItemRefs = useRef<Array<HTMLLIElement | null>>([]);
  const paletteAnchorRectRef = useRef<DOMRect | null>(null);
  const bodyStyleRef = useRef<{
    overflow: string;
    paddingRight: string;
  } | null>(null);
  const savedSelectionRef = useRef<{
    key: string;
    offset: number;
    type: "text" | "element";
  } | null>(null);
  const facetStateKeysRef = useRef<Set<string>>(new Set());

  const commandOptions: CommandOption[] = useMemo(
    () => [
      {
        id: "facet",
        title: "/create",
        description: "a new facets.",
      },
      {
        id: "update",
        title: "/update",
        description: "this facet.",
      },
      {
        id: "hone",
        title: "/hone",
        description: "this facet with another one.",
      },
    ],
    [],
  );

  const captureSelection = useCallback(
    (options?: { applyPosition?: boolean }) => {
      const applyPosition = options?.applyPosition !== false;
      const rect = editor.getEditorState().read(() => {
        let localRect: DOMRect | null = null;
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) {
          savedSelectionRef.current = null;
          return null;
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
        if (domSelection && domSelection.rangeCount > 0) {
          const range = domSelection.getRangeAt(0).cloneRange();
          range.collapse(true);
          const clientRect = Array.from(range.getClientRects()).find(
            (r) => r.width || r.height,
          );
          localRect = clientRect ?? range.getBoundingClientRect();
        }

        if (
          (!localRect || (localRect.width === 0 && localRect.height === 0)) &&
          anchorNode
        ) {
          const anchorElement = editor.getElementByKey(anchorNode.getKey());
          if (anchorElement) {
            localRect = anchorElement.getBoundingClientRect();
          }
        }

        return localRect;
      });

      if (!applyPosition) {
        return rect;
      }

      if (rect && isFinite(rect.left) && isFinite(rect.top)) {
        paletteAnchorRectRef.current = rect;
        setPalettePosition({
          top: rect.bottom + 6,
          left: rect.left,
        });
      } else {
        paletteAnchorRectRef.current = null;
        setPalettePosition({
          top: 40,
          left: 40,
        });
      }

      return rect;
    },
    [editor],
  );

  const ensureCaretInView = useCallback((rect: DOMRect | null) => {
    if (!rect) {
      return false;
    }

    const margin = 16;
    if (rect.top < margin) {
      window.scrollBy({ top: rect.top - margin, behavior: "auto" });
      return true;
    }

    if (rect.bottom > window.innerHeight - margin) {
      window.scrollBy({
        top: rect.bottom - window.innerHeight + margin,
        behavior: "auto",
      });
      return true;
    }

    return false;
  }, []);

  const openPalette = useCallback(() => {
    const rect = captureSelection({ applyPosition: false });
    const didScroll = ensureCaretInView(rect);

    const finalizeOpen = () => {
      captureSelection();
      setIsOpen(true);
      setQuery("");
      setSelectedIndex(0);
      setPaletteMode("commands");
      setHoneCandidates([]);
      setTargetFacet(null);
      setTimeout(() => inputRef.current?.focus(), 0);
    };

    if (didScroll) {
      requestAnimationFrame(finalizeOpen);
    } else {
      finalizeOpen();
    }
  }, [captureSelection, ensureCaretInView]);

  const closePalette = useCallback(
    (restoreSelection: boolean = true) => {
      setIsOpen(false);
      setQuery("");
      setSelectedIndex(0);
      setPaletteMode("commands");
      setHoneCandidates([]);
      setTargetFacet(null);
      if (!restoreSelection) {
        savedSelectionRef.current = null;
        return;
      }

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
    },
    [editor],
  );

  const isSelectionAtLineStart = useCallback(() => {
    return editor.getEditorState().read(() => {
      const selection = $getSelection();
      if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
        return false;
      }

      const domSelection = window.getSelection();
      if (!domSelection || domSelection.rangeCount === 0) {
        return false;
      }

      const range = domSelection.getRangeAt(0).cloneRange();
      if (!range.collapsed) {
        return false;
      }

      const anchorNode = selection.anchor.getNode();
      const topLevel = anchorNode.getTopLevelElementOrThrow();
      const topLevelElement = editor.getElementByKey(
        topLevel.getKey(),
      ) as HTMLElement | null;

      if (!topLevelElement) {
        return false;
      }

      const textRange = range.cloneRange();
      try {
        textRange.setStart(topLevelElement, 0);
      } catch (error) {
        return false;
      }

      if (textRange.toString().trim().length === 0) {
        return true;
      }

      const caretRects = Array.from(range.getClientRects());
      const caretRect =
        caretRects.find((rect) => rect.width || rect.height) ??
        range.getBoundingClientRect();
      if (!caretRect || (caretRect.width === 0 && caretRect.height === 0)) {
        return false;
      }

      if (
        range.startContainer.nodeType !== Node.TEXT_NODE ||
        range.startOffset === 0
      ) {
        return false;
      }

      const previousRange = document.createRange();
      previousRange.setStart(range.startContainer, range.startOffset - 1);
      previousRange.setEnd(range.startContainer, range.startOffset);

      const previousRects = Array.from(previousRange.getClientRects());
      const previousRect =
        previousRects.find((rect) => rect.width || rect.height) ??
        previousRange.getBoundingClientRect();
      if (
        !previousRect ||
        (previousRect.width === 0 && previousRect.height === 0)
      ) {
        return false;
      }

      const elementRect = topLevelElement.getBoundingClientRect();
      const paddingLeft = Number.parseFloat(
        window.getComputedStyle(topLevelElement).paddingLeft || "0",
      );
      const lineStartLeft = elementRect.left + paddingLeft;
      const lineStartThreshold = 6;
      const isAtLineStart =
        caretRect.left <= lineStartLeft + lineStartThreshold;
      const isNewVisualLine = caretRect.top - previousRect.top > 1;

      return isAtLineStart && isNewVisualLine;
    });
  }, [editor]);

  const buildFacetSnapshot = useCallback((facetNode: FacetTitleNode) => {
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

    return {
      facetId,
      title: titleText || facetId,
      bodyText: bodyTexts.join("\n"),
    };
  }, []);

  const getFacetStatus = useCallback(
    (snapshot: FacetSnapshot): "updated" | "draft" => {
      const libraryFacet = library.facetsById[snapshot.facetId];
      if (!libraryFacet) {
        return "draft";
      }

      const isSameTitle = libraryFacet.title.trim() === snapshot.title.trim();
      const isSameBody =
        libraryFacet.bodyText.trim() === snapshot.bodyText.trim();

      return isSameTitle && isSameBody ? "updated" : "draft";
    },
    [library],
  );

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

      snapshot = buildFacetSnapshot(facetNode);
    });

    return snapshot;
  }, [buildFacetSnapshot, editor]);

  const syncFacetTitleStates = useCallback(() => {
    const facetStates = new Map<string, "updated" | "dirty">();

    editor.getEditorState().read(() => {
      const root = $getRoot();
      const children = root.getChildren();
      let currentFacetNode: FacetTitleNode | null = null;
      let bodyTexts: string[] = [];

      const flushFacet = () => {
        if (!currentFacetNode) {
          return;
        }

        const facetId = currentFacetNode.getUniqueId();
        const titleText = currentFacetNode
          .getTextContent()
          .replace(/^\$\s*/, "")
          .trim();
        const snapshot: FacetSnapshot = {
          facetId,
          title: titleText || facetId,
          bodyText: bodyTexts.join("\n"),
        };
        const status = getFacetStatus(snapshot);
        facetStates.set(
          currentFacetNode.getKey(),
          status === "updated" ? "updated" : "dirty",
        );
      };

      for (const child of children) {
        if (child instanceof FacetTitleNode) {
          flushFacet();
          currentFacetNode = child;
          bodyTexts = [];
          continue;
        }

        if (currentFacetNode) {
          const text = child.getTextContent().trim();
          if (text.length > 0) {
            bodyTexts.push(text);
          }
        }
      }

      flushFacet();
    });

    const nextKeys = new Set<string>();
    facetStates.forEach((state, key) => {
      nextKeys.add(key);
      const element = editor.getElementByKey(key) as HTMLElement | null;
      if (element) {
        element.setAttribute("data-state", state);
      }
    });

    facetStateKeysRef.current.forEach((key) => {
      if (!nextKeys.has(key)) {
        const element = editor.getElementByKey(key) as HTMLElement | null;
        element?.removeAttribute("data-state");
      }
    });

    facetStateKeysRef.current = nextKeys;
  }, [editor, getFacetStatus]);

  useEffect(() => {
    syncFacetTitleStates();
  }, [library, syncFacetTitleStates]);

  useEffect(() => {
    const unregisterUpdateListener = editor.registerUpdateListener(() => {
      syncFacetTitleStates();
    });

    return () => unregisterUpdateListener();
  }, [editor, syncFacetTitleStates]);

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
    if (!isOpen) {
      if (bodyStyleRef.current) {
        const body = document.body;
        body.style.overflow = bodyStyleRef.current.overflow;
        body.style.paddingRight = bodyStyleRef.current.paddingRight;
        bodyStyleRef.current = null;
      }
      return;
    }

    const body = document.body;
    if (!bodyStyleRef.current) {
      bodyStyleRef.current = {
        overflow: body.style.overflow,
        paddingRight: body.style.paddingRight,
      };
    }

    const scrollbarWidth =
      window.innerWidth - document.documentElement.clientWidth;
    body.style.overflow = "hidden";
    body.style.paddingRight = scrollbarWidth > 0 ? `${scrollbarWidth}px` : "";

    return () => {
      if (bodyStyleRef.current) {
        body.style.overflow = bodyStyleRef.current.overflow;
        body.style.paddingRight = bodyStyleRef.current.paddingRight;
        bodyStyleRef.current = null;
      }
    };
  }, [isOpen]);

  const positionPalette = useCallback(() => {
    const anchorRect = paletteAnchorRectRef.current;
    const paletteElement = paletteRef.current;
    if (!anchorRect || !paletteElement) {
      return;
    }

    const paletteRect = paletteElement.getBoundingClientRect();
    const margin = 8;
    const spacing = 6;
    let top = anchorRect.bottom + spacing;
    let left = anchorRect.left;

    const maxBottom = window.innerHeight - margin;
    if (top + paletteRect.height > maxBottom) {
      const aboveTop = anchorRect.top - spacing - paletteRect.height;
      if (aboveTop >= margin) {
        top = aboveTop;
      } else {
        top = Math.max(margin, maxBottom - paletteRect.height);
      }
    }

    const maxRight = window.innerWidth - margin;
    if (left + paletteRect.width > maxRight) {
      left = Math.max(margin, maxRight - paletteRect.width);
    }
    if (left < margin) {
      left = margin;
    }

    if (
      Math.abs(top - palettePosition.top) > 1 ||
      Math.abs(left - palettePosition.left) > 1
    ) {
      setPalettePosition({ top, left });
    }
  }, [palettePosition.left, palettePosition.top]);

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
        facetTitleNode.insertAfter(bodyParagraph);
      } else if ($isParagraphNode(topLevel)) {
        selection.insertNodes([facetTitleNode, bodyParagraph]);
      } else {
        topLevel.insertAfter(facetTitleNode);
        facetTitleNode.insertAfter(bodyParagraph);
      }

      const firstChild = facetTitleNode.getFirstChild();
      if (firstChild && $isTextNode(firstChild)) {
        const selection = $createRangeSelection();
        const endOffset = firstChild.getTextContentSize();
        selection.setTextNodeRange(
          firstChild,
          endOffset,
          firstChild,
          endOffset,
        );
        $setSelection(selection);
      } else {
        facetTitleNode.select();
      }
    });

    onMessageChange("Created facet", true);
    closePalette(false);
    setTimeout(() => editor.focus(), 0);
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
    let workingLibrary = loadLibrary();

    if (workingLibrary.updatedAt !== library.updatedAt) {
      setLibrary(workingLibrary);
    }

    if (Object.keys(workingLibrary.facetsById).length === 0) {
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
    setQuery("");
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
      closePalette(false);
      setTimeout(() => editor.focus(), 0);
    },
    [
      closePalette,
      editor,
      insertHonedText,
      library,
      onMessageChange,
      targetFacet,
    ],
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

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handlePosition = () => {
      positionPalette();
    };

    const raf = window.requestAnimationFrame(handlePosition);
    window.addEventListener("resize", handlePosition);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("resize", handlePosition);
    };
  }, [currentOptions.length, isOpen, paletteMode, positionPalette]);

  useEffect(() => {
    if (!isOpen || selectedIndex < 0) {
      return;
    }

    const listElement = paletteListRef.current;
    const selectedElement = paletteItemRefs.current[selectedIndex];
    if (!listElement || !selectedElement) {
      return;
    }

    const listRect = listElement.getBoundingClientRect();
    const itemRect = selectedElement.getBoundingClientRect();
    const itemTop = itemRect.top - listRect.top + listElement.scrollTop;
    const itemBottom = itemTop + selectedElement.offsetHeight;
    const viewTop = listElement.scrollTop;
    const viewBottom = viewTop + listElement.clientHeight;

    if (itemTop < viewTop) {
      listElement.scrollTop = itemTop;
      return;
    }

    if (itemBottom > viewBottom) {
      listElement.scrollTop = itemBottom - listElement.clientHeight;
    }
  }, [currentOptions.length, isOpen, paletteMode, selectedIndex]);

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
      <div className="editor-overlay" onClick={() => closePalette()}></div>
      <div
        ref={paletteRef}
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
            placeholder="Type a command..."
          />
        </div>
        <ul ref={paletteListRef} className="command-palette-list">
          {currentOptions.map((option, index) => (
            <li
              key={
                paletteMode === "commands"
                  ? (option as CommandOption).id
                  : (option as HoneCandidate).facetId
              }
              ref={(element) => {
                paletteItemRefs.current[index] = element;
              }}
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
              <div className="command-line">
                <span className="command-title">
                  {paletteMode === "commands"
                    ? (option as CommandOption).title
                    : (option as HoneCandidate).title}
                </span>
                <span className="command-subtitle">
                  {paletteMode === "commands"
                    ? (option as CommandOption).description
                    : `${Math.round((option as HoneCandidate).similarity * 100)}%`}
                </span>
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
