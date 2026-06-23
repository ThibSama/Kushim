import { useEffect, useRef } from "react";
import {
  createBrowserRouter,
  Navigate,
  Outlet,
  useLocation,
  useNavigate,
  useParams,
} from "react-router-dom";
import { getWebsiteLoginUrl, useAuthStore } from "../stores/auth";
import { exchangeHandoffCode } from "../lib/api/authApi";
import { getBusinessMe } from "../lib/api/businessApi";
import { useRefreshTrackingStore } from "../stores/refreshTracking";
import { ServiceGate } from "./components/ServiceGate";
import { Root } from "./Root";
import { Dashboard } from "./pages/Dashboard";
import { Assets } from "./pages/Assets";
import { AssetDetail } from "./pages/AssetDetail";
import { Positions } from "./pages/Positions";
import { Transactions } from "./pages/Transactions";
import { Settings } from "./pages/Settings";

function Handoff() {
  const location = useLocation();
  const navigate = useNavigate();
  const { token, setTokens } = useAuthStore();
  const exchangingRef = useRef(false);

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const handoffCode = params.get("handoff_code");

    if (handoffCode && !exchangingRef.current) {
      exchangingRef.current = true;
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
  }, [location.search, navigate, setTokens, token]);

  return null;
}

function RequireAuth() {
  const { token, sessionStatus, validateSession } = useAuthStore();
  const validationStarted = useRef(false);

  useEffect(() => {
    if (!token) {
      window.location.href = getWebsiteLoginUrl();
      return;
    }

    if (
      sessionStatus === "idle" &&
      !validationStarted.current
    ) {
      validationStarted.current = true;
      validateSession().then((valid) => {
        if (!valid) {
          window.location.href = getWebsiteLoginUrl();
        } else {
          smokeTestBusinessApi();
          // Reload-recovery hook: if a previous tab session had an active
          // portfolio refresh request when it was reloaded, pick the polling
          // back up without creating a new request. Idempotent under Strict
          // Mode and a no-op when nothing is persisted.
          useRefreshTrackingStore.getState().resumeFromStorage();
        }
      });
    }
  }, [token, sessionStatus, validateSession]);

  if (!token) {
    return null;
  }

  if (sessionStatus === "idle" || sessionStatus === "validating") {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "60vh",
          color: "var(--text-secondary)",
          fontSize: "15px",
        }}>
        Vérification de la session…
      </div>
    );
  }

  if (sessionStatus === "unauthenticated") {
    return null;
  }

  // Session is valid: only now does the business-service gate run. It blocks the
  // protected pages when kushim-api is down and layers degraded-service banners
  // when only the worker / market-data are down — without touching the session.
  return (
    <ServiceGate>
      <Outlet />
    </ServiceGate>
  );
}

function smokeTestBusinessApi() {
  const token = useAuthStore.getState().token;
  if (!token) return;

  getBusinessMe(token).then(
    (data) => {
      console.info("[kushim-app] GET /v1/me success:", data);
    },
    (err) => {
      console.warn("[kushim-app] GET /v1/me failed:", err);
    },
  );
}

function FrenchAssetRedirect() {
  const { id } = useParams();
  return <Navigate to={id ? `/assets/${id}` : "/assets"} replace />;
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
          { path: "assets", Component: Assets },
          { path: "assets/:id", Component: AssetDetail },
          { path: "positions", Component: Positions },
          { path: "transactions", Component: Transactions },
          { path: "parametres", Component: Settings },
          { path: "actifs", Component: FrenchAssetRedirect },
          { path: "actifs/:id", Component: FrenchAssetRedirect },
          { path: "settings", element: <Navigate to="/parametres" replace /> },
        ],
      },
      { path: "*", element: <Navigate to="/dashboard" replace /> },
    ],
  },
]);
