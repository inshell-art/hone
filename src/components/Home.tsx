// Home
import React from "react";
import { Route, Routes, NavLink, Navigate } from "react-router-dom";
import Articles from "./Articles";
import Facets from "./Facets";
import { v4 as uuidv4 } from "uuid";

const Home: React.FC = () => {
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
          <NavLink to={`/editor/${uuidv4()}`} className="nav-link-create">
            Create Article
          </NavLink>
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
            Hone is crafted by Inshell
          </a>
        </div>
      </footer>
    </div>
  );
};

export default Home;
