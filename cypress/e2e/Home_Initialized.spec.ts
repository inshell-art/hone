/*
 * @description: E2E tests for the Home page initialized
 * Initialized Hone (not Home) is the state the GettingStarted.json file is imported into Hone
 * That is the state "with data", in contrast to the state "without data" (EmptyData))
 * So the Initialized and NoData states together constitute cy testing for Hone in MECE.
 */

import { HoneData } from "../../src/types/types";
import { extractFacets } from "../../src/utils/extractFacets";
import { INITIALIZED_DATA } from "../../src/utils/utils";

describe("Home Initialized E2E Tests", () => {
  let honeData: HoneData = {};

  before(() => {
    cy.readFile(`./public/${INITIALIZED_DATA}`).then((data) => {
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

    cy.log(`honeData: ${honeData}`);
    const facets = extractFacets(honeData);
    const facetsCount = facets.length;
    cy.log(`facetsCount: ${facetsCount}`);

    cy.url().should("include", "/facets");
    cy.get(".facets-list").within(() => {
      cy.get(".facet-item").should("have.length", facetsCount);
    });
  });

  it("should trigger download when Export link is clicked", () => {
    cy.window().then((win) => {
      cy.spy(win.document, "createElement").as("createElement");
    });

    cy.contains("Export").click();

    cy.get("@createElement").should("be.calledWith", "a");
  });
});
