import React from "react";
import "./styles.css";
import {
  BrowserRouter as Router,
  Routes,
  Route,
  useParams,
} from "react-router-dom";
import Editor from "./components/Editor";
import Home from "./components/Home";

// Extract params id from the URL and pass it to the Editor component
const EditorWithParams: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  if (id) {
    return <Editor key={id} articleId={id} />;
  }
  return null;
};

const App: React.FC = () => {
  return (
    <Router>
      <Routes>
        {/* Route for Editor that takes up the full window */}
        <Route path="/editor/:id" element={<EditorWithParams />} />
        {/* All other routes within Home layout */}
        <Route path="/*" element={<Home />} />
      </Routes>
    </Router>
  );
};

export default App;
