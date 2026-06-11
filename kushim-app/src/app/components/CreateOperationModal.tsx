import React, { useEffect, useState } from "react";
import { AlertCircle } from "lucide-react";
import { Card } from "./Card";
import { Button } from "./Button";
import { Input } from "./Input";
import { AssetSearchSelect } from "./AssetSearchSelect";
import { usePortfolioStore } from "../../stores/portfolio";
import { useOperationsStore } from "../../stores/operations";
import { operationTypeLabel, CASH_OPERATION_TYPES, cacheAssetDisplay } from "../../lib/operations";
import type { Asset, CreateOperationPayload } from "../../lib/api/businessApi";
import { ApiRequestError } from "../../lib/api/httpClient";

const ASSET_OPERATION_TYPES = ["buy", "sell", "dividend"];

const SUPPORTED_TYPES = [
  ...CASH_OPERATION_TYPES,
  ...ASSET_OPERATION_TYPES,
];

function isAssetType(type: string): boolean {
  return ASSET_OPERATION_TYPES.includes(type);
}

function needsQuantity(type: string): boolean {
  return type === "buy" || type === "sell";
}

function needsPrice(type: string): boolean {
  return type === "buy" || type === "sell";
}

type Props = {
  portfolioId: string;
  onClose: () => void;
};

