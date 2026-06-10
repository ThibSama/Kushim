export type Locale = "fr" | "en";

export interface Dictionary {
  locale: Locale;

  common: {
    kushim: string;
    loading: string;
    or: string;
  };

  nav: {
    product: string;
    security: string;
    pricing: string;
    signIn: string;
    getStarted: string;
    toggleTheme: string;
    language: string;
    closeMenu: string;
    menu: string;
  };

  login: {
    title: string;
    subtitle: string;
    usernameLabel: string;
    usernamePlaceholder: string;
    passwordLabel: string;
    passwordPlaceholder: string;
    showPassword: string;
    hidePassword: string;
    forgotPassword: string;
    submit: string;
    submitting: string;
    successRedirect: string;
    noAccount: string;
    createAccount: string;
  };

  signup: {
    title: string;
    subtitle: string;
    usernameLabel: string;
    usernamePlaceholder: string;
    passwordLabel: string;
    passwordPlaceholder: string;
    confirmPasswordLabel: string;
    confirmPasswordPlaceholder: string;
    showPassword: string;
    hidePassword: string;
    passwordHint: string;
    submit: string;
    submitting: string;
    hasAccount: string;
    signIn: string;
  };

  recoverySetup: {
    title: string;
    subtitle: string;
    phraseIntro: string;
    phraseWarning: string;
    confirmLabel: string;
    confirm: string;
    confirming: string;
    successRedirect: string;
  };

  recovery: {
    title: string;
    subtitle: string;
    usernameLabel: string;
    usernamePlaceholder: string;
    phraseLabel: string;
    phrasePlaceholder: string;
    phraseHelper: string;
    phraseWordCountError: string;
    newPasswordLabel: string;
    passwordPlaceholder: string;
    showPassword: string;
    hidePassword: string;
    submit: string;
    submitting: string;
    hasCredentials: string;
    signIn: string;
  };

  recoveryRotation: {
    title: string;
    subtitle: string;
    phraseIntro: string;
    phraseWarning: string;
    confirm: string;
  };

  validation: {
    usernameRequired: string;
    usernameFormat: string;
    passwordRequired: string;
    passwordMinLength: string;
    passwordMaxLength: string;
    passwordMismatch: string;
    recoveryPhraseRequired: string;
  };

  apiErrors: {
    invalidCredentials: string;
    usernameConflict: string;
    rateLimited: string;
    passwordTooShort: string;
    passwordTooLong: string;
    invalidUsername: string;
    blankPassword: string;
    invalidRecoveryPhrase: string;
    handoffFailed: string;
    genericError: string;
    unexpectedError: string;
    networkError: string;
  };

  footer: {
    guide: string;
    sitemap: string;
    resources: string;
    documentation: string;
    about: string;
    contact: string;
    cookies: string;
    terms: string;
    privacy: string;
    legal: string;
  };
}
