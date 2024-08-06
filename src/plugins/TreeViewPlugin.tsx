import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { TreeView } from "@lexical/react/LexicalTreeView";
import { FC } from "react";
import "./TreeViewStyles.css";

const TreeViewPlugin: FC = () => {
  const [editor] = useLexicalComposerContext();

  return (
    <TreeView
      viewClassName="tree-view-output"
      treeTypeButtonClassName="tree-type-button"
      timeTravelPanelClassName="debug-timetravel-panel"
      timeTravelButtonClassName="debug-timetravel-button"
      timeTravelPanelSliderClassName="debug-timetravel-panel-slider"
      timeTravelPanelButtonClassName="debug-timetravel-panel-button"
      editor={editor}
    />
  );
};

export default TreeViewPlugin;
