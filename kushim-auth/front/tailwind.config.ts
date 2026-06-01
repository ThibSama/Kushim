import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/app/**/*.{ts,tsx}", "./src/mockup/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        gain: "var(--color-gain)",
        loss: "var(--color-loss)",
        accent: "var(--color-accent)",
        canvas: "var(--canvas-bg)",
      },
      borderRadius: {
        sm: "var(--radius-sm)",
        md: "var(--radius-md)",
        lg: "var(--radius-lg)",
        xl: "var(--radius-xl)",
        pill: "var(--radius-pill)",
      },
      boxShadow: {
        glass: "var(--glass-shadow)",
        "glass-soft": "var(--glass-shadow-soft)",
        "glass-strong": "var(--glass-shadow-strong)",
      },
      fontFamily: {
        sans: ["Inter", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },
    },
  },
};

export default config;
