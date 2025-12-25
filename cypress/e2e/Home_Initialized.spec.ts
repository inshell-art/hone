/*
 * @description: E2E tests for the Home page initialized
 * Initialized Hone (not Home) is the state the GettingStarted.json file is imported into Hone
 * That is the state "with data", in contrast to the state "without data" (EmptyData))
 * So the Initialized and NoData states together constitute cy testing for Hone in MECE.
 */

import { HoneData } from "../../src/types/types";
import {
  FACET_LIBRARY_KEY,
  HONE_DATA_KEY,
  INITIALIZED_DATA_PATH,
} from "../../src/constants/storage";
import { extractFacets } from "../../src/utils/extractFacets";

describe("Home Initialized E2E Tests", () => {
  let honeData: HoneData = {};

  before(() => {
    cy.readFile(`./public${INITIALIZED_DATA_PATH}`).then((data) => {
      honeData = data;
    });
  });

  beforeEach(() => {
    cy.clearLocalStorage();
    cy.visit("/");
  });

  it("should display articles list correctly", () => {
    cy.contains("Articles").click();
    const articlesCount = Object.keys(honeData).length;

    cy.url().should("include", "/articles");
    cy.get(".content-container").within(() => {
      cy.get(".articles-list").within(() => {
        cy.get("li").should("have.length", articlesCount);
      });
    });
  });

  it("should display facets list correctly", () => {
    cy.contains("Facets").click();

    cy.url().should("include", "/facets");
    cy.get(".no-facets").should(
      "contain.text",
      "No facets in the library yet."
    );
  });

  it("should trigger download when Export link is clicked", () => {
    cy.window().then((win) => {
      cy.spy(win.document, "createElement").as("createElement");
    });

    cy.contains("Export").click();

    cy.get("@createElement").should("be.calledWith", "a");
  });

  it("removes orphaned facets from the library", () => {
    const orphanFacetId = "orphan-facet-123";
    const now = Date.now();
    const facets = extractFacets(honeData);
    const liveFacet = facets[0];
    const liveTitle = liveFacet?.title.trim();

    if (!liveFacet || !liveTitle) {
      throw new Error("Expected at least one facet in initialized data.");
    }

    const library = {
      version: 2,
      updatedAt: now,
      facetsById: {
        [liveFacet.facetId]: {
          facetId: liveFacet.facetId,
          title: liveFacet.title,
          bodyText: liveFacet.content.join("\n"),
          updatedAt: now,
          honedFrom: [],
        },
        [orphanFacetId]: {
          facetId: orphanFacetId,
          title: "Orphan Facet",
          bodyText: "Missing content",
          updatedAt: now,
          honedFrom: [],
        },
      },
    };

    cy.visit("/facets", {
      onBeforeLoad(win) {
        win.localStorage.setItem(HONE_DATA_KEY, JSON.stringify(honeData));
        win.localStorage.setItem(
          FACET_LIBRARY_KEY,
          JSON.stringify(library),
        );
      },
    });

    cy.contains(".facet-link", liveTitle).should("be.visible");
    cy.contains(".facet-link", "Orphan Facet").should("not.exist");

    cy.window().then((win) => {
      const stored = JSON.parse(
        win.localStorage.getItem(FACET_LIBRARY_KEY) || "{}",
      );
      expect(stored.facetsById).to.have.property(liveFacet.facetId);
      expect(stored.facetsById).to.not.have.property(orphanFacetId);
    });
  });
});
