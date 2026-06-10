"use client";

import { useState, type FormEvent, type ReactNode } from "react";
import Link from "next/link";
import { AlertTriangle, CheckCircle2, Copy, Eye, EyeOff, Loader2, ShieldCheck } from "lucide-react";
import { Button } from "@/mockup/components/Button";
import { Card } from "@/mockup/components/Card";
import { Input } from "@/mockup/components/Input";
import * as authApi from "@/lib/auth-api";
import { AuthApiError } from "@/lib/auth-api";
import { storeTokens } from "@/lib/auth-storage";
import { generateRecoveryPhrase } from "@/lib/recovery-phrase";
import { useI18n } from "@/i18n/context";
import type { Dictionary } from "@/i18n/types";

type LoginErrors = Partial<Record<"username" | "password" | "api", string>>;
type RegisterErrors = Partial<
  Record<"username" | "password" | "confirmPassword" | "api", string>
>;
type RecoveryErrors = Partial<
  Record<"username" | "recoveryPhrase" | "newPassword" | "api", string>
>;

function validateUsername(value: string, t: Dictionary) {
  if (!value.trim()) return t.validation.usernameRequired;
  if (!/^[a-z0-9_][a-z0-9_-]{2,39}$/.test(value))
    return t.validation.usernameFormat;
  return undefined;
}

function validatePassword(password: string, t: Dictionary) {
  if (!password) return t.validation.passwordRequired;
  if (password.length < 12) return t.validation.passwordMinLength;
  if (password.length > 128) return t.validation.passwordMaxLength;
  return undefined;
}

function normalizePhrase(raw: string): string {
  return raw.trim().replace(/\s+/g, " ");
}

function countWords(phrase: string): number {
  const normalized = normalizePhrase(phrase);
  if (!normalized) return 0;
  return normalized.split(" ").length;
}

function mapApiError(error: unknown, t: Dictionary): string {
  if (error instanceof AuthApiError) {
    switch (error.code) {
      case "invalid_credentials":
        return t.apiErrors.invalidCredentials;
      case "username_conflict":
        return t.apiErrors.usernameConflict;
      case "rate_limited":
        return t.apiErrors.rateLimited;
      case "network_error":
        return t.apiErrors.networkError;
      case "password_too_short":
        return t.apiErrors.passwordTooShort;
      case "password_too_long":
        return t.apiErrors.passwordTooLong;
      case "invalid_username":
        return t.apiErrors.invalidUsername;
      case "blank_password":
        return t.apiErrors.blankPassword;
      case "invalid_recovery_phrase":
        return t.apiErrors.invalidRecoveryPhrase;
      default:
        return error.serverMessage || t.apiErrors.genericError;
    }
  }
  return t.apiErrors.unexpectedError;
}

function BrandHeader({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="text-center mb-8">
      <div
        className="w-12 h-12 rounded-full mx-auto mb-4 flex items-center justify-center"
        style={{ background: "var(--color-accent)" }}
      >
        <span className="text-white font-bold text-xl">K</span>
      </div>
      <h1 className="mb-2" style={{ fontSize: "24px", fontWeight: 700 }}>
        {title}
      </h1>
      <p style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>{subtitle}</p>
    </div>
  );
}

