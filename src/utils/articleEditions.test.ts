import { describe, expect, it } from "vitest";
import {
  createEmptyPublishState,
  publishArticleEdition,
} from "./articleEditions";

const buildContent = (title: string, body: string) => ({
  root: {
    children: [
      {
        children: [
          {
            detail: 0,
            format: 0,
            mode: "normal",
            style: "",
            text: title,
            type: "text",
            version: 1,
          },
        ],
        direction: null,
        format: "",
        indent: 0,
        type: "article-title",
        version: 1,
        tag: "h1",
      },
      {
        children: [
          {
            detail: 0,
            format: 0,
            mode: "normal",
            style: "",
            text: body,
            type: "text",
            version: 1,
          },
        ],
        direction: "ltr",
        format: "",
        indent: 0,
        type: "paragraph",
        version: 1,
        textFormat: 0,
        textStyle: "",
      },
    ],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "root",
    version: 1,
  },
});

describe("publishArticleEdition", () => {
  it("increments version per publish and keeps previous editions intact", () => {
    const initial = createEmptyPublishState();
    const first = publishArticleEdition(initial, {
      articleId: "article-1",
      content: buildContent("Title A", "Body A"),
    });

    expect(first.status).toBe("published");
    expect(first.edition?.version).toBe(1);

    const second = publishArticleEdition(first.state, {
      articleId: "article-1",
      content: buildContent("Title A", "Body B"),
    });

    expect(second.edition?.version).toBe(2);

    const record = second.state.articles["article-1"];
    expect(record.latestVersion).toBe(2);
    expect(record.editionsOrder.length).toBe(2);
    expect(record.editionsById[first.edition?.editionId || ""]?.version).toBe(
      1,
    );
  });
});
