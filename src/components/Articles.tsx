import React, { useState, useEffect } from "react";
import { ArticleRecord } from "../types/types";

const Articles: React.FC = () => {
  const [articles, setArticles] = useState<ArticleRecord>({});

  useEffect(() => {
    const storedArticles = localStorage.getItem("HoneEditorArticles");
    if (storedArticles) {
      try {
        const parsedArticles: ArticleRecord = JSON.parse(storedArticles);

        if (parsedArticles && typeof parsedArticles === "object") {
          setArticles(parsedArticles);
        } else {
          console.error(
            "Parsed articles data is not an object:",
            parsedArticles
          );
          setArticles({});
        }
      } catch (error) {
        console.error("Failed to parse articles from localStorage:", error);
        setArticles({});
      }
    } else {
      console.log("No articles found in localStorage");
      setArticles({});
    }
  }, []);

  return (
    <div className="articles-container">
      <ul className="articles-list">
        {Object.keys(articles).length > 0 ? (
          Object.entries(articles).map(([id, content]) => {
            const headingNode = content.root.children.find(
              (node) => "tag" in node && node.tag === "h1"
            );

            const title =
              headingNode?.children?.[0]?.text || "Untitled Article";

            return (
              <li key={id} className="article-item">
                <a href={`/editor/${id}`} className="article-link">
                  {title}
                </a>
                <div className="article-date">2024-01-01</div>
              </li>
            );
          })
        ) : (
          <li>No articles found</li>
        )}
      </ul>
    </div>
  );
};

export default Articles;
