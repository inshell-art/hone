import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import CustomErrorBoundary from "../components/CustomErrorBoundary";
import HoneEditorTheme from "../themes/HoneEditorTheme";
import { ParagraphNode, TextNode } from "lexical";
import TreeViewPlugin from "../plugins/TreeViewPlugin";
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
import HandlePastePlugin from "../plugins/HandlePastePlugin";
import DisableTextFormatPlugin from "../plugins/DisableTextFormatPlugin";

const Editor: React.FC<EditorProps> = ({ articleId }) => {
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
      <div className="editor-container">
        <RichTextPlugin
          contentEditable={<ContentEditable className="editor-input" />}
          placeholder={<Placeholder />}
          ErrorBoundary={CustomErrorBoundary}
        />
        <SetArticleTitlePlugin />
        <SetFacetTitlePlugin articleId={articleId} />
        <HistoryPlugin />
        <LoadArticlePlugin articleId={articleId} />
        <AutoSavePlugin articleId={articleId} />
        <HonePanelPlugin />
        <TreeViewPlugin />
        <DisableLineBreakInFacetTitlePlugin />
        <HandlePastePlugin />
        <DisableTextFormatPlugin />
      </div>
    </LexicalComposer>
  );
};

export default Editor;
