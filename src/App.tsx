import React, { useEffect } from "react";
import "./styles.css";
import {
  BrowserRouter as Router,
  Routes,
  Route,
  useParams,
  useLocation,
  Navigate,
  useNavigate,
} from "react-router-dom";
import { logEvent } from "firebase/analytics";
import Editor from "./components/Editor";
import Home from "./components/Home";
import { analytics } from "./firebase";
import EditionViewer from "./components/EditionViewer";
import { getArticleRecord, loadArticleEditions } from "./utils/articleEditions";

const DraftRoute: React.FC = () => {
  const { articleId } = useParams<{ articleId: string }>();
  const isEditable = import.meta.env.VITE_IS_FACETS !== "true";
  const navigate = useNavigate();

  useEffect(() => {
    if (!isEditable && articleId) {
      const publishState = loadArticleEditions();
      const record = getArticleRecord(publishState, articleId);
      if (record?.latestVersion) {
        navigate(`/a/${articleId}/v/${record.latestVersion}`, {
          replace: true,
        });
      }
    }
  }, [articleId, isEditable, navigate]);

  if (!articleId) {
    return null;
  }

  if (!isEditable) {
    return (
      <div className="editor-container">
        <div className="editor-placeholder">No published editions yet.</div>
      </div>
    );
  }

  return <Editor key={articleId} articleId={articleId} isEditable />;
};

const EditionRoute: React.FC = () => {
  const { articleId, version } = useParams<{
    articleId: string;
    version: string;
  }>();
  const parsedVersion = Number(version);

  if (!articleId || !Number.isFinite(parsedVersion)) {
    return (
      <div className="editor-container">
        <div className="editor-placeholder">No edition found at the link</div>
      </div>
    );
  }

  return <EditionViewer articleId={articleId} version={parsedVersion} />;
};

const LegacyArticleRoute: React.FC = () => {
  const { id } = useParams<{ id: string }>();

  if (!id) {
    return null;
  }

  return <Navigate to={`/a/${id}`} replace />;
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
        <Route path="/a/:articleId/v/:version" element={<EditionRoute />} />
        <Route path="/a/:articleId" element={<DraftRoute />} />
        <Route path="/article/:id" element={<LegacyArticleRoute />} />
        <Route path="/*" element={<Home />} />
      </Routes>
    </Router>
  );
};

export default App;
