import React from "react";
import {
  BrowserRouter as Router,
  Route,
  Routes,
  NavLink,
  Navigate,
  useParams,
} from "react-router-dom";
import Articles from "./Articles";
import Facets from "./Facets";
import Editor from "./Editor";
import { v4 as uuidv4 } from "uuid";

// Extract params id from the URL and pass it to the Editor component
const EditorWithParams: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  if (id) {
    return <Editor key={id} articleId={id} />;
  }
  return null;
};

const Home: React.FC = () => {
  return (
    <Router>
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
            <NavLink to={`/editor/${uuidv4()}`} className="nav-link-create">
              Create Article
            </NavLink>
          </div>
        </nav>
        <div className="content-container">
          <Routes>
            <Route path="/facets" element={<Facets />} />
            <Route path="/articles" element={<Articles />} />
            <Route path="/editor/:id" element={<EditorWithParams />} />
            <Route path="/" element={<Navigate to="/articles" />} />
          </Routes>
        </div>
        <footer className="footer">
          <div className="footer-left">
            <a href="#import" className="footer-link">
              Import
            </a>
            <a href="#export" className="footer-link">
              Export
            </a>
          </div>
          <div className="footer-right">
            <a href="https://hone.example.com" className="footer-link">
              Craft by Inshell
            </a>
          </div>
        </footer>
      </div>
    </Router>
  );
};

export default Home;
