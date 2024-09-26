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
import HonePanelPlugin from "../plugins/HonePanelPlugin";
import SetFacetTitlePlugin from "../plugins/SetFacetTitlePlugin";
import { FacetTitleNode } from "../models/FacetTitleNode";
import { ArticleTitleNode } from "../models/ArticleTitleNode";
import { HeadingNode } from "@lexical/rich-text";
import DisableLineBreakInFacetTitlePlugin from "../plugins/DisableLineBreakInFacetTitlePlugin";
import MessageDisplay from "./MessageDisplay";
import { useCallback, useEffect, useState } from "react";
import { useLocation } from "react-router-dom";
import DisableTextFormattingPlugin from "../plugins/DisableTextFormattingPlugin";
import StripFormattingPastePlugin from "../plugins/StripFormattingPastePlugin";

const Editor: React.FC<EditorProps> = ({ articleId }) => {
  const [message, setMessage] = useState<string | null>(null);
  const [isTemporary, setIsTemporary] = useState<boolean>(false);
  const location = useLocation();
  const queryParam = new URLSearchParams(location.search);
  const facetId = queryParam.get("facetId");

  // Scroll to the facet title when the facetId query param is present
  useEffect(() => {
    if (facetId) {
      scrollToFacet(facetId);
    }
  }, [facetId]);

  const scrollToFacet = (facetId: string) => {
    const facetTitleNode = document.querySelector(
      `[data-facet-title-id="${facetId}"]`,
    );

    if (facetTitleNode) {
      facetTitleNode.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  };

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

  const initialConfig = {
    namespace: "HoneEditor",
    theme: HoneEditorTheme,
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
      <div className="editor-placeholder">Type your article title here...</div>
    );
  };

  return (
    <LexicalComposer initialConfig={initialConfig}>
      <MessageDisplay
        message={message}
        isTemporary={isTemporary}
        clearMessage={clearMessage}
      />
      <div className="editor-container">
        <RichTextPlugin
          contentEditable={<ContentEditable className="editor-input" />}
          placeholder={<Placeholder />}
          ErrorBoundary={CustomErrorBoundary}
        />
        <SetArticleTitlePlugin />
        <SetFacetTitlePlugin articleId={articleId} />
        <HistoryPlugin />
        <LoadArticlePlugin
          articleId={articleId}
          onMessageChange={handleMessageChange}
        />
        <AutoSavePlugin
          articleId={articleId}
          onMessageChange={handleMessageChange}
        />

        <HonePanelPlugin />
        <DisableLineBreakInFacetTitlePlugin />
        <StripFormattingPastePlugin />
        <DisableTextFormattingPlugin />
      </div>
    </LexicalComposer>
  );
};

export default Editor;
