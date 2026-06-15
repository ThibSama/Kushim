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

// Pre-hydration theme script. Resolves the stored theme (or prefers-color-scheme
// when none is stored) and applies the `dark` class to <html> before first paint.
// Without this, SSR HTML has no `dark` class, the browser paints once in light,
// then React's effect adds `dark` — visible as a FOUC flash on every page load.
const themeInitScript = `(function(){try{var s=localStorage.getItem("theme");var d=s?s==="dark":window.matchMedia("(prefers-color-scheme: dark)").matches;document.documentElement.classList.toggle("dark",d);}catch(e){document.documentElement.classList.add("dark");}})();`;

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="fr" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeInitScript }} />
      </head>
      <body>
        <AuthShell>{children}</AuthShell>
      </body>
    </html>
  );
}
