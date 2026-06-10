import type { Dictionary } from "./types";

export const en: Dictionary = {
  locale: "en",

  common: {
    kushim: "KUSHIM",
    loading: "Loading…",
    or: "or",
  },

  nav: {
    product: "Product",
    security: "Security",
    pricing: "Pricing",
    signIn: "Sign in",
    getStarted: "Get started",
    toggleTheme: "Toggle theme",
    language: "Language",
    closeMenu: "Close menu",
    menu: "Menu",
  },

  login: {
    title: "Sign in",
    subtitle: "Access your account.",
    usernameLabel: "Username",
    usernamePlaceholder: "e.g. alex_martin",
    passwordLabel: "Password",
    passwordPlaceholder: "At least 12 characters",
    showPassword: "Show password",
    hidePassword: "Hide password",
    forgotPassword: "Forgot your password?",
    submit: "Sign in",
    submitting: "Signing in…",
    successRedirect: "Signed in. Redirecting…",
    noAccount: "Don't have an account?",
    createAccount: "Create an account",
  },

  signup: {
    title: "Create an account",
    subtitle: "Set up your Kushim account.",
    usernameLabel: "Username",
    usernamePlaceholder: "e.g. alex_martin",
    passwordLabel: "Password",
    passwordPlaceholder: "At least 12 characters",
    confirmPasswordLabel: "Confirm password",
    confirmPasswordPlaceholder: "Re-enter your password",
    showPassword: "Show password",
    hidePassword: "Hide password",
    passwordHint:
      "Use a long and unique password (12 characters minimum).",
    submit: "Create my account",
    submitting: "Creating…",
    hasAccount: "Already have an account?",
    signIn: "Sign in",
  },

  recoverySetup: {
    title: "Recovery phrase",
    subtitle: "Save this phrase to recover your access.",
    phraseIntro:
      "Here is your recovery phrase. It is the only way to reset your password.",
    phraseWarning:
      "Write down these 12 words in a safe place. This phrase will not be shown again.",
    confirmLabel: "I have saved my recovery phrase",
    confirm: "Confirm and continue",
    confirming: "Saving…",
    successRedirect: "Phrase saved. Redirecting…",
  },

  recovery: {
    title: "Recover access",
    subtitle: "Reset your password using your recovery phrase.",
    usernameLabel: "Username",
    usernamePlaceholder: "e.g. alex_martin",
    phraseLabel: "Recovery phrase",
    phrasePlaceholder: "Paste your 12 words in order, separated by spaces",
    phraseHelper:
      "Your phrase contains exactly 12 words, in the original order, separated by spaces.",
    phraseWordCountError:
      "The recovery phrase must contain exactly 12 words.",
    newPasswordLabel: "New password",
    passwordPlaceholder: "At least 12 characters",
    showPassword: "Show password",
    hidePassword: "Hide password",
    submit: "Reset password",
    submitting: "Resetting…",
    hasCredentials: "Have your credentials?",
    signIn: "Sign in",
  },

  recoveryRotation: {
    title: "New recovery phrase",
    subtitle: "Your password has been reset. Your old phrase is no longer valid.",
    phraseIntro:
      "Here is your new recovery phrase. It replaces the previous one.",
    phraseWarning:
      "Write down these 12 words in a safe place. This phrase will not be shown again.",
    confirm: "I have saved my phrase",
  },

  validation: {
    usernameRequired: "Username is required.",
    usernameFormat:
      "3–40 characters: lowercase letters, digits, _ or - (must start with a letter, digit, or _).",
    passwordRequired: "Password is required.",
    passwordMinLength: "Use at least 12 characters.",
    passwordMaxLength: "128 characters maximum.",
    passwordMismatch: "Passwords do not match.",
    recoveryPhraseRequired: "Recovery phrase is required.",
  },

  apiErrors: {
    invalidCredentials: "Incorrect username or password.",
    usernameConflict: "This username is already taken.",
    rateLimited: "Too many attempts. Please try again in a few minutes.",
    passwordTooShort: "Password must be at least 12 characters.",
    passwordTooLong: "Password must not exceed 128 characters.",
    invalidUsername: "Invalid username format.",
    blankPassword: "Password cannot be empty.",
    invalidRecoveryPhrase: "Invalid recovery phrase.",
    genericError: "An error occurred.",
    unexpectedError: "An unexpected error occurred.",
    networkError: "Unable to reach the authentication server.",
  },

  footer: {
    guide: "Guide",
    sitemap: "Sitemap",
    resources: "Resources",
    documentation: "Documentation",
    about: "About",
    contact: "Contact",
    cookies: "Cookies",
    terms: "Terms",
    privacy: "Privacy policy",
    legal: "Legal notice",
  },
};
