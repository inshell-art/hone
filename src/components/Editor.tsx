import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import CustomErrorBoundary from "../components/CustomErrorBoundary";
import HoneEditorTheme from "../themes/HoneEditorTheme";
import { HeadingNode } from "@lexical/rich-text";
import { ParagraphNode, TextNode } from "lexical";
import TreeViewPlugin from "../plugins/TreeViewPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import StyleArticleTitlePlugin from "../plugins/StyleArticleTitlePlugin";
import { EditorProps } from "../types/types";
import AutoSavePlugin from "../plugins/AutoSavePlugin";
import LoadArticlePlugin from "../plugins/LoadArticlePlugin";
import HonePanelPlugin from "../plugins/HonePanelPlugin";
import SetFacetTitlePlugin from "../plugins/SetFacetTitlePlugin";
import { FacetTitleNode } from "../models/FacetTitleNode";

const Editor: React.FC<EditorProps> = ({ articleId }) => {
  const initialConfig = {
    namespace: "HoneEditor",
    theme: HoneEditorTheme,
    onError(error: Error) {
      throw error;
    },
    nodes: [TextNode, HeadingNode, ParagraphNode, FacetTitleNode],
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
        <StyleArticleTitlePlugin />
        <SetFacetTitlePlugin articleId={articleId} />
        <AutoFocusPlugin />
        <HistoryPlugin />
        <LoadArticlePlugin articleId={articleId} />
        <AutoSavePlugin articleId={articleId} />
        <HonePanelPlugin />
        <TreeViewPlugin />
      </div>
    </LexicalComposer>
  );
};

export default Editor;
