import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.tsx";
import { INITIALIZED_DATA, HONE_DATA } from "./utils/utils.ts";

const initializeApp = async () => {
  const existingData = localStorage.getItem(HONE_DATA);

  if (!existingData) {
    try {
      const response = await fetch(INITIALIZED_DATA);
      if (!response.ok) {
        throw new Error("Failed to fetch" + INITIALIZED_DATA + response.status);
      }

      const data = await response.json();
      localStorage.setItem(HONE_DATA, JSON.stringify(data));
      console.log("Initialized app with " + INITIALIZED_DATA);
    } catch (error) {
      console.error(
        "Failed to initialize the app with " + INITIALIZED_DATA,
        error,
      );
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
