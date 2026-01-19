import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import CustomErrorBoundary from "../components/CustomErrorBoundary";
import HoneEditorTheme from "../themes/HoneEditorTheme";
import { ParagraphNode, TextNode } from "lexical";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import SetArticleTitlePlugin from "../plugins/SetArticleTitlePlugin";
import { EditorProps } from "../types/types";
import AutoSavePlugin from "../plugins/AutoSavePlugin";
import LoadArticlePlugin from "../plugins/LoadArticlePlugin";
import SetFacetTitlePlugin from "../plugins/SetFacetTitlePlugin";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { ArticleTitleNode } from "../models/ArticleTitleNode";
import { HeadingNode } from "@lexical/rich-text";
import MessageDisplay from "./MessageDisplay";
import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import DisableTextFormattingPlugin from "../plugins/DisableTextFormattingPlugin";
import StripFormattingPastePlugin from "../plugins/StripFormattingPastePlugin";
import KeepTitlesInOneLinePlugin from "../plugins/KeepTitlesInOneLinePlugin";
import SlashCommandPlugin from "../plugins/SlashCommandPlugin";
import {
  ARTICLE_EDITIONS_UPDATED_EVENT,
  getEditionsForArticle,
  getArticleRecord,
  loadArticleEditions,
} from "../utils/articleEditions";
import { HONE_ARTICLE_EDITIONS_KEY } from "../constants/storage";
import { ArticleEdition } from "../types/types";
import { formatTimestamp } from "../utils/utils";

const Editor: React.FC<EditorProps> = ({ articleId, isEditable }) => {
  const [message, setMessage] = useState<string | null>(null);
  const [isTemporary, setIsTemporary] = useState<boolean>(false);
  const [editions, setEditions] = useState<ArticleEdition[]>([]);
  const [latestVersion, setLatestVersion] = useState<number | null>(null);
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const historyRef = useRef<HTMLDivElement | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const queryParam = new URLSearchParams(location.search);
  const facetId = queryParam.get("facetId");

  // Scroll to the facet title when the facetId query param is present
  useEffect(() => {
    if (!facetId) {
      return;
    }

    const maxAttempts = 20;
    const retryDelayMs = 50;
    let attempts = 0;
    let timeoutId: number | null = null;
    let cancelled = false;

    const tryScroll = () => {
      if (cancelled) {
        return;
      }

      const facetTitleNode = document.querySelector(
        `[data-facet-title-id="${facetId}"]`,
      );

      if (facetTitleNode) {
        facetTitleNode.scrollIntoView({ behavior: "smooth", block: "start" });
        return;
      }

      if (attempts >= maxAttempts) {
        return;
      }

      attempts += 1;
      timeoutId = window.setTimeout(tryScroll, retryDelayMs);
    };

    tryScroll();

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [facetId]);

  // Handle messages from child components
  const handleMessageChange = useCallback(
    (message: string | null, isTemporary?: boolean) => {
      setMessage(message);
      setIsTemporary(isTemporary || false);
    },
    [],
  );

  const clearMessage = () => {
    setMessage(null);
  };

  const refreshPublishInfo = useCallback(() => {
    const publishState = loadArticleEditions();
    const record = getArticleRecord(publishState, articleId);
    setLatestVersion(record?.latestVersion ?? null);
    setEditions(getEditionsForArticle(publishState, articleId));
  }, [articleId]);

  useEffect(() => {
    refreshPublishInfo();
    const handleStorage = (event: StorageEvent) => {
      if (event.key === HONE_ARTICLE_EDITIONS_KEY) {
        refreshPublishInfo();
      }
    };

    window.addEventListener("storage", handleStorage);
    window.addEventListener(ARTICLE_EDITIONS_UPDATED_EVENT, refreshPublishInfo);

    return () => {
      window.removeEventListener("storage", handleStorage);
      window.removeEventListener(
        ARTICLE_EDITIONS_UPDATED_EVENT,
        refreshPublishInfo,
      );
    };
  }, [refreshPublishInfo]);

  useEffect(() => {
    if (!isHistoryOpen) {
      return;
    }

    const handleClickOutside = (event: MouseEvent) => {
      if (
        historyRef.current &&
        !historyRef.current.contains(event.target as Node)
      ) {
        setIsHistoryOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isHistoryOpen]);

  const copyEditionLink = useCallback(
    async (version: number) => {
      const url = `${window.location.origin}/a/${articleId}/v/${version}`;
      try {
        await navigator.clipboard.writeText(url);
        handleMessageChange(`Copied v${version} link`, true);
      } catch (error) {
        window.prompt("Copy edition link:", url);
      }
    },
    [articleId, handleMessageChange],
  );

  const initialConfig = {
    namespace: "HoneEditor",
    theme: HoneEditorTheme,
    editable: isEditable,
    onError(error: Error) {
      throw error;
    },
    nodes: [
      TextNode,
      ParagraphNode,
      ArticleTitleNode,
      FacetTitleNode,
      HeadingNode,
    ],
  };

  const Placeholder = () => {
    return (
      <div className="editor-placeholder">
        {isEditable
          ? "Type your article title here..."
          : "No article found at the link"}
      </div>
    );
  };

  return (
    <LexicalComposer initialConfig={initialConfig}>
      {isEditable && (
        <MessageDisplay
          message={message}
          isTemporary={isTemporary}
          clearMessage={clearMessage}
        />
      )}
      <div className="editor-container">
        {isEditable && (
          <div className="editor-status">
            <span className="editor-status-chip">Draft</span>
            {latestVersion ? (
              <div className="editor-publish" ref={historyRef}>
                <button
                  type="button"
                  className="editor-status-chip editor-status-button"
                  onClick={() => setIsHistoryOpen((open) => !open)}
                >
                  Published v{latestVersion}
                </button>
                <button
                  type="button"
                  className="editor-status-link"
                  onClick={() => copyEditionLink(latestVersion)}
                >
                  Copy v{latestVersion} link
                </button>
                {isHistoryOpen && (
                  <div className="edition-history">
                    {editions.map((edition) => (
                      <div
                        key={edition.editionId}
                        className="edition-history-item"
                      >
                        <button
                          type="button"
                          className="edition-history-link"
                          onClick={() => {
                            setIsHistoryOpen(false);
                            navigate(`/a/${articleId}/v/${edition.version}`);
                          }}
                        >
                          v{edition.version} —{" "}
                          {formatTimestamp(edition.createdAt)}
                        </button>
                        <button
                          type="button"
                          className="edition-history-copy"
                          onClick={() => copyEditionLink(edition.version)}
                        >
                          Copy link
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ) : null}
          </div>
        )}
        <RichTextPlugin
          contentEditable={<ContentEditable className="editor-input" />}
          placeholder={<Placeholder />}
          ErrorBoundary={CustomErrorBoundary}
        />
        <SetArticleTitlePlugin />
        {isEditable && <SetFacetTitlePlugin articleId={articleId} />}
        <LoadArticlePlugin
          articleId={articleId}
          onMessageChange={handleMessageChange}
        />
        {isEditable && (
          <>
            <HistoryPlugin />
            <AutoSavePlugin
              articleId={articleId}
              onMessageChange={handleMessageChange}
            />
            <SlashCommandPlugin
              articleId={articleId}
              onMessageChange={handleMessageChange}
            />
            <KeepTitlesInOneLinePlugin />
            <StripFormattingPastePlugin />
            <DisableTextFormattingPlugin />
          </>
        )}
      </div>
    </LexicalComposer>
  );
};

export default Editor;
