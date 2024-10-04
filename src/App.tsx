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
        <Route path="/editor/:id" element={<EditorWithParams />} />
        <Route path="/*" element={<Home />} />
      </Routes>
    </Router>
  );
};

export default App;