export function CreateOperationModal({ portfolioId, onClose }: Props) {
  const { createOperation, operationTypes, loadReferenceData } =
    useOperationsStore();
  const portfolio = usePortfolioStore(
    (s) => s.portfolios.find((p) => p.id_portfolio === portfolioId) ?? null,
  );

  const [opType, setOpType] = useState("deposit");
  const [executedAt, setExecutedAt] = useState(
    new Date().toISOString().slice(0, 16),
  );
  const [currency, setCurrency] = useState(portfolio?.base_currency ?? "EUR");
  const [selectedAsset, setSelectedAsset] = useState<Asset | null>(null);
  const [quantity, setQuantity] = useState("");
  const [price, setPrice] = useState("");
  const [grossAmount, setGrossAmount] = useState("");
  const [cashAmount, setCashAmount] = useState("");
  const [feeAmount, setFeeAmount] = useState("");
  const [taxAmount, setTaxAmount] = useState("");
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadReferenceData();
  }, [loadReferenceData]);

  const handleTypeChange = (newType: string) => {
    setOpType(newType);
    if (!isAssetType(newType)) {
      setSelectedAsset(null);
      setQuantity("");
      setPrice("");
    }
  };

  const autoGross = (() => {
    if (needsPrice(opType) && quantity && price) {
      const q = parseFloat(quantity);
      const p = parseFloat(price);
      if (!Number.isNaN(q) && !Number.isNaN(p) && q > 0 && p > 0) {
        return (q * p).toFixed(2);
      }
    }
    return null;
  })();

  const effectiveGross = grossAmount || autoGross || "";

  const supportedRefTypes = operationTypes.filter((t) =>
    SUPPORTED_TYPES.includes(t.value),
  );
  const typeItems =
    supportedRefTypes.length > 0
      ? supportedRefTypes
      : SUPPORTED_TYPES.map((v) => ({ value: v, label: operationTypeLabel(v) }));

  const handleSubmit = async () => {
    setError(null);

    if (isAssetType(opType) && !selectedAsset) {
      setError("Veuillez sélectionner un actif.");
      return;
    }
    if (needsQuantity(opType) && (!quantity || parseFloat(quantity) <= 0)) {
      setError("La quantité doit être supérieure à zéro.");
      return;
    }
    if (needsPrice(opType) && (!price || parseFloat(price) <= 0)) {
      setError("Le prix doit être supérieur à zéro.");
      return;
    }

    const gross = Math.round(parseFloat(effectiveGross || "0") * 100);
    const cash = cashAmount
      ? Math.round(parseFloat(cashAmount) * 100)
      : gross;
    const fees = feeAmount
      ? Math.round(parseFloat(feeAmount) * 100)
      : undefined;
    const taxes = taxAmount
      ? Math.round(parseFloat(taxAmount) * 100)
      : undefined;
    const priceMinor = price
      ? Math.round(parseFloat(price) * 100)
      : undefined;

    if (gross <= 0 && !["transfer_in", "transfer_out"].includes(opType)) {
      setError("Le montant brut doit être supérieur à zéro.");
      return;
    }

    const payload: CreateOperationPayload = {
      operation_type: opType,
      executed_at: new Date(executedAt).toISOString(),
      currency: currency.toUpperCase(),
      gross_amount_minor: gross > 0 ? gross : undefined,
      cash_amount_minor: cash > 0 ? cash : undefined,
      fees_minor: fees,
      taxes_minor: taxes,
      notes: notes.trim() || undefined,
    };

    if (isAssetType(opType) && selectedAsset) {
      payload.id_asset = selectedAsset.id_asset;
    }
    if (needsQuantity(opType) && quantity) {
      payload.quantity = quantity;
    }
    if (needsPrice(opType) && priceMinor != null && priceMinor > 0) {
      payload.price_minor = priceMinor;
    }

    if (selectedAsset) {
      cacheAssetDisplay(selectedAsset);
    }

    setSubmitting(true);
    try {
      await createOperation(portfolioId, payload);
      onClose();
    } catch (e) {
      if (e instanceof ApiRequestError) {
        setError(`${e.code}: ${e.message}`);
      } else {
        setError("Erreur inattendue lors de la création.");
      }
    } finally {
      setSubmitting(false);
    }
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const showAssetSelector = isAssetType(opType);
  const showQuantity = needsQuantity(opType);
  const showPrice = needsPrice(opType);

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
      <div className="fixed inset-0 z-50 flex items-center justify-center p-6" style={{ overflowY: "auto" }}>
        <Card level={3} className="w-full max-w-[560px]">
          <h2
            className="mb-2"
            style={{ fontSize: "18px", fontWeight: 600, color: "var(--text-primary)" }}>
            Nouvelle opération
          </h2>
          <p style={{ fontSize: "14px", color: "var(--text-secondary)", marginBottom: "20px" }}>
            {isAssetType(opType)
              ? "Enregistrez un achat, une vente ou un dividende lié à un actif."
              : "Enregistrez un dépôt, retrait, frais, taxe ou mouvement de trésorerie."}
          </p>

          <div className="flex flex-col gap-4">
            {/* Type */}
            <div className="w-full">
              <label className="block mb-1.5" style={{ fontSize: "12px", fontWeight: 500, color: "var(--text-secondary)" }}>
                Type d'opération
              </label>
              <select
                value={opType}
                onChange={(e) => handleTypeChange(e.target.value)}
                className="glass-field w-full px-5 py-3 rounded-[9999px]"
                style={{
                  border: "1px solid var(--surface-2-border)",
                  fontSize: "15px",
                  color: "var(--text-primary)",
                  background: "var(--surface-1-bg)",
                  cursor: "pointer",
                }}>
                <optgroup label="Opérations sur actifs">
                  {typeItems
                    .filter((t) => ASSET_OPERATION_TYPES.includes(t.value))
                    .map((t) => (
                      <option key={t.value} value={t.value}>{t.label}</option>
                    ))}
                </optgroup>
                <optgroup label="Trésorerie">
                  {typeItems
                    .filter((t) => CASH_OPERATION_TYPES.includes(t.value))
                    .map((t) => (
                      <option key={t.value} value={t.value}>{t.label}</option>
                    ))}
                </optgroup>
              </select>
            </div>

            {/* Asset selector */}
            {showAssetSelector && (
              <AssetSearchSelect
                selectedAsset={selectedAsset}
                onSelect={setSelectedAsset}
                label="Actif"
                error={undefined}
              />
            )}

            {/* Date + Currency */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input
                label="Date/heure"
                type="datetime-local"
                value={executedAt}
                onChange={(e) => setExecutedAt(e.target.value)}
              />
              <Input
                label="Devise"
                value={currency}
                onChange={(e) => setCurrency(e.target.value.toUpperCase())}
                maxLength={3}
                placeholder="EUR"
              />
            </div>

            {/* Quantity + Price for buy/sell */}
            {showQuantity && (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <Input
                  label="Quantité"
                  type="number"
                  step="any"
                  min="0"
                  value={quantity}
                  onChange={(e) => setQuantity(e.target.value)}
                  placeholder="10"
                />
                {showPrice && (
                  <Input
                    label="Prix unitaire"
                    type="number"
                    step="0.01"
                    min="0"
                    value={price}
                    onChange={(e) => setPrice(e.target.value)}
                    placeholder="150.00"
                    helperText="En devise majeure"
                  />
                )}
              </div>
            )}

            {/* Gross + Cash amounts */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input
                label="Montant brut"
                type="number"
                step="0.01"
                min="0"
                value={grossAmount || (autoGross ?? "")}
                onChange={(e) => setGrossAmount(e.target.value)}
                placeholder={autoGross ?? "100.00"}
                helperText={autoGross && !grossAmount ? "Auto-calculé (quantité × prix)" : undefined}
              />
              <Input
                label="Montant net (optionnel)"
                type="number"
                step="0.01"
                min="0"
                value={cashAmount}
                onChange={(e) => setCashAmount(e.target.value)}
                placeholder="= montant brut"
                helperText="Si différent du montant brut"
              />
            </div>

            {/* Fees + Taxes */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <Input
                label="Frais (optionnel)"
                type="number"
                step="0.01"
                min="0"
                value={feeAmount}
                onChange={(e) => setFeeAmount(e.target.value)}
                placeholder="0.00"
              />
              <Input
                label="Taxes (optionnel)"
                type="number"
                step="0.01"
                min="0"
                value={taxAmount}
                onChange={(e) => setTaxAmount(e.target.value)}
                placeholder="0.00"
              />
            </div>

            {/* Notes */}
            <Input
              label="Note (optionnel)"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Motif de l'opération"
            />
          </div>

          {error && (
            <div
              className="flex items-start gap-2 mt-4"
              style={{ fontSize: "13px", color: "var(--color-loss)" }}>
              <AlertCircle size={16} style={{ flexShrink: 0, marginTop: "1px" }} />
              <span>{error}</span>
            </div>
          )}

          <div className="flex gap-3 justify-end mt-6">
            <Button variant="ghost" onClick={onClose} disabled={submitting}>
              Annuler
            </Button>
            <Button variant="primary" onClick={handleSubmit} disabled={submitting}>
              {submitting ? "Création…" : "Enregistrer"}
            </Button>
          </div>
        </Card>
      </div>
    </>
  );
}
