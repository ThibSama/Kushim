import Image from "next/image";

type BrandMarkProps = {
  variant?: "compact" | "standard" | "symbol";
  showName?: boolean;
  className?: string;
};

const variantSizes = {
  compact: 24,
  standard: 30,
  symbol: 46,
} as const;

export function BrandMark({
  variant = "standard",
  showName = variant !== "symbol",
  className,
}: BrandMarkProps) {
  const size = variantSizes[variant];

  return (
    <span
      className={["inline-flex items-center shrink-0", className].filter(Boolean).join(" ")}
      style={{ gap: variant === "compact" ? "8px" : "10px" }}
      aria-label={showName ? undefined : "Kushim"}
    >
      <span
        aria-hidden="true"
        style={{
          alignItems: "center",
          background: "#FAFAFA",
          borderRadius: "9999px",
          boxShadow: "0 0 0 1px var(--surface-1-border)",
          display: "inline-flex",
          flexShrink: 0,
          height: `${size}px`,
          justifyContent: "center",
          width: `${size}px`,
        }}
      >
        <Image
          src="/brand/junon-moneta.svg"
          alt=""
          width={size - 4}
          height={size - 4}
          draggable={false}
          priority={variant === "compact"}
          style={{ display: "block", objectFit: "contain" }}
        />
      </span>
      {showName && (
        <span
          className="uppercase tracking-wider"
          style={{
            color: "var(--text-primary)",
            fontSize: variant === "compact" ? "clamp(14px, 2.5vw, 16px)" : "16px",
            fontWeight: 800,
            letterSpacing: "0.04em",
            whiteSpace: "nowrap",
          }}
        >
          KUSHIM
        </span>
      )}
    </span>
  );
}
