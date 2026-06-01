import { useState } from "react";
import { ArrowDownUp, Check, X } from "lucide-react";
import { Button } from "./Button";
import { Card } from "./Card";

export function SwapModal({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const [fromAsset, setFromAsset] = useState("EUR");
  const [toAsset, setToAsset] = useState("BTC");
  const [amount, setAmount] = useState("");
  const [step, setStep] = useState<"input" | "confirm" | "success">("input");

  if (!isOpen) return null;

  const assets = ["EUR", "USD", "BTC", "ETH", "AAPL"];

  const close = () => {
    setStep("input");
    setAmount("");
    onClose();
  };

  const confirmSwap = () => {
    setStep("success");
    window.setTimeout(close, 900);
  };

  return (
    <>
      <div className="fixed inset-0 z-40 glass-overlay" onClick={close} />
      <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
        <Card level={3} className="w-full max-w-[425px]">
          <div className="mb-6 flex items-start justify-between gap-4">
            <div>
              <h2 style={{ fontSize: "20px", fontWeight: 700, color: "var(--text-primary)" }}>
                Echanger des actifs
              </h2>
              <p className="mt-1" style={{ fontSize: "14px", color: "var(--text-secondary)" }}>
                Simulation locale sans execution de marche.
              </p>
            </div>
            <button
              type="button"
              onClick={close}
              className="rounded-full p-2"
              style={{ color: "var(--text-secondary)" }}
              aria-label="Fermer"
            >
              <X size={18} />
            </button>
          </div>

          {step === "input" && (
            <div className="flex flex-col gap-5">
              <AssetRow
                label="Payer avec"
                amount={amount}
                onAmountChange={setAmount}
                asset={fromAsset}
                onAssetChange={setFromAsset}
                assets={assets}
              />
              <div className="flex justify-center">
                <div className="glass-field rounded-full p-3">
                  <ArrowDownUp size={18} style={{ color: "var(--text-secondary)" }} />
                </div>
              </div>
              <AssetRow
                label="Recevoir"
                amount={amount ? (Number(amount) * 0.000014).toFixed(6) : ""}
                onAmountChange={() => undefined}
                asset={toAsset}
                onAssetChange={setToAsset}
                assets={assets}
                readOnly
              />
              <Button
                variant="primary"
                onClick={() => setStep("confirm")}
                disabled={!amount || Number(amount) <= 0}
              >
                Continuer
              </Button>
            </div>
          )}

          {step === "confirm" && (
            <div className="space-y-5">
              <div className="glass-field rounded-[var(--radius-lg)] p-4">
                <p style={{ color: "var(--text-secondary)", fontSize: "14px" }}>Vous echangez</p>
                <p className="mt-1" style={{ color: "var(--text-primary)", fontSize: "20px", fontWeight: 700 }}>
                  {amount} {fromAsset} vers {toAsset}
                </p>
              </div>
              <div className="flex gap-3">
                <Button variant="secondary" className="flex-1" onClick={() => setStep("input")}>
                  Retour
                </Button>
                <Button variant="primary" className="flex-1" onClick={confirmSwap}>
                  Confirmer
                </Button>
              </div>
            </div>
          )}

          {step === "success" && (
            <div className="py-8 text-center">
              <Check className="mx-auto mb-4" size={42} style={{ color: "var(--color-gain)" }} />
              <p style={{ color: "var(--text-primary)", fontSize: "18px", fontWeight: 700 }}>
                Echange simule
              </p>
            </div>
          )}
        </Card>
      </div>
    </>
  );
}

function AssetRow({
  label,
  amount,
  onAmountChange,
  asset,
  onAssetChange,
  assets,
  readOnly = false,
}: {
  label: string;
  amount: string;
  onAmountChange: (value: string) => void;
  asset: string;
  onAssetChange: (value: string) => void;
  assets: string[];
  readOnly?: boolean;
}) {
  return (
    <label className="block">
      <span style={{ fontSize: "13px", color: "var(--text-secondary)", fontWeight: 600 }}>
        {label}
      </span>
      <div className="glass-field mt-2 flex rounded-[var(--radius-md)] p-2">
        <input
          type="number"
          value={amount}
          readOnly={readOnly}
          placeholder="0.00"
          onChange={(event) => onAmountChange(event.target.value)}
          className="min-w-0 flex-1 bg-transparent px-3"
          style={{ color: "var(--text-primary)", outline: "none" }}
        />
        <select
          value={asset}
          onChange={(event) => onAssetChange(event.target.value)}
          className="rounded-[var(--radius-sm)] px-3"
          style={{ color: "var(--text-primary)" }}
        >
          {assets.map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
      </div>
    </label>
  );
}
