import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.tsx";
import {
  INITIALIZED_DATA_PATH,
  HONE_DATA_KEY,
  FACETS_DATA_KEY,
  FACET_LIBRARY_KEY,
  HONE_ARTICLE_EDITIONS_KEY,
} from "./constants/storage.ts";
import { normalizeFacetsPayload } from "./utils/facetsPayload.ts";

// Initialize for hone editor and facets page by isFacets
// If you are here for hone only, just simply set the environment variable VITE_IS_FACETS to false
const initializeApp = async () => {
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  // For the editor, decorate styles and initialize with the data from the local storage
  if (isEditable) {
    const root = document.documentElement;
    root.style.setProperty("--light-white", "#EFEFE4");
    root.style.setProperty("--dark-white", "#EFEFE4");

    const existingData = localStorage.getItem(HONE_DATA_KEY);

    if (existingData) {
      return;
    } else {
      try {
        const response = await fetch(INITIALIZED_DATA_PATH);
        if (!response.ok) {
          throw new Error(
            "Failed to fetch" + INITIALIZED_DATA_PATH + response.status,
          );
        }

        const data = await response.json();
        const normalized = normalizeFacetsPayload(data);
        localStorage.setItem(
          HONE_DATA_KEY,
          JSON.stringify(normalized.honeData),
        );
        localStorage.setItem(
          FACET_LIBRARY_KEY,
          JSON.stringify(normalized.facetsLibrary),
        );
        localStorage.setItem(
          HONE_ARTICLE_EDITIONS_KEY,
          JSON.stringify(normalized.articleEditions),
        );
        console.log("Initialized app with " + INITIALIZED_DATA_PATH);
      } catch (error) {
        console.error(
          "Failed to initialize the app with " + INITIALIZED_DATA_PATH,
          error,
        );
      }
    }
  }

  // For the facets page, initialize with the data from the environment variable
  if (!isEditable) {
    const facetsDataUrl = import.meta.env.VITE_FACETS_DATA_URL;
    if (facetsDataUrl) {
      try {
        const response = await fetch(facetsDataUrl);
        if (!response.ok) {
          throw new Error("Failed to fetch facets data");
        }
        const data = await response.json();
        localStorage.setItem(FACETS_DATA_KEY, JSON.stringify(data));
        const normalized = normalizeFacetsPayload(data);
        localStorage.setItem(
          FACET_LIBRARY_KEY,
          JSON.stringify(normalized.facetsLibrary),
        );
        localStorage.setItem(
          HONE_ARTICLE_EDITIONS_KEY,
          JSON.stringify(normalized.articleEditions),
        );
        if (Object.keys(normalized.honeData).length > 0) {
          localStorage.setItem(
            HONE_DATA_KEY,
            JSON.stringify(normalized.honeData),
          );
        }
        console.log("Initialized app with the fetched facets data");
      } catch (error) {
        console.error("Failed to initialize the facets data", error);
      }
    }
  }
};

initializeApp().finally(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
});
