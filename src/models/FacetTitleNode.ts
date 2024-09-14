import { HeadingNode, SerializedHeadingNode } from "@lexical/rich-text";
import { EditorConfig } from "lexical";

interface SerializedFacetTitleNode extends SerializedHeadingNode {
  uniqueId: string;
  active: boolean;
  honedBy: Array<string>;
}

export class FacetTitleNode extends HeadingNode {
  __uniqueId: string;
  __active: boolean;
  __honedBy: Array<string>;

  constructor(
    uniqueId: string,
    active: boolean = true,
    honedBy: Array<string> = [],

    key?: string,
  ) {
    super(active ? "h2" : "h3", key);
    this.__uniqueId = uniqueId;
    this.__active = active;
    this.__honedBy = honedBy;

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

      node.__key,
    );
  }

  createDOM(config: EditorConfig) {
    const dom = super.createDOM(config);
    dom.setAttribute("data-facet-title-id", this.__uniqueId);
    dom.setAttribute("data-active", this.__active.toString());

    return dom;
  }

  exportJSON(): SerializedFacetTitleNode {
    return {
      ...super.exportJSON(),
      type: "facet-title",
      uniqueId: this.__uniqueId,
      active: this.__active,
      honedBy: this.__honedBy,
    };
  }

  static importJSON(serializedNode: SerializedFacetTitleNode): FacetTitleNode {
    const { uniqueId, active, honedBy } = serializedNode;
    const newNode = new FacetTitleNode(uniqueId, active, honedBy);

    Object.defineProperty(newNode, "__active", {
      writable: true,
      value: active,
    });

    return newNode;
  }

  getUniqueId(): string {
    return this.__uniqueId;
  }

  // Methods to manipulate honedBy and calculate honedAmount
  addHonedBy(facetTitleNodeId: string) {
    const writableNode = this.getWritable();
    writableNode.__honedBy.push(facetTitleNodeId);
  }

  isActive(): boolean {
    return this.__active;
  }
}
