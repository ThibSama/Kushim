import { useEffect } from "react";
import {
  createBrowserRouter,
  Navigate,
  Outlet,
  useLocation,
  useNavigate,
  useParams,
} from "react-router-dom";
import { getWebsiteLoginUrl, useAuthStore } from "../stores/auth";
import { Root } from "./Root";
import { Dashboard } from "./pages/Dashboard";
import { Assets } from "./pages/Assets";
import { AssetDetail } from "./pages/AssetDetail";
import { Transactions } from "./pages/Transactions";
import { Settings } from "./pages/Settings";

function Handoff() {
  const location = useLocation();
  const navigate = useNavigate();
  const { token, setToken } = useAuthStore();

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const nextToken = params.get("token");

    if (nextToken) {
      setToken(nextToken);
      navigate("/dashboard", { replace: true });
      return;
    }

    if (token || localStorage.getItem("kushim_token")) {
      navigate("/dashboard", { replace: true });
      return;
    }

    window.location.href = getWebsiteLoginUrl();
  }, [location.search, navigate, setToken, token]);

  return null;
}

function RequireAuth() {
  const token = useAuthStore((state) => state.token);

  useEffect(() => {
    if (!token && !localStorage.getItem("kushim_token")) {
      window.location.href = getWebsiteLoginUrl();
    }
  }, [token]);

  if (!token && !localStorage.getItem("kushim_token")) {
    return null;
  }

  return <Outlet />;
}

function LegacyAssetRedirect() {
  const { id } = useParams();
  return <Navigate to={id ? `/actifs/${id}` : "/actifs"} replace />;
}

export const router = createBrowserRouter([
  {
    path: "/",
    Component: Root,
    children: [
      { index: true, Component: Handoff },
      {
        Component: RequireAuth,
        children: [
          { path: "dashboard", Component: Dashboard },
          { path: "actifs", Component: Assets },
          { path: "actifs/:id", Component: AssetDetail },
          { path: "transactions", Component: Transactions },
          { path: "parametres", Component: Settings },
          { path: "assets", Component: LegacyAssetRedirect },
          { path: "assets/:id", Component: LegacyAssetRedirect },
          { path: "settings", element: <Navigate to="/parametres" replace /> },
        ],
      },
      { path: "*", element: <Navigate to="/dashboard" replace /> },
    ],
  },
]);
