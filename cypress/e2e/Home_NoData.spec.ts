describe("Home No Data E2E Tests", () => {
  beforeEach(() => {
    cy.clearLocalStorage();
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

  it("should navigate to #import section when Import link is clicked", () => {
    cy.contains("Import").click();
    cy.url().should("include", "#import");
  });

  it("should navigate to #export section when Export link is clicked", () => {
    cy.contains("Export").click();
    cy.url().should("include", "#export");
  });
});