export function LoginForm() {
  const { t } = useI18n();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<LoginErrors>({});
  const [status, setStatus] = useState<"idle" | "loading" | "success">("idle");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const nextErrors: LoginErrors = {
      username: validateUsername(username, t),
      password: password.trim() ? undefined : t.validation.passwordRequired,
    };
    setErrors(nextErrors);
    if (nextErrors.username || nextErrors.password) return;

    setStatus("loading");
    setErrors({});

    try {
      const response = await authApi.login({
        username,
        password,
      });
      storeTokens(response.access_token, response.refresh_token);
      setStatus("success");
      const appUrl = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:5173";
      window.setTimeout(() => {
        window.location.href = appUrl;
      }, 350);
    } catch (error) {
      setStatus("idle");
      setErrors({ api: mapApiError(error, t) });
    }
  };

  return (
    <AuthCard title={t.login.title} subtitle={t.login.subtitle} onSubmit={submit}>
      <Input
        label={t.login.usernameLabel}
        type="text"
        placeholder={t.login.usernamePlaceholder}
        value={username}
        onChange={(event) => setUsername(event.target.value.toLowerCase())}
        error={errors.username}
        autoComplete="username"
        required
      />
      <PasswordField
        label={t.login.passwordLabel}
        placeholder={t.login.passwordPlaceholder}
        value={password}
        onChange={setPassword}
        show={showPassword}
        onToggle={() => setShowPassword((current) => !current)}
        error={errors.password}
        showLabel={t.login.showPassword}
        hideLabel={t.login.hidePassword}
      />
      {errors.api && <ErrorMessage>{errors.api}</ErrorMessage>}
      {status === "success" && <SuccessMessage>{t.login.successRedirect}</SuccessMessage>}
      <div className="text-right">
        <Link href="/recuperation" style={{ fontSize: "12px", color: "var(--text-tertiary)" }}>
          {t.login.forgotPassword}
        </Link>
      </div>
      <Button type="submit" variant="primary" className="w-full" disabled={status === "loading"}>
        {status === "loading" ? (
          <>
            <Loader2 size={16} className="animate-spin" /> {t.login.submitting}
          </>
        ) : (
          t.login.submit
        )}
      </Button>
      <FooterLink text={t.login.noAccount} href="/inscription" label={t.login.createAccount} />
    </AuthCard>
  );
}

export function RegisterForm() {
  const { t } = useI18n();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<RegisterErrors>({});
  const [status, setStatus] = useState<"idle" | "loading">("idle");

  const [step, setStep] = useState<"form" | "phraseSetup">("form");
  const [generatedPhrase, setGeneratedPhrase] = useState("");
  const [accessToken, setAccessToken] = useState("");
  const [savedPassword, setSavedPassword] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [setupStatus, setSetupStatus] = useState<"idle" | "loading" | "success">("idle");
  const [setupError, setSetupError] = useState("");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const nextErrors: RegisterErrors = {
      username: validateUsername(username, t),
      password: validatePassword(password, t),
      confirmPassword:
        confirmPassword === password ? undefined : t.validation.passwordMismatch,
    };
    setErrors(nextErrors);
    if (nextErrors.username || nextErrors.password || nextErrors.confirmPassword) return;

    setStatus("loading");
    setErrors({});

    try {
      const response = await authApi.signup({ username, password });
      storeTokens(response.access_token, response.refresh_token);
      setAccessToken(response.access_token);
      setSavedPassword(password);
      const phrase = generateRecoveryPhrase();
      setGeneratedPhrase(phrase);
      setStep("phraseSetup");
    } catch (error) {
      setStatus("idle");
      setErrors({ api: mapApiError(error, t) });
    }
  };

  const confirmSetup = async () => {
    setSetupStatus("loading");
    setSetupError("");

    try {
      await authApi.setupRecoveryPhrase(accessToken, {
        current_password: savedPassword,
        recovery_phrase: generatedPhrase,
      });
      setSetupStatus("success");
      const appUrl = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:5173";
      window.setTimeout(() => {
        window.location.href = appUrl;
      }, 550);
    } catch (error) {
      setSetupStatus("idle");
      setSetupError(mapApiError(error, t));
    }
  };

  if (step === "phraseSetup") {
    return (
      <PhraseDisplayCard
        title={t.recoverySetup.title}
        subtitle={t.recoverySetup.subtitle}
        intro={t.recoverySetup.phraseIntro}
        warning={t.recoverySetup.phraseWarning}
        phrase={generatedPhrase}
      >
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={confirmed}
            onChange={(event) => setConfirmed(event.target.checked)}
            className="mt-0.5 accent-[var(--color-accent)]"
          />
          <span style={{ fontSize: "13px", color: "var(--text-secondary)" }}>
            {t.recoverySetup.confirmLabel}
          </span>
        </label>
        {setupError && <ErrorMessage>{setupError}</ErrorMessage>}
        {setupStatus === "success" && (
          <SuccessMessage>{t.recoverySetup.successRedirect}</SuccessMessage>
        )}
        <Button
          type="button"
          variant="primary"
          className="w-full"
          disabled={!confirmed || setupStatus === "loading"}
          onClick={confirmSetup}
        >
          {setupStatus === "loading" ? (
            <>
              <Loader2 size={16} className="animate-spin" /> {t.recoverySetup.confirming}
            </>
          ) : (
            t.recoverySetup.confirm
          )}
        </Button>
      </PhraseDisplayCard>
    );
  }

  return (
    <AuthCard title={t.signup.title} subtitle={t.signup.subtitle} onSubmit={submit}>
      <Input
        label={t.signup.usernameLabel}
        type="text"
        placeholder={t.signup.usernamePlaceholder}
        value={username}
        onChange={(event) => setUsername(event.target.value.toLowerCase())}
        error={errors.username}
        autoComplete="username"
        required
      />
      <PasswordField
        label={t.signup.passwordLabel}
        placeholder={t.signup.passwordPlaceholder}
        value={password}
        onChange={setPassword}
        show={showPassword}
        onToggle={() => setShowPassword((current) => !current)}
        error={errors.password}
        showLabel={t.signup.showPassword}
        hideLabel={t.signup.hidePassword}
      />
      <Input
        label={t.signup.confirmPasswordLabel}
        type={showPassword ? "text" : "password"}
        placeholder={t.signup.confirmPasswordPlaceholder}
        value={confirmPassword}
        onChange={(event) => setConfirmPassword(event.target.value)}
        error={errors.confirmPassword}
        autoComplete="new-password"
        required
      />
      <Card level={2} className="flex gap-3">
        <AlertTriangle size={16} style={{ color: "var(--color-warning)", flexShrink: 0 }} />
        <p style={{ fontSize: "12px", lineHeight: 1.5, color: "var(--text-secondary)" }}>
          {t.signup.passwordHint}
        </p>
      </Card>
      {errors.api && <ErrorMessage>{errors.api}</ErrorMessage>}
      <Button type="submit" variant="primary" className="w-full" disabled={status === "loading"}>
        {status === "loading" ? (
          <>
            <Loader2 size={16} className="animate-spin" /> {t.signup.submitting}
          </>
        ) : (
          t.signup.submit
        )}
      </Button>
      <FooterLink text={t.signup.hasAccount} href="/connexion" label={t.signup.signIn} />
    </AuthCard>
  );
}

