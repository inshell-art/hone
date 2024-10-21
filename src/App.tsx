import React, { useEffect } from "react";
import "./styles.css";
import {
  BrowserRouter as Router,
  Routes,
  Route,
  useParams,
  useLocation,
} from "react-router-dom";
import { logEvent } from "firebase/analytics";
import Editor from "./components/Editor";
import Home from "./components/Home";
import { analytics } from "./firebase";

const EditorWithParams: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  if (id) {
    return <Editor key={id} articleId={id} />;
  }
  return null;
};

const RouteTracker: React.FC = () => {
  const location = useLocation();

  useEffect(() => {
    if (analytics) {
      logEvent(analytics, "page_view", { page_path: location.pathname });
    }
  }, [location]);

  return null;
};

const App: React.FC = () => {
  return (
    <Router>
      <RouteTracker />
      <Routes>
        <Route path="/editor/:id" element={<EditorWithParams />} />
        <Route path="/*" element={<Home />} />
      </Routes>
    </Router>
  );
};

export default App;
