import {
  SerializedElementNode,
  SerializedLexicalNode,
  SerializedTextNode,
} from "lexical";

export const INSERT_SYMBOL = ">>>>>>>";

export const collectTextFromDescendants = (
  node: SerializedElementNode | SerializedLexicalNode | SerializedTextNode,
  collectedTexts: string[],
): string[] => {
  if (node.type === "text") {
    collectedTexts.push((node as SerializedTextNode).text);
  } else if ("children" in node && Array.isArray(node.children)) {
    node.children.forEach((child) => {
      collectTextFromDescendants(child, collectedTexts);
    });
  }

  return collectedTexts;
};
