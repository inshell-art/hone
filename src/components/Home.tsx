import React from "react";
import { Route, Routes, NavLink, Navigate } from "react-router-dom";
import Articles from "./Articles";
import Facets from "./Facets";
import { v4 as uuidv4 } from "uuid";
import { exportSavedArticles, importSavedArticles } from "../utils/utils";

const Home: React.FC = () => {
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";

  return (
    <div className="home-container">
      <nav className="navbar">
        <div className="navbar-left">
          <NavLink
            to="/facets"
            className={({ isActive }) =>
              isActive ? "nav-link-facets active" : "nav-link-facets"
            }
          >
            Facets
          </NavLink>
          <NavLink
            to="/articles"
            className={({ isActive }) =>
              isActive ? "nav-link-articles active" : "nav-link-articles"
            }
          >
            Articles
          </NavLink>
        </div>
        <div className="navbar-right">
          {isEditable ? (
            <NavLink to={`/a/${uuidv4()}`} className="nav-link-create">
              Create Article
            </NavLink>
          ) : null}
        </div>
      </nav>
      <div className="content-container">
        <Routes>
          <Route path="/facets" element={<Facets />} />
          <Route path="/articles" element={<Articles />} />
          <Route path="/" element={<Navigate to="/articles" />} />
        </Routes>
      </div>
      <footer className="footer">
        {isEditable && (
          <div className="footer-data">
            {/* Hidden file input that allows the user to select a file */}
            <input
              type="file"
              id="fileInput"
              accept="application/json"
              style={{
                display: "none",
              }}
              onChange={(e) => {
                console.log("Importing articles...");
                importSavedArticles(e);
              }}
            />
            {/* Link to trigger the file input */}
            <a
              href="#import"
              onClick={(e) => {
                e.preventDefault();
                document.getElementById("fileInput")?.click();
                console.log("Importing articles...");
              }}
            >
              Import
            </a>

            <a
              href="#export"
              onClick={(e) => {
                e.preventDefault();
                console.log("Exporting articles...");
                exportSavedArticles();
              }}
            >
              Export
            </a>
          </div>
        )}
        <div className="footer-author">
          {isEditable ? (
            <a
              href="https://inshell.art"
              target="_blank"
              rel="noopener noreferrer"
            >
              Hone is crafted by Inshell
            </a>
          ) : (
            <a
              href="https://hone.inshell.art"
              target="_blank"
              rel="noopener noreferrer"
            >
              Facets is crafted by Hone
            </a>
          )}
        </div>
      </footer>
    </div>
  );
};

export default Home;
