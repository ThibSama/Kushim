import React from "react";
import { User, Info } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Card } from "../components/Card";
import { Input } from "../components/Input";
import { Button } from "../components/Button";
import { getWebsiteLoginUrl, useAuthStore } from "../../stores/auth";

function SectionTitle({
  icon: Icon,
  title,
  helper,
}: {
  icon: React.ElementType;
  title: string;
  helper?: string;
}) {
  return (
    <div className="flex items-start gap-3">
      <div
        className="glass-field flex items-center justify-center rounded-full"
        style={{
          width: "36px",
          height: "36px",
          color: "var(--color-accent)",
          flexShrink: 0,
        }}>
        <Icon size={17} />
      </div>
      <div>
        <h2
          style={{
            fontSize: "18px",
            fontWeight: 700,
            color: "var(--text-primary)",
          }}>
          {title}
        </h2>
        {helper ? (
          <p
            style={{
              marginTop: "5px",
              fontSize: "13px",
              lineHeight: 1.5,
              color: "var(--text-secondary)",
            }}>
            {helper}
          </p>
        ) : null}
      </div>
    </div>
  );
}

export function Settings() {
  const navigate = useNavigate();
  const logout = useAuthStore((state) => state.logout);
  const user = useAuthStore((state) => state.user);

  const displayName = user?.username ?? "Utilisateur";
  const displayHandle = user?.public_handle ?? user?.username ?? "—";
  const displayInitial = displayName.charAt(0).toUpperCase();

  const handleLogout = async () => {
    await logout();
    window.location.href = getWebsiteLoginUrl();
  };

  return (
    <div className="app-page-container max-w-[1180px] mx-auto px-4 sm:px-6 py-12">
      <div className="mb-8">
        <h1
          style={{
            fontSize: "clamp(26px, 5vw, 34px)",
            fontWeight: 700,
            color: "var(--text-primary)",
            letterSpacing: "-0.01em",
          }}>
          Paramètres
        </h1>
        <p
          style={{
            marginTop: "8px",
            maxWidth: "640px",
            fontSize: "clamp(14px, 2.5vw, 15px)",
            lineHeight: 1.6,
            color: "var(--text-secondary)",
          }}>
          Consultez votre profil et déconnectez-vous de votre session Kushim.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[340px_1fr] gap-6 lg:gap-8 items-start">
        <div className="space-y-4">
          <Card level={1} className="overflow-hidden">
            <div className="flex flex-col items-center text-center">
              <div
                className="flex items-center justify-center rounded-full"
                style={{
                  width: "84px",
                  height: "84px",
                  background:
                    "linear-gradient(135deg, var(--color-cta-bg), var(--color-accent))",
                  color: "var(--color-cta-text)",
                  fontSize: "30px",
                  fontWeight: 800,
                  boxShadow: "0 18px 45px rgba(16, 185, 129, 0.22)",
                }}>
                {displayInitial}
              </div>
              <div
                style={{
                  marginTop: "18px",
                  fontSize: "20px",
                  fontWeight: 700,
                  color: "var(--text-primary)",
                }}>
                {displayName}
              </div>
              <div
                style={{
                  marginTop: "6px",
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: "13px",
                  color: "var(--text-tertiary)",
                }}>
                {displayHandle}
              </div>
              <div
                className="mt-4 rounded-full"
                style={{
                  padding: "6px 12px",
                  border: "1px solid rgba(16, 185, 129, 0.28)",
                  background: "rgba(16, 185, 129, 0.10)",
                  color: "var(--color-gain)",
                  fontSize: "12px",
                  fontWeight: 700,
                }}>
                {user?.role ?? "user"}
              </div>
            </div>

            <p
              style={{
                marginTop: "24px",
                fontSize: "12px",
                lineHeight: 1.5,
                color: "var(--text-tertiary)",
                textAlign: "center",
              }}>
              {(() => {
                if (!user?.created_at) return " ";
                const d = new Date(user.created_at);
                if (Number.isNaN(d.getTime())) return "Non disponible";
                return `Compte créé le ${d.toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" })}`;
              })()}
            </p>
          </Card>
        </div>

        <div className="space-y-6">
          <Card level={1}>
            <SectionTitle
              icon={User}
              title="Profil"
              helper="Informations renvoyées par le service d'authentification Kushim."
            />
            <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input label="Nom d'utilisateur" value={displayName} readOnly />
              <Input label="Identifiant public" value={displayHandle} readOnly />
            </div>
            <div className="mt-6 flex flex-col sm:flex-row justify-end gap-3">
              <Button variant="secondary" onClick={() => navigate("/dashboard")}>
                Retour au tableau de bord
              </Button>
              <Button variant="danger" onClick={handleLogout}>
                Se déconnecter
              </Button>
            </div>
          </Card>

          <Card level={1}>
            <SectionTitle
              icon={Info}
              title="Autres actions"
              helper="La gestion du mot de passe, des préférences et la suppression de compte ne sont pas exposées dans cette version. Ces actions seront traitées par le frontend d'authentification dédié."
            />
          </Card>
        </div>
      </div>
    </div>
  );
}
