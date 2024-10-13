/*
 * @description: E2E tests for the Home page when there is empty data in localStorage
 * The state is the pre-initial state
 * So the state is used for functionalities ensuring merely.
 */

import { HONE_DATA, INITIALIZED_DATA } from "../../src/utils/utils";

describe("Home Empty Data E2E Tests", () => {
  beforeEach(() => {
    cy.clearLocalStorage();

    const emptyData = {};
    cy.window().then((win) => {
      win.localStorage.setItem(HONE_DATA, JSON.stringify(emptyData));
    });

    cy.visit("/");
  });

  it("should display navigation links correctly", () => {
    cy.get(".navbar-left").within(() => {
      cy.contains("Facets").should("be.visible");
      cy.contains("Articles").should("be.visible");
    });

    cy.get(".navbar-right").within(() => {
      cy.contains("Create Article").should("be.visible");
    });
  });

  it("should navigate to Article by default when the root path is visited", () => {
    cy.url().should("include", "/articles");
    cy.get(".content-container").within(() => {
      cy.get(".articles-list").should("be.visible");
    });
    cy.contains("Articles").should("have.class", "active");
  });

  it("should navigate to Facets page when Facets link is clicked", () => {
    cy.contains("Facets").click();

    cy.url().should("include", "/facets");
    cy.get(".no-facets").should("contain.text", "No facets found");
    cy.contains("Facets").should("have.class", "active");
    cy.contains("Articles").should("not.have.class", "active");
  });

  it("should navigate to Articles page when Articles link is clicked", () => {
    cy.contains("Articles").click();

    cy.url().should("include", "/articles");
    cy.get(".content-container").within(() => {
      cy.get(".articles-list").within(() => {
        cy.get("li").should("contain.text", "No articles found");
      });
    });
    cy.contains("Articles").should("have.class", "active");
    cy.contains("Facets").should("not.have.class", "active");
  });

  it("should navigate to Editor page when Create Article link is clicked", () => {
    cy.contains("Create Article").click();

    cy.url().should("match", /\/editor\/[a-f0-9-]{36}$/);
    cy.get(".editor-container").should("be.visible");
    cy.get(".editor-placeholder").should(
      "contain.text",
      "Type your article title here..."
    );
    cy.get(".editor-input").should("be.visible");
  });

  it('should redirect root path "/" to "/articles"', () => {
    cy.visit("/");
    cy.url().should("include", "/articles");
  });

  it("should display footer links correctly", () => {
    cy.get(".footer-left").within(() => {
      cy.contains("Import").should("be.visible");
      cy.contains("Export").should("be.visible");
    });

    cy.get(".footer-right").within(() => {
      cy.contains("Hone is crafted by Inshell")
        .should("be.visible")
        .and("have.attr", "href", "https://inshell.art");
    });
  });

  it("should trigger the file input click when the import link is clicked", () => {
    cy.get('input[type="file"]').then(($input) => {
      const spy = cy.spy($input[0], "click");
      cy.get('a[href="#import"]')
        .click()
        .then(() => {
          expect(spy).to.be.calledOnce;
        });
    });
  });

  it("should import a JSON file, store data in localStorage, and display it", () => {
    cy.get('input[type="file"]')
      .should("exist")
      .selectFile(`./public${INITIALIZED_DATA}`, { force: true }); // Share from initial data

    cy.wait(100);

    cy.window().then((win) => {
      const savedData = JSON.parse(win.localStorage.getItem(HONE_DATA) || "{}");
      cy.log("Saved data:", savedData);

      expect(Object.keys(savedData)).to.have.length(2);
    });

    cy.get(".articles-list").within(() => {
      cy.get("li").should("have.length", 2);
    });
  });

  it("should show no articles to export when export link is clicked", () => {
    cy.get('a[href="#export"]').click();

    cy.on("window:alert", (str) => {
      expect(str).to.equal("No articles to export.");
    });
  });

  it("should link to Inshell's website when the footer link is clicked", () => {
    cy.get(".footer-right .footer-link").should(
      "have.attr",
      "href",
      "https://inshell.art"
    );
  });
});
