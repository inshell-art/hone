import { HeadingNode, SerializedHeadingNode } from "@lexical/rich-text";
import { EditorConfig } from "lexical";

interface SerializedFacetTitleNode extends SerializedHeadingNode {
  uniqueId: string;
  active: boolean;
  honedBy: Array<string>;
  honedAmount: number;
}

export class FacetTitleNode extends HeadingNode {
  __uniqueId: string;
  __active: boolean;
  __honedBy: Array<string>;
  __honedAmount: number;

  constructor(
    uniqueId: string,
    active: boolean = true,
    honedBy: Array<string> = [],
    honedAmount: number = 0,
    key?: string,
  ) {
    super(active ? "h2" : "h3", key);
    this.__uniqueId = uniqueId;
    this.__active = active;
    this.__honedBy = honedBy;
    this.__honedAmount = honedAmount;
    // A active facet title looks as h2,
    // a non-active facet title looks as h3 which looks as paragraph
    // but they are all facet titles
    // to remain the data of the facet title until it's omitted intentionally
  }

  static getType() {
    return "facet-title";
  }

  static clone(node: FacetTitleNode) {
    return new FacetTitleNode(
      node.__uniqueId,
      node.__active,
      node.__honedBy,
      node.__honedAmount,
      node.__key,
    );
  }

  createDOM(config: EditorConfig) {
    const dom = super.createDOM(config);
    dom.setAttribute("data-facet-title-id", this.__uniqueId);
    dom.setAttribute("data-active", this.__active.toString());
    dom.setAttribute("data-honed-amount", this.__honedAmount.toString());
    return dom;
  }

  exportJSON(): SerializedFacetTitleNode {
    return {
      ...super.exportJSON(),
      type: "facet-title",
      uniqueId: this.__uniqueId,
      active: this.__active,
      honedBy: this.__honedBy,
      honedAmount: this.__honedAmount,
    };
  }

  static importJSON(serializedNode: SerializedFacetTitleNode): FacetTitleNode {
    const { uniqueId, active, honedBy, honedAmount } = serializedNode;
    const newNode = new FacetTitleNode(uniqueId, active, honedBy, honedAmount);

    Object.defineProperty(newNode, "__active", {
      writable: true,
      value: active,
    });

    return newNode;
  }

  // Methods to manipulate honedBy and calculate honedAmount
  addHone(facetTitleNodeId: string) {
    this.__honedBy.push(facetTitleNodeId);
    this.__honedAmount += 1; // Increase the honedAmount by 1
  }

  getHonedAmount(): number {
    return this.__honedAmount;
  }

  isActive(): boolean {
    return this.__active;
  }
}