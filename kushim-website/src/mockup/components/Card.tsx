"use client";

import React from "react";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  level?: 1 | 2 | 3;
  noPadding?: boolean;
}

export function Card({
  children,
  className = "",
  level = 1,
  style,
  noPadding = false,
  ...props
}: CardProps) {
  const levelStyles = {
    1: {
      borderRadius: "var(--radius-xl)",
      padding: "clamp(20px, 3vw, 24px)",
    },
    2: {
      borderRadius: "var(--radius-md)",
      padding: "clamp(14px, 2.5vw, 16px)",
    },
    3: {
      borderRadius: "var(--radius-md)",
      padding: "clamp(14px, 2.5vw, 16px)",
    },
  };

  const mergedStyle = {
    ...levelStyles[level],
    ...style,
    ...(noPadding ? { padding: "0px" } : null),
  } as React.CSSProperties;

  return (
    <div
      className={[
        level === 1 ? "glass glass-hover" : level === 2 ? "glass-elevated glass-hover" : "glass-strong glass-hover",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      style={mergedStyle}
      {...props}>
      {children}
    </div>
  );
}
