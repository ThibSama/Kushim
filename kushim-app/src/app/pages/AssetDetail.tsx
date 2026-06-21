import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Loader2, AlertCircle, ExternalLink, Plus } from "lucide-react";
import { Card } from "../components/Card";
import { Button } from "../components/Button";
import { CreateOperationModal } from "../components/CreateOperationModal";
import { RefreshNotice } from "../components/RefreshNotice";
import { useAssetsStore } from "../../stores/assets";
import { usePortfolioStore } from "../../stores/portfolio";

const ASSET_CLASS_LABELS: Record<string, string> = {
  equity: "Action",
  etf: "ETF",
  fund: "Fonds",
  bond: "Obligation",
  crypto: "Crypto",
  commodity: "Matière première",
  cash: "Cash",
  forex: "Forex",
  index: "Indice",
  real_estate: "Immobilier",
  private_equity: "Private Equity",
  derivative: "Dérivé",
  other: "Autre",
};

function assetClassLabel(cls: string): string {
  return ASSET_CLASS_LABELS[cls] ?? cls;
}

function formatPrice(minor: number, currency: string): string {
  const value = minor / 100;
  try {
    return new Intl.NumberFormat("fr-FR", {
      style: "currency",
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(value);
  } catch {
    return `${value.toFixed(2)} ${currency}`;
  }
}

function formatDateTime(iso: string): string {
  try {
    return new Intl.DateTimeFormat("fr-FR", {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(iso));
  } catch {
    return iso;
  }
}

function InfoRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string | null | undefined;
  mono?: boolean;
}) {
  if (!value) return null;
  return (
    <div
      className="flex items-center justify-between"
      style={{
        padding: "10px 0",
        borderBottom: "1px solid var(--surface-1-border)",
      }}>
      <span style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>
        {label}
      </span>
      <span
        style={{
          fontSize: "14px",
          fontWeight: 500,
          fontFamily: mono ? "'JetBrains Mono', monospace" : "inherit",
          color: "var(--text-primary)",
          textAlign: "right",
          maxWidth: "60%",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}>
        {value}
      </span>
    </div>
  );
}

export function AssetDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const {
    detailAsset: asset,
    detailStatus,
    detailError: error,
    loadAssetDetail,
  } = useAssetsStore();
  const {
    activePortfolioId,
    status: portfolioStatus,
    loadPortfolios,
  } = usePortfolioStore();
  const [showAddAsset, setShowAddAsset] = useState(false);

  useEffect(() => {
    if (id) loadAssetDetail(id);
  }, [id, loadAssetDetail]);

  // AssetDetail can be reached via a direct URL (deep link), so it must not
  // rely on Dashboard having loaded the portfolios first. Load them here when
  // the store is still idle so the active portfolio is resolved before any
  // operation modal can open. We do NOT fall back to the modal's EUR currency
  // when the portfolio is still unresolved — the action stays disabled instead.
  useEffect(() => {
    if (portfolioStatus === "idle") loadPortfolios();
  }, [portfolioStatus, loadPortfolios]);

  const portfolioReady = portfolioStatus === "success" && !!activePortfolioId;
  const noPortfolio = portfolioStatus === "success" && !activePortfolioId;
  const portfolioLoading =
    portfolioStatus === "loading" || portfolioStatus === "idle";

  const loading = detailStatus === "loading" || detailStatus === "idle";

  if (loading) {
    return (
      <div
        className="max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
        style={{ paddingTop: "clamp(100px, 15vw, 120px)" }}>
        <div
          className="flex items-center justify-center gap-3"
          style={{
            minHeight: "300px",
            color: "var(--text-tertiary)",
            fontSize: "14px",
          }}>
          <Loader2 size={20} className="animate-spin" />
          Chargement de l'actif…
        </div>
      </div>
    );
  }

  if (error || !asset) {
    return (
      <div
        className="max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
        style={{ paddingTop: "clamp(100px, 15vw, 120px)" }}>
        <button
          onClick={() => navigate("/assets")}
          className="flex items-center gap-2 mb-6"
          style={{
            background: "none",
            border: "none",
            cursor: "pointer",
            color: "var(--text-secondary)",
            fontSize: "14px",
            padding: 0,
          }}>
          <ArrowLeft size={18} />
          Retour au catalogue
        </button>
        <Card level={1}>
          <div
            className="flex items-center gap-3"
            style={{ color: "var(--color-loss)" }}>
            <AlertCircle size={20} />
            <div>
              <div style={{ fontWeight: 600, fontSize: "14px" }}>
                Cet actif est introuvable ou inaccessible
              </div>
              {error && (
                <div
                  style={{
                    fontSize: "13px",
                    color: "var(--text-secondary)",
                    marginTop: "2px",
                  }}>
                  {error}
                </div>
              )}
            </div>
          </div>
        </Card>
      </div>
    );
  }

  const md = asset.market_data;
  const meta = asset.metadata;

  return (
    <div
      className="max-w-[1200px] mx-auto px-4 sm:px-6 py-12"
      style={{ paddingTop: "clamp(100px, 15vw, 120px)" }}>
      {/* Header */}
      <div className="flex flex-wrap items-start justify-between gap-4 mb-8">
        <div>
          <div className="flex items-center gap-3 mb-2">
            <button
              onClick={() => navigate("/assets")}
              className="p-1 transition-colors"
              style={{
                background: "none",
                border: "none",
                cursor: "pointer",
                color: "var(--text-secondary)",
                minWidth: "44px",
                minHeight: "44px",
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = "var(--text-primary)";
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = "var(--text-secondary)";
              }}>
              <ArrowLeft size={20} />
            </button>
            <h1
              style={{
                fontSize: "clamp(20px, 4vw, 24px)",
                fontWeight: 700,
                color: "var(--text-primary)",
              }}>
              {asset.name}
            </h1>
            {(asset.ticker ?? asset.symbol) && (
              <span
                style={{
                  fontSize: "12px",
                  fontWeight: 500,
                  color: "var(--text-secondary)",
                  background: "var(--surface-2-bg)",
                  border: "1px solid var(--surface-2-border)",
                  borderRadius: "var(--radius-md)",
                  padding: "4px 8px",
                }}>
                {asset.ticker ?? asset.symbol}
              </span>
            )}
            <span
              className="rounded-full px-2 py-0.5"
              style={{
                fontSize: "11px",
                fontWeight: 600,
                background:
                  asset.status === "active"
                    ? "color-mix(in srgb, var(--color-gain) 12%, transparent)"
                    : "var(--surface-2-bg)",
                color:
                  asset.status === "active"
                    ? "var(--color-gain)"
                    : "var(--text-tertiary)",
              }}>
              {asset.status === "active"
                ? "Actif"
                : asset.status === "inactive"
                  ? "Inactif"
                  : asset.status === "delisted"
                    ? "Délisté"
                    : asset.status}
            </span>
          </div>

          {/* Current price */}
          {md && (
            <div className="flex items-center gap-3 ml-9">
              <span
                style={{
                  fontSize: "18px",
                  fontWeight: 600,
                  color: "var(--text-primary)",
                  fontFamily: "'JetBrains Mono', monospace",
                }}>
                {formatPrice(md.price_minor, md.currency)}
              </span>
              {md.change_24h_pct != null && (
                <span
                  style={{
                    fontSize: "14px",
                    fontWeight: 500,
                    color: md.change_24h_pct.startsWith("-")
                      ? "var(--color-loss)"
                      : "var(--color-gain)",
                  }}>
                  {md.change_24h_pct.startsWith("-") ? "" : "+"}
                  {md.change_24h_pct}% (24h)
                </span>
              )}
            </div>
          )}
        </div>

        {/* "Ajouter au portefeuille" — records a posted buy operation against
            the active portfolio; does not create or modify an asset row. The
            action is disabled (not silently broken) while the portfolio is
            loading, missing, or failed to load. */}
        <div className="flex flex-col items-end gap-1.5">
          {portfolioStatus === "error" ? (
            <span
              style={{
                fontSize: "13px",
                color: "var(--color-loss)",
                textAlign: "right",
              }}>
              Portefeuille indisponible
            </span>
          ) : portfolioLoading ? (
            <Button variant="secondary" icon={Plus} disabled>
              Chargement…
            </Button>
          ) : noPortfolio ? (
            <>
              <Button variant="secondary" icon={Plus} disabled>
                Ajouter au portefeuille
              </Button>
              <span
                style={{
                  fontSize: "12px",
                  color: "var(--text-tertiary)",
                  textAlign: "right",
                  maxWidth: "260px",
                }}>
                Créez d'abord un portefeuille pour enregistrer un achat.
              </span>
            </>
          ) : (
            <Button
              variant="primary"
              icon={Plus}
              onClick={() => setShowAddAsset(true)}>
              Ajouter au portefeuille
            </Button>
          )}
        </div>
      </div>

      <RefreshNotice />

      {/* Market data card */}
      {md ? (
        <Card level={1} className="mb-6">
          <h2
            className="mb-4"
            style={{
              fontSize: "16px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Données de marché
          </h2>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
            <div>
              <div
                className="uppercase mb-1"
                style={{
                  fontSize: "11px",
                  color: "var(--text-tertiary)",
                  letterSpacing: "0.05em",
                }}>
                Prix
              </div>
              <div
                style={{
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: "16px",
                  fontWeight: 700,
                  color: "var(--text-primary)",
                }}>
                {formatPrice(md.price_minor, md.currency)}
              </div>
            </div>
            {md.change_24h_pct != null && (
              <div>
                <div
                  className="uppercase mb-1"
                  style={{
                    fontSize: "11px",
                    color: "var(--text-tertiary)",
                    letterSpacing: "0.05em",
                  }}>
                  Variation 24h
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "16px",
                    fontWeight: 700,
                    color: md.change_24h_pct.startsWith("-")
                      ? "var(--color-loss)"
                      : "var(--color-gain)",
                  }}>
                  {md.change_24h_pct.startsWith("-") ? "" : "+"}
                  {md.change_24h_pct}%
                </div>
              </div>
            )}
            {md.change_7d_pct != null && (
              <div>
                <div
                  className="uppercase mb-1"
                  style={{
                    fontSize: "11px",
                    color: "var(--text-tertiary)",
                    letterSpacing: "0.05em",
                  }}>
                  Variation 7j
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "16px",
                    fontWeight: 700,
                    color: md.change_7d_pct.startsWith("-")
                      ? "var(--color-loss)"
                      : "var(--color-gain)",
                  }}>
                  {md.change_7d_pct.startsWith("-") ? "" : "+"}
                  {md.change_7d_pct}%
                </div>
              </div>
            )}
            {md.change_30d_pct != null && (
              <div>
                <div
                  className="uppercase mb-1"
                  style={{
                    fontSize: "11px",
                    color: "var(--text-tertiary)",
                    letterSpacing: "0.05em",
                  }}>
                  Variation 30j
                </div>
                <div
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: "16px",
                    fontWeight: 700,
                    color: md.change_30d_pct.startsWith("-")
                      ? "var(--color-loss)"
                      : "var(--color-gain)",
                  }}>
                  {md.change_30d_pct.startsWith("-") ? "" : "+"}
                  {md.change_30d_pct}%
                </div>
              </div>
            )}
          </div>
          <div
            style={{
              fontSize: "12px",
              color: "var(--text-tertiary)",
              marginTop: "12px",
            }}>
            Source : {md.data_source ?? "—"} • Dernière mise à jour :{" "}
            {formatDateTime(md.as_of)}
          </div>
        </Card>
      ) : (
        <Card level={1} className="mb-6">
          <div
            className="text-center"
            style={{
              padding: "24px 16px",
              color: "var(--text-tertiary)",
              fontSize: "14px",
            }}>
            Données de marché indisponibles
          </div>
        </Card>
      )}

      {/* Identity section */}
      <Card level={1} className="mb-6">
        <h2
          className="mb-4"
          style={{
            fontSize: "16px",
            fontWeight: 600,
            color: "var(--text-primary)",
          }}>
          Identité de l'actif
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8">
          <div>
            <InfoRow label="Nom" value={asset.name} />
            <InfoRow label="Ticker" value={asset.ticker} mono />
            <InfoRow label="Symbole" value={asset.symbol} mono />
            <InfoRow label="ISIN" value={asset.isin} mono />
            <InfoRow
              label="Classe d'actif"
              value={assetClassLabel(asset.asset_class)}
            />
          </div>
          <div>
            <InfoRow label="Bourse" value={asset.exchange} />
            <InfoRow label="Réseau" value={asset.network} />
            <InfoRow label="Devise native" value={asset.native_currency} mono />
            <InfoRow
              label="Statut"
              value={
                asset.status === "active"
                  ? "Actif"
                  : asset.status === "inactive"
                    ? "Inactif"
                    : asset.status === "delisted"
                      ? "Délisté"
                      : asset.status
              }
            />
          </div>
        </div>
      </Card>

      {/* Metadata section */}
      {meta &&
        (meta.sector ||
          meta.industry ||
          meta.country ||
          meta.description ||
          meta.website_url) && (
          <Card level={1} className="mb-6">
            <h2
              className="mb-4"
              style={{
                fontSize: "16px",
                fontWeight: 600,
                color: "var(--text-primary)",
              }}>
              Informations complémentaires
            </h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8">
              <div>
                <InfoRow label="Secteur" value={meta.sector} />
                <InfoRow label="Industrie" value={meta.industry} />
                <InfoRow label="Pays" value={meta.country} />
              </div>
              <div>
                {meta.website_url && (
                  <div
                    className="flex items-center justify-between"
                    style={{
                      padding: "10px 0",
                      borderBottom: "1px solid var(--surface-1-border)",
                    }}>
                    <span
                      style={{
                        fontSize: "14px",
                        color: "var(--text-tertiary)",
                      }}>
                      Site web
                    </span>
                    <a
                      href={meta.website_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1"
                      style={{
                        fontSize: "14px",
                        fontWeight: 500,
                        color: "var(--color-accent)",
                      }}>
                      Voir
                      <ExternalLink size={12} />
                    </a>
                  </div>
                )}
                <InfoRow label="Provider" value={meta.provider} />
              </div>
            </div>
            {meta.description && (
              <div style={{ marginTop: "16px" }}>
                <div
                  style={{
                    fontSize: "12px",
                    fontWeight: 500,
                    color: "var(--text-tertiary)",
                    marginBottom: "6px",
                  }}>
                  Description
                </div>
                <p
                  style={{
                    fontSize: "14px",
                    color: "var(--text-secondary)",
                    lineHeight: "1.6",
                  }}>
                  {meta.description}
                </p>
              </div>
            )}
          </Card>
        )}

      {/* Aliases section */}
      {asset.aliases && asset.aliases.length > 0 && (
        <Card level={1} className="mb-6">
          <h2
            className="mb-4"
            style={{
              fontSize: "16px",
              fontWeight: 600,
              color: "var(--text-primary)",
            }}>
            Identifiants alternatifs
          </h2>
          <div className="flex flex-wrap gap-2">
            {asset.aliases.map((alias, i) => (
              <span
                key={i}
                className="rounded-full px-3 py-1"
                style={{
                  fontSize: "12px",
                  background: "var(--surface-2-bg)",
                  border: "1px solid var(--surface-2-border)",
                  color: "var(--text-secondary)",
                }}>
                <span style={{ fontWeight: 600 }}>{alias.alias_type}:</span>{" "}
                {alias.alias_value}
              </span>
            ))}
          </div>
        </Card>
      )}

      {/* "Ajouter au portefeuille" records a posted `buy` operation against the
          active portfolio — it does NOT create or modify any asset-catalogue
          row. The modal reuses the existing operation contract, idempotency-key
          lifecycle and refresh tracking; the returned refresh request is shown
          via the shared <RefreshNotice /> above (no second polling loop). */}
      {showAddAsset && portfolioReady && asset && activePortfolioId && (
        <CreateOperationModal
          portfolioId={activePortfolioId}
          initialOperationType="buy"
          initialAsset={asset}
          onClose={() => setShowAddAsset(false)}
        />
      )}
    </div>
  );
}
