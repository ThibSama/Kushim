import type { Metadata } from "next";
import "./globals.css";
import { SiteShell } from "./site-shell";

export const metadata: Metadata = {
  metadataBase: new URL("http://localhost:3000"),
  title: {
    default: "Kushim",
    template: "%s | Kushim",
  },
  description: "Suivi patrimonial prive, multi-actifs et zero-knowledge.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="fr" suppressHydrationWarning>
      <body>
        <SiteShell>{children}</SiteShell>
      </body>
    </html>
  );
}
