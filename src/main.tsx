import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.tsx";
import { INITIALIZED_DATA } from "./utils/utils.ts";

// Initialize for hone editor and facets page by isFacets
// If you are here for hone only, just simply set the environment variable VITE_IS_FACETS to false
const initializeApp = async () => {
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  // For the editor, decorate styles and initialize with the data from the local storage
  if (isEditable) {
    const root = document.documentElement;
    root.style.setProperty("--light-white", "#EFEFE4");
    root.style.setProperty("--dark-white", "#EFEFE4");

    const existingData = localStorage.getItem("honeData");

    if (existingData) {
      return;
    } else {
      try {
        const response = await fetch(INITIALIZED_DATA);
        if (!response.ok) {
          throw new Error(
            "Failed to fetch" + INITIALIZED_DATA + response.status,
          );
        }

        const data = await response.json();
        localStorage.setItem("honeData", JSON.stringify(data));
        console.log("Initialized app with " + INITIALIZED_DATA);
      } catch (error) {
        console.error(
          "Failed to initialize the app with " + INITIALIZED_DATA,
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
        localStorage.setItem("facetsData", JSON.stringify(data));
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
