import { LexicalComposer } from "@lexical/react/LexicalComposer";

import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import LexicalErrorBoundary from "@lexical/react/LexicalErrorBoundary";

import HoneTheme from "../themes/HoneTheme";
import { HeadingNode } from "@lexical/rich-text";
import { ParagraphNode, TextNode } from "lexical";
import TreeViewPlugin from "../plugins/TreeViewPlugin";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import StyleArticleTitlePlugin from "../plugins/StyleArticleTitlePlugin";
import StyleFacetTitlePlugin from "../plugins/StyleFacetTitlePlugin";

const HoneEditor = () => {
  const initialConfig = {
    namespace: "HoneEditor",
    theme: HoneTheme,
    onError(error: Error) {
      throw error;
    },
    nodes: [TextNode, HeadingNode, ParagraphNode],
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
          ErrorBoundary={LexicalErrorBoundary}
        />

        <StyleArticleTitlePlugin />
        <StyleFacetTitlePlugin />
        <AutoFocusPlugin />
        <HistoryPlugin />
        <TreeViewPlugin />
      </div>
    </LexicalComposer>
  );
};

export default HoneEditor;
