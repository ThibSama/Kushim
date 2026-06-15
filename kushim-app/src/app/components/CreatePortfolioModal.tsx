import React, { useState } from "react";
import { AlertCircle } from "lucide-react";
import { Card } from "./Card";
import { Button } from "./Button";
import { Input } from "./Input";
import { CurrencySelect } from "./CurrencySelect";
import { usePortfolioStore } from "../../stores/portfolio";
import { type CreatePortfolioPayload } from "../../lib/api/businessApi";
import { ApiRequestError } from "../../lib/api/httpClient";

function mapBackendErrorToFrench(code: string, fallback: string): string {
  switch (code) {
    case "unsupported_currency":
      return "Cette devise n'est pas prise en charge.";
    default:
      return `${code}: ${fallback}`;
  }
}

type Props = { onClose: () => void };

export function CreatePortfolioModal({ onClose }: Props) {
  const { createPortfolio } = usePortfolioStore();
  const [name, setName] = useState("");
  const [baseCurrency, setBaseCurrency] = useState("EUR");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    setError(null);
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Le nom du portefeuille est requis.");
      return;
    }
    if (trimmed.length > 50) {
      setError("Le nom ne peut pas dépasser 50 caractères.");
      return;
    }
    const currency = baseCurrency.trim().toUpperCase();
    // The CurrencySelect component only yields canonical catalogue codes,
    // so the format check below is defensive. The backend remains
    // authoritative on the catalogue.
    if (!/^[A-Z]{3}$/.test(currency)) {
      setError("Veuillez sélectionner une devise valide.");
      return;
    }

    setSubmitting(true);
    try {
      const payload: CreatePortfolioPayload = {
        name: trimmed,
        base_currency: currency,
        visibility: "private",
      };
      await createPortfolio(payload);
      onClose();
    } catch (e) {
      if (e instanceof ApiRequestError) {
        setError(mapBackendErrorToFrench(e.code, e.message));
      } else {
        setError("Erreur inattendue lors de la création.");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <>
      <div
        className="fixed inset-0 z-40"
        style={{
          background: "rgba(0, 0, 0, 0.40)",
          backdropFilter: "blur(4px)",
          WebkitBackdropFilter: "blur(4px)",
        }}
        onClick={onClose}
      />
      <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
        <Card level={3} className="w-full max-w-[480px]">
          <h2
            className="mb-2"
            style={{
              fontSize: "18px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Créer un portefeuille
          </h2>
          <p
            style={{
              fontSize: "14px",
              color: "var(--text-secondary)",
              marginBottom: "20px",
            }}>
            Un portefeuille regroupe vos transactions et actifs pour suivre vos performances.
          </p>
          <div className="flex flex-col gap-4">
            <Input
              label="Nom"
              placeholder="Mon portefeuille principal"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={50}
            />
            <CurrencySelect
              id="portfolio-base-currency"
              label="Devise de base"
              value={baseCurrency}
              onChange={setBaseCurrency}
            />
          </div>
          {error && (
            <div
              className="flex items-start gap-2 mt-4"
              style={{
                fontSize: "13px",
                color: "var(--color-loss)",
              }}>
              <AlertCircle size={16} style={{ flexShrink: 0, marginTop: "1px" }} />
              <span>{error}</span>
            </div>
          )}
          <div className="flex gap-3 justify-end mt-6">
            <Button variant="ghost" onClick={onClose} disabled={submitting}>
              Annuler
            </Button>
            <Button variant="primary" onClick={handleSubmit} disabled={submitting}>
              {submitting ? "Création…" : "Créer"}
            </Button>
          </div>
        </Card>
      </div>
    </>
  );
}
