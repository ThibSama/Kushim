import type { Metadata } from "next";
import "./globals.css";
import { AuthShell } from "./auth-shell";

export const metadata: Metadata = {
  title: {
    default: "Kushim Auth",
    template: "%s | Kushim Auth",
  },
  description: "Connexion, inscription et recuperation d'acces Kushim.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="fr" suppressHydrationWarning>
      <body>
        <AuthShell>{children}</AuthShell>
      </body>
    </html>
  );
}
