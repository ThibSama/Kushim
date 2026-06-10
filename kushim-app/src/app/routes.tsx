import { useEffect, useState } from "react";
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

const AUTH_API_URL =
  import.meta.env.VITE_AUTH_API_URL || "http://localhost:3002";

async function exchangeHandoffCode(
  code: string,
): Promise<{ access_token: string; refresh_token: string } | null> {
  try {
    const response = await fetch(`${AUTH_API_URL}/auth/handoff/exchange`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ handoff_code: code }),
    });
    if (!response.ok) return null;
    return await response.json();
  } catch {
    return null;
  }
}

function Handoff() {
  const location = useLocation();
  const navigate = useNavigate();
  const { token, setTokens } = useAuthStore();
  const [exchanging, setExchanging] = useState(false);

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const handoffCode = params.get("handoff_code");

    if (handoffCode && !exchanging) {
      setExchanging(true);
      exchangeHandoffCode(handoffCode).then((result) => {
        if (result) {
          setTokens(result.access_token, result.refresh_token);
          navigate("/dashboard", { replace: true });
        } else {
          window.location.href = getWebsiteLoginUrl();
        }
      });
      return;
    }

    if (token) {
      navigate("/dashboard", { replace: true });
      return;
    }

    window.location.href = getWebsiteLoginUrl();
  }, [location.search, navigate, setTokens, token, exchanging]);

  return null;
}

function RequireAuth() {
  const token = useAuthStore((state) => state.token);

  useEffect(() => {
    if (!token) {
      window.location.href = getWebsiteLoginUrl();
    }
  }, [token]);

  if (!token) {
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
