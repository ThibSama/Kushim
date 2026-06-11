import React from "react";
import { Shield, User, SlidersHorizontal, TriangleAlert } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { Card } from "../components/Card";
import { Input } from "../components/Input";
import { Button } from "../components/Button";
import { getWebsiteLoginUrl, useAuthStore } from "../../stores/auth";

const fieldStyle: React.CSSProperties = {
  width: "100%",
  minHeight: "46px",
  padding: "0 18px",
  borderRadius: "9999px",
  border: "1px solid var(--input-border)",
  background: "var(--input-bg)",
  color: "var(--text-primary)",
  fontSize: "15px",
  outline: "none",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  marginBottom: "8px",
  fontSize: "12px",
  fontWeight: 600,
  color: "var(--text-secondary)",
};

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

function SelectField({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label style={labelStyle}>{label}</label>
      <select style={fieldStyle}>{children}</select>
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
          Gérez votre profil, vos préférences et la sécurité de votre compte.
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

            <div
              style={{
                height: "1px",
                margin: "24px 0",
                background: "var(--surface-1-border)",
              }}
            />

            <div className="space-y-3">
              {["Profil", "Préférences", "Sécurité", "Danger"].map((item) => (
                <div
                  key={item}
                  className="rounded-full"
                  style={{
                    padding: "10px 14px",
                    background:
                      item === "Profil" ? "var(--surface-2-bg)" : "transparent",
                    color:
                      item === "Profil"
                        ? "var(--text-primary)"
                        : "var(--text-secondary)",
                    border:
                      item === "Profil"
                        ? "1px solid var(--surface-2-border)"
                        : "1px solid transparent",
                    fontSize: "13px",
                    fontWeight: 600,
                  }}>
                  {item}
                </div>
              ))}
            </div>

            <p
              style={{
                marginTop: "24px",
                fontSize: "12px",
                lineHeight: 1.5,
                color: "var(--text-tertiary)",
              }}>
              {user?.created_at
                ? `Compte créé le ${new Date(user.created_at).toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" })}`
                : " "}
            </p>
          </Card>
        </div>

        <div className="space-y-6">
          <Card level={1}>
            <SectionTitle
              icon={User}
              title="Profil"
              helper="Les informations de base de votre espace Kushim."
            />
            <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input label="Nom d'utilisateur" value={displayName} readOnly />
              <Input label="Identifiant" value={displayHandle} readOnly />
            </div>
          </Card>

          <Card level={1}>
            <SectionTitle
              icon={SlidersHorizontal}
              title="Préférences"
              helper="Personnalisez l'affichage et les formats utilisés dans votre portefeuille."
            />
            <div className="mt-6 grid grid-cols-1 md:grid-cols-3 gap-4">
              <SelectField label="Devise de base">
                <option>EUR (€)</option>
                <option>USD ($)</option>
                <option>GBP (£)</option>
                <option>CHF (Fr)</option>
              </SelectField>
              <SelectField label="Thème">
                <option>Clair</option>
                <option>Sombre</option>
                <option>Système</option>
              </SelectField>
              <SelectField label="Langue">
                <option>Français</option>
              </SelectField>
            </div>
            <div className="mt-6 flex justify-end">
              <Button variant="primary">Enregistrer les préférences</Button>
            </div>
          </Card>

          <Card level={1}>
            <SectionTitle
              icon={Shield}
              title="Sécurité"
              helper="Modifiez votre mot de passe régulièrement pour protéger votre compte."
            />
            <div className="mt-6 grid grid-cols-1 gap-4">
              <Input
                label="Mot de passe actuel"
                type="password"
                placeholder="Entrez votre mot de passe actuel"
              />
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <Input
                  label="Nouveau mot de passe"
                  type="password"
                  placeholder="Entrez un nouveau mot de passe"
                />
                <Input
                  label="Confirmer le nouveau mot de passe"
                  type="password"
                  placeholder="Confirmez le nouveau mot de passe"
                />
              </div>
            </div>
            <div className="mt-6 flex justify-end">
              <Button variant="primary">Mettre à jour le mot de passe</Button>
            </div>
          </Card>

          <Card
            level={1}
            style={{
              borderColor: "rgba(239, 68, 68, 0.28)",
              boxShadow: "0 18px 60px rgba(239, 68, 68, 0.08)",
            }}>
            <SectionTitle
              icon={TriangleAlert}
              title="Zone dangereuse"
              helper="La suppression de votre compte est irréversible."
            />
            <p
              style={{
                marginTop: "18px",
                fontSize: "14px",
                lineHeight: 1.6,
                color: "var(--text-secondary)",
              }}>
              Toutes vos données seront définitivement effacées. Cette action ne
              peut pas être annulée.
            </p>
            <div className="mt-6 flex flex-col sm:flex-row justify-end gap-3">
              <Button variant="secondary" onClick={() => navigate("/dashboard")}>
                Retour au tableau de bord
              </Button>
              <Button variant="danger" onClick={handleLogout}>
                Se déconnecter
              </Button>
              <Button variant="danger">Supprimer mon compte</Button>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
