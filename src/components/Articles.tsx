import React, { useState, useEffect } from "react";
import { Article, ArticleContent } from "../types/types";

const Articles: React.FC = () => {
  const [articles, setArticles] = useState<Article[]>([]);

  useEffect(() => {
    const storedArticles = localStorage.getItem("HoneEditorArticles");
    if (storedArticles) {
      try {
        const parsedArticles: Record<string, ArticleContent> =
          JSON.parse(storedArticles);

        if (parsedArticles && typeof parsedArticles === "object") {
          const articlesArray = Object.entries(parsedArticles).map(
            ([id, content]: [string, ArticleContent]) => {
              const headingNode = content.root.children.find(
                (node) => node.type === "heading" && node.tag === "h1"
              );

              const title =
                headingNode?.children?.[0]?.text || "Untitled Article";

              return {
                id,
                title,
                content,
              };
            }
          );

          setArticles(articlesArray);
        } else {
          console.error(
            "Parsed articles data is not an object:",
            parsedArticles
          );
          setArticles([]);
        }
      } catch (error) {
        console.error("Failed to parse articles from localStorage:", error);
        setArticles([]);
      }
    } else {
      console.log("No articles found in localStorage");
      setArticles([]);
    }
  }, []);

  return (
    <div className="articles-container">
      <ul className="articles-list">
        {articles.length > 0 ? (
          articles.map((article) => (
            <li key={article.id} className="article-item">
              <a href={`/editor/${article.id}`} className="article-link">
                {article.title}
              </a>
              <div className="article-date">2024-01-01</div>
            </li>
          ))
        ) : (
          <li>No articles found</li>
        )}
      </ul>
    </div>
  );
};

export default Articles;