export function RecoveryForm() {
  const { t } = useI18n();
  const [username, setUsername] = useState("");
  const [recoveryPhrase, setRecoveryPhrase] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<RecoveryErrors>({});
  const [status, setStatus] = useState<"idle" | "loading">("idle");

  const [step, setStep] = useState<"form" | "phraseRotation">("form");
  const [newPhrase, setNewPhrase] = useState("");

  const wordCount = countWords(recoveryPhrase);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const normalized = normalizePhrase(recoveryPhrase);
    const nextErrors: RecoveryErrors = {
      username: validateUsername(username, t),
      recoveryPhrase: !normalized
        ? t.validation.recoveryPhraseRequired
        : countWords(recoveryPhrase) !== 12
          ? t.recovery.phraseWordCountError
          : undefined,
      newPassword: validatePassword(newPassword, t),
    };
    setErrors(nextErrors);
    if (nextErrors.username || nextErrors.recoveryPhrase || nextErrors.newPassword) return;

    setStatus("loading");
    setErrors({});

    const generatedNewPhrase = generateRecoveryPhrase();

    try {
      await authApi.resetPassword({
        username,
        recovery_phrase: normalized,
        new_password: newPassword,
        new_recovery_phrase: generatedNewPhrase,
      });
      setNewPhrase(generatedNewPhrase);
      setStep("phraseRotation");
    } catch (error) {
      setStatus("idle");
      setErrors({ api: mapApiError(error, t) });
    }
  };

  if (step === "phraseRotation") {
    return (
      <PhraseDisplayCard
        title={t.recoveryRotation.title}
        subtitle={t.recoveryRotation.subtitle}
        intro={t.recoveryRotation.phraseIntro}
        warning={t.recoveryRotation.phraseWarning}
        phrase={newPhrase}
      >
        <Link href="/connexion">
          <Button variant="primary" className="w-full">
            {t.recoveryRotation.confirm}
          </Button>
        </Link>
      </PhraseDisplayCard>
    );
  }

  return (
    <AuthCard title={t.recovery.title} subtitle={t.recovery.subtitle} onSubmit={submit}>
      <Input
        label={t.recovery.usernameLabel}
        type="text"
        placeholder={t.recovery.usernamePlaceholder}
        value={username}
        onChange={(event) => setUsername(event.target.value.toLowerCase())}
        error={errors.username}
        autoComplete="username"
        required
      />
      <RecoveryPhraseField
        label={t.recovery.phraseLabel}
        placeholder={t.recovery.phrasePlaceholder}
        helperText={t.recovery.phraseHelper}
        value={recoveryPhrase}
        onChange={setRecoveryPhrase}
        error={errors.recoveryPhrase}
        wordCount={wordCount}
      />
      <PasswordField
        label={t.recovery.newPasswordLabel}
        placeholder={t.recovery.passwordPlaceholder}
        value={newPassword}
        onChange={setNewPassword}
        show={showPassword}
        onToggle={() => setShowPassword((current) => !current)}
        error={errors.newPassword}
        showLabel={t.recovery.showPassword}
        hideLabel={t.recovery.hidePassword}
      />
      {errors.api && <ErrorMessage>{errors.api}</ErrorMessage>}
      <Button type="submit" variant="primary" className="w-full" disabled={status === "loading"}>
        {status === "loading" ? (
          <>
            <Loader2 size={16} className="animate-spin" /> {t.recovery.submitting}
          </>
        ) : (
          t.recovery.submit
        )}
      </Button>
      <FooterLink text={t.recovery.hasCredentials} href="/connexion" label={t.recovery.signIn} />
    </AuthCard>
  );
}

