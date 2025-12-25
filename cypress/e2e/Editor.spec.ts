import { v4 as uuid4 } from "uuid";
import { FACET_LIBRARY_KEY, HONE_DATA_KEY } from "../../src/constants/storage";

describe("Editor E2E Tests", () => {
  let articleId = "";
  const ARTICLE_TITLE = "This is a test article";
  const FACET_TITLE = "$ This is a test facet";
  const PARAGRAPH_TEXT = "This is a test paragraph";

  before(() => {
    articleId = uuid4();
  });

  beforeEach(() => {
    cy.clearLocalStorage();
    cy.visit(`/article/${articleId}`);
  });

  it("should load the editor with empty data", () => {
    cy.get(".editor-container").should("be.visible");
    cy.get(".editor-placeholder").should(
      "contain.text",
      "Type your article title here...",
    );
    cy.get(".editor-input").should("have.text", "");
    cy.contains("Skipping auto-save: content has no text.").should(
      "be.visible",
    );
  });

  it("should auto save and delete the article after clearing the editor", () => {
    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");
    cy.get(".editor-input").type(FACET_TITLE + "{enter}");
    cy.get(".editor-input").type(PARAGRAPH_TEXT);

    cy.wait(1000);

    cy.contains("Auto-saved changes to localStorage in 1 second.").should(
      "be.visible",
    );

    cy.get(".editor-input")
      .find("h1.article-title")
      .should("have.text", ARTICLE_TITLE);

    cy.get(".editor-input")
      .find("h2.facet-title")
      .should("have.text", FACET_TITLE);

    cy.get(".editor-input").find("p").should("have.text", PARAGRAPH_TEXT);

    cy.window().then((win) => {
      const savedArticles = win.localStorage.getItem(HONE_DATA_KEY);
      expect(savedArticles).to.not.be.null;
      const parsedArticles = JSON.parse(savedArticles as string);
      expect(parsedArticles[articleId]).to.not.be.undefined;
    });

    cy.get(".editor-input").clear();

    cy.wait(1000);

    cy.window().then((win) => {
      const savedArticles = win.localStorage.getItem(HONE_DATA_KEY);
      expect(savedArticles).to.not.be.null;
      const parsedArticles = JSON.parse(savedArticles as string);
      expect(parsedArticles[articleId]).to.be.undefined;
    });
  });

  it("opens slash commands to create and update a facet", () => {
    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");
    cy.get(".editor-input").type("/");

    cy.get(".command-palette").should("be.visible");
    cy.contains(".command-title", "/create").click();

    cy.get(".editor-input").type("Facet Alpha{enter}Facet alpha body");
    cy.get(".editor-input").find("h2.facet-title").click();
    cy.get(".editor-input").type("{home}/");

    cy.contains(".command-title", "/update").click();

    cy.window().then((win) => {
      const libraryRaw = win.localStorage.getItem(FACET_LIBRARY_KEY);
      expect(libraryRaw).to.not.be.null;
      const library = JSON.parse(libraryRaw as string);
      expect(Object.keys(library.facetsById || {})).to.have.length(1);
    });
  });

  it("inserts a honed facet block with header and footer", () => {
    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");

    cy.get(".editor-input").type("/");
    cy.get(".command-palette").should("be.visible");
    cy.contains(".command-title", "/create").click();
    cy.get(".editor-input").type("Facet One{enter}First body");

    cy.get(".editor-input").find("h2.facet-title").first().click();
    cy.get(".editor-input").type("{home}/");
    cy.contains(".command-title", "/update").click();

    cy.get(".editor-input").find("p").last().click();
    cy.get(".editor-input").type("{enter}{home}/");
    cy.contains(".command-title", "/create").click();
    cy.get(".editor-input").type("Facet Two{enter}Second body");

    cy.get(".editor-input").find("h2.facet-title").eq(1).click();
    cy.get(".editor-input").type("{home}/");
    cy.contains(".command-title", "/update").click();

    cy.get(".editor-input").find("p").last().click();
    cy.get(".editor-input").type("{home}/");
    cy.contains(".command-title", "/hone").click();
    cy.contains(".command-title", "Facet One").click();

    cy.get(".editor-input")
      .invoke("text")
      .then((text) => {
        expect(text).to.include("--- honed-from:");
        expect(text).to.include("--- end honed-from ---");
      });
  });

  it("should persist and reload an article", () => {
    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");
    cy.get(".editor-input").type(FACET_TITLE + "{enter}");
    cy.get(".editor-input").type(PARAGRAPH_TEXT);

    cy.wait(1000);

    cy.visit(`/article/${articleId}`);

    cy.wait(500);

    cy.get(".editor-input")
      .find("h1.article-title")
      .invoke("text")
      .should("eq", ARTICLE_TITLE);
    cy.get(".editor-input")
      .find("h2.facet-title")
      .invoke("text")
      .should("eq", FACET_TITLE);
  });
});
