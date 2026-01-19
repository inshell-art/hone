import { v4 as uuidv4 } from "uuid";
import {
  SerializedEditorState,
  SerializedElementNode,
  SerializedTextNode,
} from "lexical";
import {
  ArticleEdition,
  ArticlePublishRecord,
  ArticlePublishState,
  PublishedArticleSummary,
} from "../types/types";
import { HONE_ARTICLE_EDITIONS_KEY } from "../constants/storage";

const EDITIONS_VERSION = 1;
export const ARTICLE_EDITIONS_UPDATED_EVENT = "hone-article-editions-updated";

export const createEmptyPublishState = (): ArticlePublishState => ({
  version: EDITIONS_VERSION,
  updatedAt: Date.now(),
  articles: {},
});

const emitEditionsUpdated = () => {
  if (typeof window === "undefined") {
    return;
  }
  window.dispatchEvent(new Event(ARTICLE_EDITIONS_UPDATED_EVENT));
};

const isValidPublishState = (
  value: ArticlePublishState | null,
): value is ArticlePublishState => {
  return (
    !!value &&
    value.version === EDITIONS_VERSION &&
    typeof value.updatedAt === "number" &&
    typeof value.articles === "object"
  );
};

export const loadArticleEditions = (): ArticlePublishState => {
  const raw = localStorage.getItem(HONE_ARTICLE_EDITIONS_KEY);
  if (!raw) {
    return createEmptyPublishState();
  }

  try {
    const parsed = JSON.parse(raw) as ArticlePublishState;
    if (isValidPublishState(parsed)) {
      return parsed;
    }
  } catch (error) {
    console.error("Failed to parse article editions", error);
  }

  return createEmptyPublishState();
};

export const saveArticleEditions = (
  state: ArticlePublishState,
): ArticlePublishState => {
  const nextState: ArticlePublishState = {
    version: EDITIONS_VERSION,
    updatedAt: Date.now(),
    articles: state.articles,
  };

  localStorage.setItem(HONE_ARTICLE_EDITIONS_KEY, JSON.stringify(nextState));
  emitEditionsUpdated();
  return nextState;
};

const hashContent = (content: unknown): string => {
  const input = JSON.stringify(content ?? "");
  let hash = 2166136261;

  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }

  return (hash >>> 0).toString(16);
};

export const extractTitleFromContent = (
  content: SerializedEditorState | unknown,
): string => {
  const root = (content as SerializedEditorState | undefined)?.root;
  if (!root || !("children" in root)) {
    return "Untitled";
  }
  const children = (root as SerializedElementNode).children;
  if (!Array.isArray(children)) {
    return "Untitled";
  }

  const titleNode = children.find(
    (node) => node && node.type === "article-title",
  ) as SerializedElementNode | undefined;
  if (!titleNode || !("children" in titleNode)) {
    return "Untitled";
  }
  const textNode = titleNode.children?.[0] as SerializedTextNode | undefined;
  const text = typeof textNode?.text === "string" ? textNode.text.trim() : "";

  return text.length > 0 ? text : "Untitled";
};

type PublishInput = {
  articleId: string;
  content: unknown;
  title?: string;
};

type PublishResult = {
  status: "published" | "duplicate";
  state: ArticlePublishState;
  edition?: ArticleEdition;
  latestVersion: number;
};

export const publishArticleEdition = (
  state: ArticlePublishState,
  input: PublishInput,
): PublishResult => {
  const article = state.articles[input.articleId];
  const latestVersion = article?.latestVersion ?? 0;
  const contentHash = hashContent(input.content);
  const headEdition = article?.headEditionId
    ? article.editionsById[article.headEditionId]
    : undefined;

  if (headEdition?.contentHash === contentHash) {
    return { status: "duplicate", state, latestVersion };
  }

  const nextVersion = latestVersion + 1;
  const editionId = uuidv4();
  const title = input.title?.trim() || extractTitleFromContent(input.content);
  const edition: ArticleEdition = {
    editionId,
    articleId: input.articleId,
    version: nextVersion,
    createdAt: Date.now(),
    title,
    content: input.content as ArticleEdition["content"],
    contentHash,
  };

  const nextRecord: ArticlePublishRecord = {
    headEditionId: editionId,
    latestVersion: nextVersion,
    editionsById: {
      ...(article?.editionsById ?? {}),
      [editionId]: edition,
    },
    editionsOrder: [editionId, ...(article?.editionsOrder ?? [])],
  };

  const nextState: ArticlePublishState = {
    version: EDITIONS_VERSION,
    updatedAt: Date.now(),
    articles: {
      ...state.articles,
      [input.articleId]: nextRecord,
    },
  };

  return { status: "published", state: nextState, edition, latestVersion };
};

export const getArticleRecord = (
  state: ArticlePublishState,
  articleId: string,
): ArticlePublishRecord | null => {
  return state.articles[articleId] ?? null;
};

export const getEditionByVersion = (
  state: ArticlePublishState,
  articleId: string,
  version: number,
): ArticleEdition | null => {
  const article = state.articles[articleId];
  if (!article) {
    return null;
  }

  const editionId = article.editionsOrder.find(
    (id) => article.editionsById[id]?.version === version,
  );

  return editionId ? article.editionsById[editionId] : null;
};

export const getEditionsForArticle = (
  state: ArticlePublishState,
  articleId: string,
): ArticleEdition[] => {
  const article = state.articles[articleId];
  if (!article) {
    return [];
  }

  return article.editionsOrder
    .map((id) => article.editionsById[id])
    .filter(Boolean);
};

export const buildPublishedArticlesIndex = (
  state: ArticlePublishState,
): PublishedArticleSummary[] => {
  return Object.entries(state.articles)
    .map(([articleId, record]) => {
      const headEdition = record.editionsById[record.headEditionId];
      return {
        articleId,
        title: headEdition?.title ?? "Untitled",
        latestVersion: record.latestVersion,
        updatedAt: headEdition?.createdAt ?? state.updatedAt,
      };
    })
    .sort((a, b) => b.updatedAt - a.updatedAt);
};
