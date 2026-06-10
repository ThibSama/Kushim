import type { Dictionary } from "./types";

export const fr: Dictionary = {
  locale: "fr",

  common: {
    kushim: "KUSHIM",
    loading: "Chargement…",
    or: "ou",
  },

  nav: {
    product: "Produit",
    security: "Sécurité",
    pricing: "Tarifs",
    signIn: "Se connecter",
    getStarted: "Commencer",
    toggleTheme: "Changer le thème",
    language: "Langue",
    closeMenu: "Fermer le menu",
    menu: "Menu",
  },

  login: {
    title: "Connexion",
    subtitle: "Accédez à votre espace.",
    usernameLabel: "Nom d'utilisateur",
    usernamePlaceholder: "ex. camille_durand",
    passwordLabel: "Mot de passe",
    passwordPlaceholder: "Minimum 12 caractères",
    showPassword: "Afficher le mot de passe",
    hidePassword: "Masquer le mot de passe",
    forgotPassword: "Mot de passe oublié ?",
    submit: "Se connecter",
    submitting: "Connexion…",
    successRedirect: "Connexion validée. Redirection…",
    noAccount: "Pas encore de compte ?",
    createAccount: "Créer un accès",
  },

  signup: {
    title: "Créer un accès",
    subtitle: "Configurez votre compte Kushim.",
    usernameLabel: "Nom d'utilisateur",
    usernamePlaceholder: "ex. camille_durand",
    passwordLabel: "Mot de passe",
    passwordPlaceholder: "Minimum 12 caractères",
    confirmPasswordLabel: "Confirmer le mot de passe",
    confirmPasswordPlaceholder: "Confirmez votre mot de passe",
    showPassword: "Afficher le mot de passe",
    hidePassword: "Masquer le mot de passe",
    passwordHint:
      "Utilisez un mot de passe long et unique (12 caractères minimum).",
    submit: "Créer mon accès",
    submitting: "Création…",
    hasAccount: "Déjà un compte ?",
    signIn: "Se connecter",
  },

  recoverySetup: {
    title: "Phrase de récupération",
    subtitle: "Sauvegardez cette phrase pour récupérer votre accès.",
    phraseIntro:
      "Voici votre phrase de récupération. Elle est le seul moyen de réinitialiser votre mot de passe.",
    phraseWarning:
      "Notez ces 12 mots dans un endroit sûr. Cette phrase ne sera plus affichée.",
    confirmLabel: "J'ai bien sauvegardé ma phrase de récupération",
    confirm: "Confirmer et continuer",
    confirming: "Enregistrement…",
    successRedirect: "Phrase enregistrée. Redirection…",
  },

  recovery: {
    title: "Récupérer mon accès",
    subtitle:
      "Réinitialisez votre mot de passe à l'aide de votre phrase de récupération.",
    usernameLabel: "Nom d'utilisateur",
    usernamePlaceholder: "ex. camille_durand",
    phraseLabel: "Phrase de récupération",
    phrasePlaceholder:
      "Collez vos 12 mots dans l'ordre, séparés par des espaces",
    phraseHelper:
      "Votre phrase contient exactement 12 mots, dans l'ordre original, séparés par des espaces.",
    phraseWordCountError:
      "La phrase de récupération doit contenir exactement 12 mots.",
    newPasswordLabel: "Nouveau mot de passe",
    passwordPlaceholder: "Minimum 12 caractères",
    showPassword: "Afficher le mot de passe",
    hidePassword: "Masquer le mot de passe",
    submit: "Réinitialiser le mot de passe",
    submitting: "Réinitialisation…",
    hasCredentials: "Vous avez vos identifiants ?",
    signIn: "Se connecter",
  },

  recoveryRotation: {
    title: "Nouvelle phrase de récupération",
    subtitle: "Votre mot de passe a été réinitialisé. Votre ancienne phrase n'est plus valide.",
    phraseIntro:
      "Voici votre nouvelle phrase de récupération. Elle remplace l'ancienne.",
    phraseWarning:
      "Notez ces 12 mots dans un endroit sûr. Cette phrase ne sera plus affichée.",
    confirm: "J'ai sauvegardé ma phrase",
  },

  validation: {
    usernameRequired: "Le nom d'utilisateur est requis.",
    usernameFormat:
      "3 à 40 caractères : minuscules, chiffres, _ ou - (commence par une lettre, un chiffre ou _).",
    passwordRequired: "Le mot de passe est requis.",
    passwordMinLength: "Utilisez au moins 12 caractères.",
    passwordMaxLength: "128 caractères maximum.",
    passwordMismatch: "Les mots de passe ne correspondent pas.",
    recoveryPhraseRequired: "La phrase de récupération est requise.",
  },

  apiErrors: {
    invalidCredentials: "Nom d'utilisateur ou mot de passe incorrect.",
    usernameConflict: "Ce nom d'utilisateur est déjà pris.",
    rateLimited: "Trop de tentatives. Réessayez dans quelques minutes.",
    passwordTooShort:
      "Le mot de passe doit comporter au moins 12 caractères.",
    passwordTooLong: "Le mot de passe ne doit pas dépasser 128 caractères.",
    invalidUsername: "Format de nom d'utilisateur invalide.",
    blankPassword: "Le mot de passe ne peut pas être vide.",
    invalidRecoveryPhrase: "Phrase de récupération invalide.",
    handoffFailed: "Impossible d'ouvrir votre session dans l'application. Réessayez.",
    genericError: "Une erreur est survenue.",
    unexpectedError: "Une erreur inattendue est survenue.",
    networkError:
      "Impossible de contacter le serveur d'authentification.",
  },

  footer: {
    guide: "Guide",
    sitemap: "Plan du site",
    resources: "Ressources",
    documentation: "Documentation",
    about: "À propos",
    contact: "Contact",
    cookies: "Cookies",
    terms: "CGU",
    privacy: "Politique de confidentialité",
    legal: "Mentions légales",
  },
};