export function RecoveryConfirmation() {
  const { t } = useI18n();
  return (
    <div className="min-h-screen flex items-center justify-center px-6 py-12">
      <Card level={1} className="w-full max-w-[420px] text-center">
        <CheckCircle2 size={42} className="mx-auto mb-4" style={{ color: "var(--color-gain)" }} />
        <h1 className="mb-2" style={{ fontSize: "24px", fontWeight: 700 }}>
          {t.recoveryRotation.title}
        </h1>
        <p className="mb-6" style={{ fontSize: "14px", color: "var(--text-secondary)" }}>
          {t.recoveryRotation.subtitle}
        </p>
        <Link href="/connexion">
          <Button variant="primary" className="w-full">
            {t.recoveryRotation.confirm}
          </Button>
        </Link>
      </Card>
    </div>
  );
}

function AuthCard({
  title,
  subtitle,
  onSubmit,
  children,
}: {
  title: string;
  subtitle: string;
  onSubmit: (event: FormEvent) => void;
  children: ReactNode;
}) {
  return (
    <div
      className="min-h-screen flex items-center justify-center px-6"
      style={{ paddingTop: "120px", paddingBottom: "48px" }}
    >
      <Card level={1} className="w-full max-w-[400px]">
        <BrandHeader title={title} subtitle={subtitle} />
        <form onSubmit={onSubmit} className="space-y-4" noValidate>
          {children}
        </form>
      </Card>
    </div>
  );
}

