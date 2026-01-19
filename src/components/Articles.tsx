import React, { useState, useEffect } from "react";
import { ArticlePublishState, HoneData } from "../types/types";
import { SerializedArticleTitleNode } from "../models/ArticleTitleNode";
import { SerializedTextNode } from "lexical";
import { formatTimestamp } from "../utils/utils";
import { HONE_ARTICLE_EDITIONS_KEY, HONE_DATA_KEY } from "../constants/storage";
import {
  ARTICLE_EDITIONS_UPDATED_EVENT,
  buildPublishedArticlesIndex,
  loadArticleEditions,
} from "../utils/articleEditions";

const Articles: React.FC = () => {
  const [articles, setArticles] = useState<HoneData>({});
  const [publishState, setPublishState] = useState<ArticlePublishState>(
    loadArticleEditions(),
  );
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  useEffect(() => {
    const refreshPublish = () => {
      setPublishState(loadArticleEditions());
    };
    const handleStorage = (event: StorageEvent) => {
      if (event.key === HONE_ARTICLE_EDITIONS_KEY) {
        refreshPublish();
      }
    };

    window.addEventListener("storage", handleStorage);
    window.addEventListener(ARTICLE_EDITIONS_UPDATED_EVENT, refreshPublish);

    return () => {
      window.removeEventListener("storage", handleStorage);
      window.removeEventListener(
        ARTICLE_EDITIONS_UPDATED_EVENT,
        refreshPublish,
      );
    };
  }, []);

  useEffect(() => {
    if (!isEditable) {
      setArticles({});
      return;
    }

    const storedArticles = localStorage.getItem(HONE_DATA_KEY);
    if (!storedArticles) {
      console.log("No articles found in localStorage");
      setArticles({});
      return;
    }

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
      const latestVersion = publishState.articles[id]?.latestVersion ?? null;

      return {
        id,
        title,
        updatedAt,
        dateTime,
        latestVersion,
      };
    })
    .sort((a, b) => b.updatedAt - a.updatedAt);

  const publishedItems = buildPublishedArticlesIndex(publishState);

  return (
    <div className="articles-container">
      <ul className="articles-list">
        {isEditable ? (
          articleItems.length > 0 ? (
            articleItems.map(({ id, title, dateTime, latestVersion }) => (
              <li key={id} className="article-item">
                <div className="article-row">
                  <a href={`/a/${id}`} className="article-link">
                    {title}
                  </a>
                  <span className="article-version">
                    <span
                      className={`article-dot ${
                        latestVersion ? "filled" : "hollow"
                      }`}
                    ></span>
                    {latestVersion ? `v${latestVersion}` : ""}
                  </span>
                </div>
                <div className="article-date">{dateTime}</div>
              </li>
            ))
          ) : (
            <li>No articles found</li>
          )
        ) : publishedItems.length > 0 ? (
          publishedItems.map((item) => (
            <li key={item.articleId} className="article-item">
              <div className="article-row">
                <a
                  href={`/a/${item.articleId}/v/${item.latestVersion}`}
                  className="article-link"
                >
                  {item.title}
                </a>
                <span className="article-version">
                  <span className="article-dot filled"></span>v
                  {item.latestVersion}
                </span>
              </div>
              <div className="article-date">
                {formatTimestamp(item.updatedAt)}
              </div>
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
