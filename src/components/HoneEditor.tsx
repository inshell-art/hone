import { $getRoot, $getSelection, EditorState } from "lexical";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { PlainTextPlugin } from "@lexical/react/LexicalPlainTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { OnChangePlugin } from "@lexical/react/LexicalOnChangePlugin";
import { AutoFocusPlugin } from "@lexical/react/LexicalAutoFocusPlugin";
import LexicalErrorBoundary from "@lexical/react/LexicalErrorBoundary";

import HoneTheme from "../themes/HoneTheme";
import StyleArticleTitlePlugin from "../plugins/StyleArticleTitlePlugin";
import { HeadingNode } from "@lexical/rich-text";
import { TextNode } from "lexical";

// When the editor changes, you can get notified via the
// LexicalOnChangePlugin!
const onChange =
  () =>
  (editorState: EditorState): void => {
    editorState.read(() => {
      // Read the contents of the EditorState here.
      const root = $getRoot();
      const selection = $getSelection();

      console.log(root, selection);
    });
  };

// Lexical React plugins are React components, which makes them
// highly composable. Furthermore, you can lazy load plugins if
// desired, so you don't pay the cost for plugins until you
// actually use them.

// Catch any errors that occur during Lexical updates and log them
// or throw them as needed. If you don't throw them, Lexical will
// try to recover gracefully without losing user data.

const HoneEditor = () => {
  const initialConfig = {
    namespace: "HoneEditor",
    theme: HoneTheme,
    onError(error: Error) {
      throw error;
    },
    nodes: [TextNode, HeadingNode],
  };

  const Placeholder = () => {
    return <div className="editor-placeholder">Type your article here.</div>;
  };

  return (
    <LexicalComposer initialConfig={initialConfig}>
      <div className="editor-container">
        <PlainTextPlugin
          contentEditable={<ContentEditable className="editor-input" />}
          placeholder={<Placeholder />}
          ErrorBoundary={LexicalErrorBoundary}
        />
        <StyleArticleTitlePlugin />
        <AutoFocusPlugin />
        <HistoryPlugin />
        <OnChangePlugin onChange={onChange} />
      </div>
    </LexicalComposer>
  );
};

export default HoneEditor;
