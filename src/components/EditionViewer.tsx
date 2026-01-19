import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ParagraphNode, TextNode } from "lexical";
import { HeadingNode } from "@lexical/rich-text";
import CustomErrorBoundary from "../components/CustomErrorBoundary";
import HoneEditorTheme from "../themes/HoneEditorTheme";
import { ArticleTitleNode } from "../models/ArticleTitleNode";
import { FacetTitleNode } from "../models/FacetTitleNode";
import LoadEditionPlugin from "../plugins/LoadEditionPlugin";

type EditionViewerProps = {
  articleId: string;
  version: number;
};

const EditionViewer: React.FC<EditionViewerProps> = ({
  articleId,
  version,
}) => {
  const initialConfig = {
    namespace: "HoneEdition",
    theme: HoneEditorTheme,
    editable: false,
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

  return (
    <LexicalComposer initialConfig={initialConfig}>
      <div className="editor-container">
        <RichTextPlugin
          contentEditable={<ContentEditable className="editor-input" />}
          placeholder={
            <div className="editor-placeholder">
              No edition found at the link
            </div>
          }
          ErrorBoundary={CustomErrorBoundary}
        />
        <LoadEditionPlugin articleId={articleId} version={version} />
      </div>
    </LexicalComposer>
  );
};

export default EditionViewer;
