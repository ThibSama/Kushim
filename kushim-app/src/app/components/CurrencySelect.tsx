import React, { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { Search, ChevronDown, Loader2 } from "lucide-react";
import { listCurrencies, type ReferenceItem } from "../../lib/api/businessApi";

type Props = {
  /** Canonical uppercase ISO 4217 code currently selected, or empty string. */
  value: string;
  /** Called with the canonical uppercase code when the user picks an entry. */
  onChange: (code: string) => void;
  label?: string;
  /** External error to render under the field (e.g. backend rejection). */
  error?: string;
  /** Optional id for testing / external label association. */
  id?: string;
};

// Module-scoped cache. The reference catalogue is static for a given
// frontend build and tiny (~150 entries), so we fetch it once per page load
// and share the result between every CurrencySelect instance.
let cachedCurrencies: ReferenceItem[] | null = null;
let inflightPromise: Promise<ReferenceItem[]> | null = null;

async function fetchCurrenciesOnce(): Promise<ReferenceItem[]> {
  if (cachedCurrencies) return cachedCurrencies;
  if (inflightPromise) return inflightPromise;
  inflightPromise = listCurrencies()
    .then((items) => {
      cachedCurrencies = items;
      inflightPromise = null;
      return items;
    })
    .catch((err) => {
      inflightPromise = null;
      throw err;
    });
  return inflightPromise;
}

/** Reset the cache (used by tests). */
export function __resetCurrencyCacheForTests() {
  cachedCurrencies = null;
  inflightPromise = null;
}

function localizedLabel(code: string, fallbackLabel: string): string {
  try {
    const intlAny = Intl as unknown as {
      DisplayNames?: new (
        locales: string[] | string,
        options: { type: string },
      ) => { of(code: string): string | undefined };
    };
    if (intlAny.DisplayNames) {
      const dn = new intlAny.DisplayNames(["fr"], { type: "currency" });
      const localized = dn.of(code);
      if (localized && localized !== code) return localized;
    }
  } catch {
    // Fall through to backend label.
  }
  return fallbackLabel;
}

export function CurrencySelect({ value, onChange, label, error, id }: Props) {
  // Lazy initializer hydrates from the module-scoped cache without a
  // cascading re-render (React 19 `set-state-in-effect` rule).
  const [items, setItems] = useState<ReferenceItem[] | null>(
    () => cachedCurrencies,
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (items) return;
    let cancelled = false;
    fetchCurrenciesOnce()
      .then((data) => {
        if (!cancelled) setItems(data);
      })
      .catch(() => {
        if (!cancelled)
          setLoadError("Impossible de charger la liste des devises.");
      });
    return () => {
      cancelled = true;
    };
  }, [items]);

  const filtered = useMemo<ReferenceItem[]>(() => {
    if (!items) return [];
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((item) => {
      const code = item.value.toLowerCase();
      const labelText = localizedLabel(item.value, item.label).toLowerCase();
      return code.includes(q) || labelText.includes(q);
    });
  }, [items, query]);

  // Clamp at usage time rather than via an effect — keeps activeIndex within
  // bounds without a re-render cascade.
  const clampedActiveIndex =
    filtered.length === 0
      ? 0
      : Math.min(activeIndex, filtered.length - 1);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    };
    if (open) document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setOpen(false);
      }
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open]);

  const handleSelect = useCallback(
    (code: string) => {
      onChange(code.toUpperCase());
      setOpen(false);
      setQuery("");
    },
    [onChange],
  );

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex(
        Math.min(clampedActiveIndex + 1, Math.max(filtered.length - 1, 0)),
      );
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex(Math.max(clampedActiveIndex - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const candidate = filtered[clampedActiveIndex];
      if (candidate) handleSelect(candidate.value);
    }
  };

  const selectedLabel = value
    ? `${value} — ${localizedLabel(value, items?.find((i) => i.value === value)?.label ?? value)}`
    : "Sélectionner une devise";

  const listboxId = id ? `${id}-listbox` : undefined;

  return (
    <div ref={containerRef} className="w-full" style={{ position: "relative" }}>
      {label && (
        <label
          htmlFor={id}
          className="block mb-1.5"
          style={{
            fontSize: "12px",
            fontWeight: 500,
            color: "var(--text-secondary)",
          }}
        >
          {label}
        </label>
      )}
      <button
        type="button"
        id={id}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        onClick={() => {
          setOpen((o) => {
            const next = !o;
            if (next) setTimeout(() => inputRef.current?.focus(), 0);
            return next;
          });
        }}
        className="glass-field w-full px-5 py-3 rounded-[9999px] flex items-center justify-between"
        style={{
          border: "1px solid var(--surface-2-border)",
          fontSize: "15px",
          color: value ? "var(--text-primary)" : "var(--text-secondary)",
          background: "var(--surface-1-bg)",
          cursor: "pointer",
          textAlign: "left",
        }}
      >
        <span>{selectedLabel}</span>
        <ChevronDown size={16} aria-hidden="true" />
      </button>

      {open && (
        <div
          className="glass-field"
          style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            left: 0,
            right: 0,
            zIndex: 60,
            border: "1px solid var(--surface-2-border)",
            background: "var(--surface-1-bg)",
            borderRadius: "16px",
            overflow: "hidden",
          }}
        >
          <div
            className="flex items-center gap-2 px-4 py-2"
            style={{ borderBottom: "1px solid var(--surface-2-border)" }}
          >
            <Search size={14} aria-hidden="true" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={handleInputKeyDown}
              placeholder="Rechercher (code ou nom)…"
              aria-label="Rechercher une devise"
              autoComplete="off"
              className="w-full"
              style={{
                background: "transparent",
                border: "none",
                outline: "none",
                fontSize: "14px",
                color: "var(--text-primary)",
              }}
            />
          </div>
          <ul
            role="listbox"
            id={listboxId}
            style={{
              maxHeight: "240px",
              overflowY: "auto",
              listStyle: "none",
              margin: 0,
              padding: 0,
            }}
          >
            {!items && !loadError && (
              <li
                className="px-4 py-3 flex items-center gap-2"
                style={{ fontSize: "13px", color: "var(--text-secondary)" }}
              >
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
                Chargement…
              </li>
            )}
            {loadError && (
              <li
                className="px-4 py-3"
                style={{ fontSize: "13px", color: "var(--color-loss)" }}
              >
                {loadError}
              </li>
            )}
            {items && filtered.length === 0 && (
              <li
                className="px-4 py-3"
                style={{ fontSize: "13px", color: "var(--text-secondary)" }}
              >
                Aucune devise trouvée.
              </li>
            )}
            {filtered.map((item, idx) => {
              const isSelected = item.value === value;
              const isActive = idx === clampedActiveIndex;
              return (
                <li
                  key={item.value}
                  role="option"
                  aria-selected={isSelected}
                  onMouseEnter={() => setActiveIndex(idx)}
                  onClick={() => handleSelect(item.value)}
                  style={{
                    padding: "10px 16px",
                    fontSize: "14px",
                    cursor: "pointer",
                    color: "var(--text-primary)",
                    background: isActive ? "var(--surface-2-bg)" : "transparent",
                    fontWeight: isSelected ? 600 : 400,
                  }}
                >
                  <span style={{ fontFamily: "monospace" }}>{item.value}</span>
                  <span style={{ color: "var(--text-secondary)" }}>
                    {" — "}
                    {localizedLabel(item.value, item.label)}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {error && (
        <p
          style={{
            marginTop: "6px",
            fontSize: "12px",
            color: "var(--color-loss)",
          }}
        >
          {error}
        </p>
      )}
    </div>
  );
}
