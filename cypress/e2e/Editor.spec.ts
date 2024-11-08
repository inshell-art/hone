import { v4 as uuid4 } from "uuid";
import { INSERT_SYMBOL } from "../../src/utils/utils";

describe("Editor E2E Tests", () => {
  let articleId = "";
  const ARTICLE_TITLE = "This is a test article";
  const FACET_TITLE = "$ This is a test facet";
  const PARAGRAPH_TEXT = "This is a test paragraph";

  before(() => {
    articleId = uuid4();
  });

  beforeEach(() => {
    cy.visit(`/article/${articleId}`);
  });

  it("should load the editor with empty data", () => {
    cy.get(".editor-container").should("be.visible");
    cy.get(".editor-placeholder").should(
      "contain.text",
      "Type your article title here..."
    );
    cy.get(".editor-input").should("have.text", "");
    cy.contains("No article found").should("be.visible");
    cy.contains("Skipping auto-save").should("be.visible");
  });

  it("should trigger auto save when article title, facet title and paragraph text are added, and delete the article stored after clear the editor  ", () => {
    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");
    cy.get(".editor-input").type(FACET_TITLE + "{enter}");
    cy.get(".editor-input").type(PARAGRAPH_TEXT);

    cy.wait(1000);

    cy.contains("Auto-saved changes to localStorage in 1 second.").should(
      "be.visible"
    );

    cy.get(".editor-input")
      .find("h1.article-title")
      .should("have.text", ARTICLE_TITLE);

    cy.get(".editor-input")
      .find("h2.facet-title")
      .should("have.text", FACET_TITLE);

    cy.get(".editor-input").find("p").should("have.text", PARAGRAPH_TEXT);

    cy.window().then((win) => {
      const savedArticles = win.localStorage.getItem("honeData");
      expect(savedArticles).to.not.be.null;
      const parsedArticles = JSON.parse(savedArticles as string);
      expect(parsedArticles[articleId]).to.not.be.undefined;
    });

    cy.get(".editor-input").clear();

    cy.wait(1000);

    cy.window().then((win) => {
      const savedArticles = win.localStorage.getItem("honeData");
      expect(savedArticles).to.not.be.null;
      const parsedArticles = JSON.parse(savedArticles as string);
      expect(parsedArticles[articleId]).to.be.undefined;
    });
  });

  it("should trigger hone panel and insert facet correctly", () => {
    const cmdOrCtrl = Cypress.platform === "darwin" ? "{cmd}" : "{ctrl}";

    cy.get(".editor-input").type(ARTICLE_TITLE + "{enter}");
    cy.get(".editor-input").type("$ facet 1{enter}");
    cy.get(".editor-input").type("facet 1 content{enter}");
    cy.get(".editor-input").type("$ facet 2{enter}");
    cy.get(".editor-input").type("facet 2 content{enter}");

    cy.wait(1000);

    cy.get(".editor-input").type(`${cmdOrCtrl}{enter}`);

    cy.get(".hone-panel").should("be.visible");

    cy.get(".hone-panel-item").contains("$ facet 1").click();

    cy.get(".editor-input")
      .find("p")
      .should("contain.text", INSERT_SYMBOL)
      .and("contain.text", "$ facet 1")
      .and("contain.text", "facet 1 content");
  });

  it("should load specified article and verify content", () => {
    cy.visit("/");
    cy.window().then((win) => {
      const savedArticles = win.localStorage.getItem("honeData");

      expect(savedArticles).to.not.be.null;

      const parsedArticles = JSON.parse(savedArticles as string);

      const articleId = Object.keys(parsedArticles)[0];
      const firstLineText =
        parsedArticles[articleId].content.root.children[0].children[0].text;

      cy.visit(`/article/${articleId}`);

      cy.wait(1000);

      cy.get(".editor-input")
        .find("h1")
        .invoke("text")
        .then((text) => {
          cy.log("Text of the first div:", text);
          expect(text).to.equal(firstLineText);
        });
    });
  });
});
