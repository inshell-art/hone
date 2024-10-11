import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.tsx";

const initializeApp = async () => {
  const existingData = localStorage.getItem("HoneEditorArticles");

  if (!existingData) {
    try {
      const response = await fetch("/GettingStarted.json");
      if (!response.ok) {
        throw new Error(
          "Failed to fetch GettingStarted.json:" + response.status,
        );
      }

      const data = await response.json();
      localStorage.setItem("HoneEditorArticles", JSON.stringify(data));
      console.log("Initialized app with GettingStarted.json");
    } catch (error) {
      console.error(
        "Failed to initialize the app with GettingStarted.json:",
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
