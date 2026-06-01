import React, { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Search,
  Download,
  Plus,
  ArrowLeftRight,
  ChevronDown,
  ChevronUp,
  MoreHorizontal,
  PencilLine,
  Trash2,
} from "lucide-react";
import { Card } from "../components/Card";
import { SwapModal } from "../components/SwapModal";
import {
  calculateAssetMetrics,
  calculateSectorMetrics,
  formatCurrency,
  formatQuantity,
  formatSignedCurrency,
  formatSignedPercent,
  getPerformanceTone,
} from "../../utils/portfolio";
import { assetGroups as groups } from "../../mocks/demoPortfolio";

const groupByOptions = ["Secteur", "Compte", "Devise", "Classe d'actifs"];

const thStyle: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: 500,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  color: "var(--text-tertiary)",
  padding: "10px 12px",
  whiteSpace: "nowrap",
};

const monoCell: React.CSSProperties = {
  fontFamily: "'JetBrains Mono', monospace",
  fontSize: "14px",
  fontVariantNumeric: "tabular-nums",
  color: "var(--text-primary)",
  padding: "14px 12px",
  whiteSpace: "nowrap",
};

const quickInputStyle: React.CSSProperties = {
  width: "100%",
  background: "var(--surface-2-bg)",
  border: "1px solid var(--surface-2-border)",
  borderRadius: "var(--radius-md)",
  padding: "10px 12px",
  fontSize: "13px",
  color: "var(--text-primary)",
};

const quickActionStyle: React.CSSProperties = {
  height: "36px",
  padding: "0 14px",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--surface-1-border)",
  background: "transparent",
  fontSize: "13px",
  fontWeight: 600,
  cursor: "pointer",
};