function PhraseDisplayCard({
  title,
  subtitle,
  intro,
  warning,
  phrase,
  children,
}: {
  title: string;
  subtitle: string;
  intro: string;
  warning: string;
  phrase: string;
  children: ReactNode;
}) {
  const [copied, setCopied] = useState(false);

  const copyPhrase = async () => {
    try {
      await navigator.clipboard.writeText(phrase);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard not available
    }
  };

  return (
    <div
      className="min-h-screen flex items-center justify-center px-6"
      style={{ paddingTop: "120px", paddingBottom: "48px" }}
    >
      <Card level={1} className="w-full max-w-[440px]">
        <div className="text-center mb-6">
          <div
            className="w-12 h-12 rounded-full mx-auto mb-4 flex items-center justify-center"
            style={{ background: "var(--color-accent)" }}
          >
            <ShieldCheck size={24} className="text-white" />
          </div>
          <h1 className="mb-2" style={{ fontSize: "24px", fontWeight: 700 }}>
            {title}
          </h1>
          <p style={{ fontSize: "14px", color: "var(--text-tertiary)" }}>{subtitle}</p>
        </div>
        <div className="space-y-4">
          <p style={{ fontSize: "13px", color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {intro}
          </p>
          <div className="relative">
            <Card level={2} className="font-mono text-center select-all" style={{ padding: "20px 16px" }}>
              <p style={{ fontSize: "16px", lineHeight: 2, color: "var(--text-primary)", wordSpacing: "0.3em" }}>
                {phrase}
              </p>
            </Card>
            <button
              type="button"
              onClick={copyPhrase}
              className="absolute top-3 right-3 p-1.5 rounded-lg transition-colors"
              style={{
                background: copied ? "var(--color-gain)" : "var(--surface-2)",
                color: copied ? "white" : "var(--text-tertiary)",
              }}
              aria-label="Copy"
            >
              {copied ? <CheckCircle2 size={14} /> : <Copy size={14} />}
            </button>
          </div>
          <Card level={2} className="flex gap-3">
            <AlertTriangle size={16} style={{ color: "var(--color-warning)", flexShrink: 0 }} />
            <p style={{ fontSize: "12px", lineHeight: 1.5, color: "var(--text-secondary)" }}>
              {warning}
            </p>
          </Card>
          {children}
        </div>
      </Card>
    </div>
  );
}

function PasswordField({
  label,
  placeholder,
  value,
  onChange,
  show,
  onToggle,
  error,
  showLabel,
  hideLabel,
}: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
  show: boolean;
  onToggle: () => void;
  error?: string;
  showLabel: string;
  hideLabel: string;
}) {
  return (
    <div className="relative">
      <Input
        label={label}
        type={show ? "text" : "password"}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        error={error}
        autoComplete="current-password"
        required
      />
      <button
        type="button"
        onClick={onToggle}
        className="absolute right-4 top-[38px]"
        style={{ color: "var(--text-tertiary)" }}
        aria-label={show ? hideLabel : showLabel}
      >
        {show ? <EyeOff size={16} /> : <Eye size={16} />}
      </button>
    </div>
  );
}

function RecoveryPhraseField({
  label,
  placeholder,
  helperText,
  value,
  onChange,
  error,
  wordCount,
}: {
  label: string;
  placeholder: string;
  helperText: string;
  value: string;
  onChange: (value: string) => void;
  error?: string;
  wordCount: number;
}) {
  const showCounter = value.trim().length > 0;

  return (
    <div className="w-full">
      <label
        className="block mb-1.5"
        style={{ fontSize: "12px", fontWeight: 500, color: "var(--text-secondary)" }}
      >
        {label}
      </label>
      <textarea
        className="glass-field w-full px-5 py-3 rounded-[16px] transition-all resize-none"
        style={{
          border: error ? "1px solid var(--color-loss)" : "1px solid var(--surface-2-border)",
          fontSize: "15px",
          color: "var(--text-primary)",
          transition: "all var(--transition-base)",
          minHeight: "100px",
          lineHeight: 1.6,
          fontFamily: "inherit",
        }}
        placeholder={placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        rows={3}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
      />
      {showCounter && (
        <p
          className="mt-1 text-right"
          style={{
            fontSize: "11px",
            color:
              wordCount === 12
                ? "var(--color-gain)"
                : wordCount > 12
                  ? "var(--color-loss)"
                  : "var(--text-tertiary)",
            fontVariantNumeric: "tabular-nums",
          }}
        >
          {wordCount}/12
        </p>
      )}
      {error && (
        <p className="mt-1" style={{ fontSize: "12px", color: "var(--color-loss)" }}>
          {error}
        </p>
      )}
      {helperText && !error && (
        <p className="mt-1" style={{ fontSize: "12px", color: "var(--text-tertiary)" }}>
          {helperText}
        </p>
      )}
    </div>
  );
}

function SuccessMessage({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2" style={{ color: "var(--color-gain)", fontSize: "13px" }}>
      <CheckCircle2 size={16} />
      <span>{children}</span>
    </div>
  );
}

function ErrorMessage({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2" style={{ color: "var(--color-loss)", fontSize: "13px" }}>
      <AlertTriangle size={16} />
      <span>{children}</span>
    </div>
  );
}

function FooterLink({ text, href, label }: { text: string; href: string; label: string }) {
  return (
    <div className="text-center pt-4">
      <span style={{ fontSize: "14px", color: "var(--text-secondary)" }}>{text} </span>
      <Link href={href} style={{ fontSize: "14px", color: "var(--color-accent)", fontWeight: 500 }}>
        {label}
      </Link>
    </div>
  );
}
