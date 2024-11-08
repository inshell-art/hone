import React, { useState, useEffect } from "react";
import { HoneData } from "../types/types";
import { SerializedArticleTitleNode } from "../models/ArticleTitleNode";
import { SerializedTextNode } from "lexical";
import { formatTimestamp } from "../utils/utils";

const Articles: React.FC = () => {
  const [articles, setArticles] = useState<HoneData>({});
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  useEffect(() => {
    const storedArticles = isEditable
      ? localStorage.getItem("honeData")
      : localStorage.getItem("facetsData");
    if (storedArticles) {
      try {
        const parsedArticles: HoneData = JSON.parse(storedArticles);

        if (parsedArticles) {
          setArticles(parsedArticles);
        } else {
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
  }, [isEditable]);

  const articleItems = Object.entries(articles)
    .map(([id, { content, updatedAt }]) => {
      const articleTitleNode = content.root.children.find(
        (node) => "type" in node && node.type === "article-title",
      );

      const textNode = (articleTitleNode as SerializedArticleTitleNode)
        .children?.[0] as SerializedTextNode;

      const title =
        textNode.text.trim().length > 0 ? textNode.text : "Untitled Article";
      const dateTime = formatTimestamp(updatedAt);

      return {
        id,
        title,
        updatedAt,
        dateTime,
      };
    })
    .sort((a, b) => b.updatedAt - a.updatedAt);

  return (
    <div className="articles-container">
      <ul className="articles-list">
        {articleItems.length > 0 ? (
          articleItems.map(({ id, title, dateTime }) => (
            <li key={id} className="article-item">
              <a href={`/article/${id}`} className="article-link">
                {title}
              </a>
              <div className="article-date">{dateTime}</div>
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