export function Assets() {
  const [search, setSearch] = useState("");
  const [groupBy, setGroupBy] = useState("Secteur");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    groups.forEach((g) => {
      if (g.defaultCollapsed) init[g.name] = true;
    });
    return init;
  });
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);
  const [editValues, setEditValues] = useState<{
    id: string;
    quantity: string;
    purchasePrice: string;
  } | null>(null);
  const [showSwap, setShowSwap] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    if (!menuOpenId) return;
    const handle = () => setMenuOpenId(null);
    document.addEventListener("click", handle);
    return () => document.removeEventListener("click", handle);
  }, [menuOpenId]);

  const toggle = (name: string) =>
    setCollapsed((p) => ({ ...p, [name]: !p[name] }));

  const filtered = groups
    .map((g) => ({
      ...g,
      sectorMetrics: calculateSectorMetrics(g.assets),
      assets: g.assets.filter(
        (a) =>
          a.name.toLowerCase().includes(search.toLowerCase()) ||
          a.ticker.toLowerCase().includes(search.toLowerCase()),
      ),
    }))
    .filter((g) => g.assets.length > 0);

  return (
    <div className="app-page-container max-w-[1200px] mx-auto px-4 sm:px-6 py-12">
      {/* Header */}
      <div className="mb-6">
        <h1
          style={{
            fontSize: "clamp(24px, 5vw, 30px)",
            fontWeight: 700,
            color: "var(--text-primary)",
          }}>
          Actifs
        </h1>
        <p
          style={{
            fontSize: "clamp(13px, 2.5vw, 14px)",
            color: "var(--text-secondary)",
            marginTop: "4px",
          }}>
          Vue consolidée de toutes les positions sur l'ensemble des comptes
        </p>
      </div>

      {/* Toolbar */}
      <div className="flex flex-col sm:flex-row sm:flex-wrap sm:items-center sm:justify-between gap-3 mt-6 mb-8">
        {/* Mobile: Stack filters vertically */}
        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3 w-full sm:w-auto">
          {/* Group by dropdown */}
          <div className="relative w-full sm:w-auto">
            <select
              value={groupBy}
              onChange={(e) => setGroupBy(e.target.value)}
              className="appearance-none pr-8 cursor-pointer w-full sm:w-auto"
              style={{
                background: "var(--surface-2-bg)",
                border: "1px solid var(--surface-2-border)",
                borderRadius: "var(--radius-md)",
                height: "44px",
                paddingLeft: "16px",
                paddingRight: "36px",
                fontSize: "14px",
                color: "var(--text-primary)",
                fontWeight: 500,
              }}>
              {groupByOptions.map((o) => (
                <option key={o} value={o}>{`Grouper par : ${o}`}</option>
              ))}
            </select>
            <ChevronDown
              size={16}
              className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none"
              style={{ color: "var(--text-tertiary)" }}
            />
          </div>

          {/* Search */}
          <div className="relative w-full sm:w-auto">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2"
              style={{ color: "var(--text-tertiary)" }}
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Rechercher un actif…"
              className="w-full sm:w-[280px]"
              style={{
                background: "var(--surface-2-bg)",
                border: "1px solid var(--surface-2-border)",
                borderRadius: "var(--radius-md)",
                height: "44px",
                paddingLeft: "36px",
                paddingRight: "16px",
                fontSize: "14px",
                color: "var(--text-primary)",
              }}
            />
          </div>
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-3 w-full sm:w-auto">
          <button
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "6px",
              height: "44px",
              padding: "0 16px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--surface-1-border)",
              background: "transparent",
              fontSize: "14px",
              fontWeight: 500,
              color: "var(--text-primary)",
              cursor: "pointer",
              flex: "1",
            }}>
            <Download size={16} />
            <span className="hidden sm:inline">Exporter</span>
          </button>
          <button
            onClick={() => setShowSwap(true)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "6px",
              height: "44px",
              padding: "0 16px",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--surface-1-border)",
              background: "transparent",
              fontSize: "14px",
              fontWeight: 500,
              color: "var(--text-primary)",
              cursor: "pointer",
              flex: "1",
            }}>
            <ArrowLeftRight size={16} />
            <span className="hidden sm:inline">Échanger des actifs</span>
          </button>
          <button
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "6px",
              height: "44px",
              padding: "0 16px",
              borderRadius: "var(--radius-md)",
              border: "none",
              background: "var(--color-cta-bg)",
              fontSize: "14px",
              fontWeight: 600,
              color: "var(--color-cta-text)",
              cursor: "pointer",
              flex: "1",
            }}>
            <Plus size={16} />
            <span className="hidden sm:inline">Ajouter un actif</span>
          </button>
        </div>
      </div>

      <SwapModal isOpen={showSwap} onClose={() => setShowSwap(false)} />

      {/* Grouped sections */}
      {filtered.map((group) => {
        const isCollapsed = !!collapsed[group.name];
        const tone = getPerformanceTone(group.sectorMetrics.performance);
        const toneColor =
          tone === "positive"
            ? "var(--color-gain)"
            : tone === "negative"
              ? "var(--color-loss)"
              : "var(--color-neutral)";

        return (
          <Card level={1} className="mb-4" key={group.name}>
            {/* Group header */}
            <button
              onClick={() => toggle(group.name)}
              className="w-full flex items-start justify-between cursor-pointer"
              style={{
                background: "none",
                border: "none",
                padding: 0,
                color: "inherit",
                gap: "8px",
                minHeight: "44px",
              }}>
              {/* Left: Chevron + Group name */}
              <div className="flex items-center gap-2 flex-shrink-0">
                {isCollapsed ? (
                  <ChevronDown
                    size={16}
                    style={{ color: "var(--text-secondary)" }}
                  />
                ) : (
                  <ChevronUp
                    size={16}
                    style={{ color: "var(--text-secondary)" }}
                  />
                )}
                <span
                  style={{
                    fontSize: "16px",
                    fontWeight: 700,
                    color: "var(--text-primary)",
                  }}
                  className="md:text-lg">
                  {group.name}
                </span>
                {/* Asset count - hidden on mobile */}
                <span
                  className="hidden sm:inline"
                  style={{
                    fontSize: "14px",
                    color: "var(--text-tertiary)",
                    marginLeft: "4px",
                  }}>
                  ({group.assets.length} actif
                  {group.assets.length > 1 ? "s" : ""})
                </span>
              </div>

              {/* Right: Metrics stack */}
              <div className="flex flex-col items-end gap-1.5 text-right flex-shrink-0 max-w-[150px] sm:max-w-none">
                <div
                  style={{ fontSize: "12px", color: "var(--text-secondary)" }}
                  aria-label={`Invested capital ${formatCurrency(group.sectorMetrics.investedValue)}`}>
                  <span className="sm:hidden">Invested: </span>
                  <span className="hidden sm:inline">Invested capital: </span>
                  <span
                    style={{
                      fontFamily: "'JetBrains Mono', monospace",
                      fontVariantNumeric: "tabular-nums",
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--text-primary)",
                    }}>
                    {formatCurrency(group.sectorMetrics.investedValue)}
                  </span>
                </div>
                <div
                  style={{ fontSize: "12px", color: "var(--text-secondary)" }}
                  aria-label={`Current value ${formatCurrency(group.sectorMetrics.currentValue)}`}>
                  <span className="sm:hidden">Current: </span>
                  <span className="hidden sm:inline">Current value: </span>
                  <span
                    style={{
                      fontFamily: "'JetBrains Mono', monospace",
                      fontVariantNumeric: "tabular-nums",
                      fontSize: "14px",
                      fontWeight: 600,
                      color: "var(--text-primary)",
                    }}>
                    {formatCurrency(group.sectorMetrics.currentValue)}
                  </span>
                </div>
                <div
                  className="whitespace-nowrap"
                  style={{
                    fontFamily: "'JetBrains Mono', monospace",
                    fontVariantNumeric: "tabular-nums",
                    fontSize: "clamp(11px, 2.6vw, 12px)",
                    fontWeight: 600,
                    color: toneColor,
                  }}
                  aria-label={`Sector performance ${formatSignedCurrency(group.sectorMetrics.performance)} ${formatSignedPercent(group.sectorMetrics.performancePct)}`}>
                  {formatSignedCurrency(group.sectorMetrics.performance)} (
                  {formatSignedPercent(group.sectorMetrics.performancePct)})
                </div>
              </div>
            </button>

            {!isCollapsed && (
              <>
                <div
                  style={{
                    borderBottom: "1px solid var(--surface-1-border)",
                    margin: "12px 0 0 0",
                  }}
                />
                <div
                  style={{
                    overflowX: "auto",
                    WebkitOverflowScrolling: "touch",
                  }}>
                  <table
                    style={{
                      width: "100%",
                      borderCollapse: "collapse",
                      minWidth: "800px",
                    }}>
                    <thead>
                      <tr>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "left",
                            minWidth: "140px",
                          }}>
                          Nom de l'actif
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "90px",
                          }}>
                          Quantité
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "120px",
                          }}
                          className="hidden sm:table-cell">
                          Prix moy. d'achat
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "110px",
                          }}
                          className="hidden sm:table-cell">
                          Total investi
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "110px",
                          }}>
                          Valeur actuelle
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "110px",
                          }}>
                          Gains / Pertes
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "center",
                            minWidth: "70px",
                          }}
                          className="hidden sm:table-cell">
                          Devise
                        </th>
                        <th
                          style={{
                            ...thStyle,
                            textAlign: "right",
                            minWidth: "70px",
                          }}>
                          Actions
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {group.assets.map((a, i) => {
                        const assetMetrics = calculateAssetMetrics(a);
                        const assetTone = getPerformanceTone(
                          assetMetrics.performance,
                        );
                        const assetColor =
                          assetTone === "positive"
                            ? "var(--color-gain)"
                            : assetTone === "negative"
                              ? "var(--color-loss)"
                              : "var(--color-neutral)";
                        const isEditing = editValues?.id === a.id;

                        return (
                          <React.Fragment key={a.id}>
                            <tr
                              onClick={() => navigate(`/actifs/${a.id}`)}
                              className="transition-colors cursor-pointer"
                              onMouseEnter={(e) => {
                                e.currentTarget.style.background =
                                  "var(--surface-2-bg)";
                              }}
                              onMouseLeave={(e) => {
                                e.currentTarget.style.background =
                                  "transparent";
                              }}
                              style={{
                                borderBottom:
                                  i < group.assets.length - 1
                                    ? "1px solid var(--surface-1-border)"
                                    : "none",
                              }}>
                              <td
                                style={{
                                  padding: "14px 12px",
                                  minWidth: "140px",
                                }}>
                                <div
                                  style={{
                                    fontSize: "14px",
                                    fontWeight: 500,
                                    color: "var(--text-primary)",
                                  }}>
                                  {a.name}
                                </div>
                                <div
                                  style={{
                                    fontSize: "12px",
                                    color: "var(--text-tertiary)",
                                  }}>
                                  {a.ticker}
                                </div>
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  minWidth: "90px",
                                }}>
                                {formatQuantity(a.quantity)}
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  minWidth: "120px",
                                }}
                                className="hidden sm:table-cell">
                                {formatCurrency(a.purchasePrice)}
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  minWidth: "110px",
                                }}
                                className="hidden sm:table-cell">
                                {formatCurrency(assetMetrics.investedValue)}
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  minWidth: "110px",
                                }}>
                                {formatCurrency(assetMetrics.currentValue)}
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  fontWeight: 600,
                                  color: assetColor,
                                  minWidth: "110px",
                                }}>
                                {formatSignedCurrency(assetMetrics.performance)}
                              </td>
                              <td
                                style={{
                                  ...monoCell,
                                  textAlign: "center",
                                  fontFamily: "Inter, sans-serif",
                                  fontSize: "14px",
                                  minWidth: "70px",
                                }}
                                className="hidden sm:table-cell">
                                {a.currency}
                              </td>
                              <td
                                style={{
                                  padding: "14px 12px",
                                  textAlign: "right",
                                  minWidth: "70px",
                                  position: "relative",
                                }}>
                                <button
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    setMenuOpenId((prev) =>
                                      prev === a.id ? null : a.id,
                                    );
                                  }}
                                  className="rounded-full"
                                  style={{
                                    width: "32px",
                                    height: "32px",
                                    border: "1px solid var(--surface-1-border)",
                                    background: "transparent",
                                    color: "var(--text-secondary)",
                                    cursor: "pointer",
                                  }}
                                  aria-label="Actions">
                                  <MoreHorizontal size={16} />
                                </button>
                                {menuOpenId === a.id && (
                                  <div
                                    onClick={(event) => event.stopPropagation()}
                                    className="flex flex-col"
                                    style={{
                                      position: "absolute",
                                      right: "10px",
                                      top: "46px",
                                      background: "var(--surface-3-bg)",
                                      border:
                                        "1px solid var(--surface-3-border)",
                                      borderRadius: "var(--radius-md)",
                                      padding: "8px",
                                      minWidth: "140px",
                                      gap: "6px",
                                      zIndex: 10,
                                      boxShadow:
                                        "0 12px 30px rgba(0, 0, 0, 0.15)",
                                      backdropFilter: "blur(16px)",
                                    }}>
                                    <button
                                      onClick={() => {
                                        setEditValues({
                                          id: a.id,
                                          quantity: String(a.quantity),
                                          purchasePrice: String(
                                            a.purchasePrice,
                                          ),
                                        });
                                        setMenuOpenId(null);
                                      }}
                                      className="flex items-center gap-2"
                                      style={{
                                        background: "transparent",
                                        border: "none",
                                        color: "var(--text-primary)",
                                        fontSize: "13px",
                                        cursor: "pointer",
                                      }}>
                                      <PencilLine size={14} />
                                      Editer
                                    </button>
                                    <button
                                      onClick={() => setMenuOpenId(null)}
                                      className="flex items-center gap-2"
                                      style={{
                                        background: "transparent",
                                        border: "none",
                                        color: "var(--color-loss)",
                                        fontSize: "13px",
                                        cursor: "pointer",
                                      }}>
                                      <Trash2 size={14} />
                                      Supprimer
                                    </button>
                                  </div>
                                )}
                              </td>
                            </tr>
                            {isEditing && (
                              <tr>
                                <td
                                  colSpan={8}
                                  style={{
                                    padding: "16px 12px",
                                    background: "var(--surface-2-bg)",
                                    borderBottom:
                                      i < group.assets.length - 1
                                        ? "1px solid var(--surface-1-border)"
                                        : "none",
                                  }}>
                                  <div className="flex flex-col lg:flex-row gap-4 lg:items-end">
                                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 flex-1">
                                      <div>
                                        <div
                                          style={{
                                            fontSize: "12px",
                                            color: "var(--text-tertiary)",
                                            marginBottom: "6px",
                                          }}>
                                          Quantite
                                        </div>
                                        <input
                                          value={editValues.quantity}
                                          onChange={(event) =>
                                            setEditValues((prev) =>
                                              prev
                                                ? {
                                                    ...prev,
                                                    quantity:
                                                      event.target.value,
                                                  }
                                                : prev,
                                            )
                                          }
                                          style={quickInputStyle}
                                        />
                                      </div>
                                      <div>
                                        <div
                                          style={{
                                            fontSize: "12px",
                                            color: "var(--text-tertiary)",
                                            marginBottom: "6px",
                                          }}>
                                          Prix d'achat
                                        </div>
                                        <input
                                          value={editValues.purchasePrice}
                                          onChange={(event) =>
                                            setEditValues((prev) =>
                                              prev
                                                ? {
                                                    ...prev,
                                                    purchasePrice:
                                                      event.target.value,
                                                  }
                                                : prev,
                                            )
                                          }
                                          style={quickInputStyle}
                                        />
                                      </div>
                                      <div>
                                        <div
                                          style={{
                                            fontSize: "12px",
                                            color: "var(--text-tertiary)",
                                            marginBottom: "6px",
                                          }}>
                                          Devise
                                        </div>
                                        <input
                                          value={a.currency}
                                          readOnly
                                          style={{
                                            ...quickInputStyle,
                                            opacity: 0.7,
                                          }}
                                        />
                                      </div>
                                    </div>
                                    <div className="flex gap-2">
                                      <button
                                        style={{
                                          ...quickActionStyle,
                                          color: "var(--text-secondary)",
                                        }}
                                        onClick={() => setEditValues(null)}>
                                        Annuler
                                      </button>
                                      <button
                                        style={{
                                          ...quickActionStyle,
                                          background: "var(--color-cta-bg)",
                                          color: "var(--color-cta-text)",
                                          border: "none",
                                        }}
                                        onClick={() => setEditValues(null)}>
                                        Enregistrer
                                      </button>
                                    </div>
                                  </div>
                                </td>
                              </tr>
                            )}
                          </React.Fragment>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </Card>
        );
      })}
    </div>
  );
}
