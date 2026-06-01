"use client";

import { useState, type FormEvent, type ReactNode } from "react";
import Link from "next/link";
import { AlertTriangle, CheckCircle2, Eye, EyeOff } from "lucide-react";
import { Button } from "@/mockup/components/Button";
import { Card } from "@/mockup/components/Card";
import { Input } from "@/mockup/components/Input";

type FormErrors = Partial<Record<"email" | "password" | "confirmPassword", string>>;

const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function validateEmail(email: string) {
  if (!email.trim()) return "L'email est requis.";
  if (!emailPattern.test(email)) return "Entrez une adresse email valide.";
  return undefined;
}

function validatePassword(password: string) {
  if (!password) return "Le mot de passe est requis.";
  if (password.length < 10) return "Utilisez au moins 10 caracteres.";
  if (!/[A-Z]/.test(password)) return "Ajoutez une majuscule.";
  if (!/[0-9]/.test(password)) return "Ajoutez un chiffre.";
  return undefined;
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
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});
  const [status, setStatus] = useState<"idle" | "success">("idle");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const nextErrors = {
      email: validateEmail(email),
      password: validatePassword(password),
    };
    setErrors(nextErrors);
    if (nextErrors.email || nextErrors.password) return;

    setStatus("success");
    // TODO: connect to kushim-auth/api for credential verification.
    window.setTimeout(() => {
      const appUrl = process.env.NEXT_PUBLIC_APP_URL ?? "http://localhost:5173";
      window.location.href = `${appUrl}?token=demo`;
    }, 350);
  };

  return (
    <AuthCard title="Connexion" subtitle="Accedez a votre espace." onSubmit={submit}>
      <Input
        label="Email"
        type="email"
        placeholder="vous@exemple.com"
        value={email}
        onChange={(event) => setEmail(event.target.value)}
        error={errors.email}
        required
      />
      <PasswordField
        label="Mot de passe"
        value={password}
        onChange={setPassword}
        show={showPassword}
        onToggle={() => setShowPassword((current) => !current)}
        error={errors.password}
      />
      {status === "success" && <SuccessMessage>Connexion validee. Redirection...</SuccessMessage>}
      <div className="text-right">
        <Link href="/recuperation" style={{ fontSize: "12px", color: "var(--text-tertiary)" }}>
          Mot de passe oublie ?
        </Link>
      </div>
      <Button type="submit" variant="primary" className="w-full">
        Se connecter
      </Button>
      <FooterLink text="Pas encore de compte ?" href="/inscription" label="Creer un acces" />
    </AuthCard>
  );
}

export function RegisterForm() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});
  const [success, setSuccess] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const nextErrors = {
      email: validateEmail(email),
      password: validatePassword(password),
      confirmPassword: confirmPassword === password ? undefined : "Les mots de passe ne correspondent pas.",
    };
    setErrors(nextErrors);
    if (nextErrors.email || nextErrors.password || nextErrors.confirmPassword) return;

    setSuccess(true);
    // TODO: connect to kushim-auth/api for account creation.
    window.setTimeout(() => {
      window.location.href = "/connexion";
    }, 550);
  };

  return (
    <AuthCard title="Creer un acces" subtitle="Configurez votre compte Kushim." onSubmit={submit}>
      <Input
        label="Email"
        type="email"
        placeholder="vous@exemple.com"
        value={email}
        onChange={(event) => setEmail(event.target.value)}
        error={errors.email}
        required
      />
      <PasswordField
        label="Mot de passe"
        value={password}
        onChange={setPassword}
        show={showPassword}
        onToggle={() => setShowPassword((current) => !current)}
        error={errors.password}
      />
      <Input
        label="Confirmer le mot de passe"
        type={showPassword ? "text" : "password"}
        placeholder="Confirmez votre mot de passe"
        value={confirmPassword}
        onChange={(event) => setConfirmPassword(event.target.value)}
        error={errors.confirmPassword}
        required
      />
      <Card level={2} className="flex gap-3">
        <AlertTriangle size={16} style={{ color: "var(--color-warning)", flexShrink: 0 }} />
        <p style={{ fontSize: "12px", lineHeight: 1.5, color: "var(--text-secondary)" }}>
          Utilisez un mot de passe long et unique. La recuperation restera limitee dans cette
          maquette.
        </p>
      </Card>
      {success && <SuccessMessage>Compte cree. Redirection vers la connexion...</SuccessMessage>}
      <Button type="submit" variant="primary" className="w-full">
        Creer mon acces
      </Button>
      <FooterLink text="Deja un compte ?" href="/connexion" label="Se connecter" />
    </AuthCard>
  );
}

export function RecoveryForm() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | undefined>();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const nextError = validateEmail(email);
    setError(nextError);
    if (nextError) return;

    // TODO: connect to kushim-auth/api for recovery request dispatch.
    window.location.href = "/recuperation/confirmation";
  };

  return (
    <AuthCard
      title="Recuperer mon acces"
      subtitle="Recevez les instructions de recuperation."
      onSubmit={submit}
    >
      <Input
        label="Email"
        type="email"
        placeholder="vous@exemple.com"
        value={email}
        onChange={(event) => setEmail(event.target.value)}
        error={error}
        required
      />
      <Button type="submit" variant="primary" className="w-full">
        Envoyer les instructions
      </Button>
      <FooterLink text="Vous avez vos identifiants ?" href="/connexion" label="Se connecter" />
    </AuthCard>
  );
}

export function RecoveryConfirmation() {
  return (
    <div className="min-h-screen flex items-center justify-center px-6 py-12">
      <Card level={1} className="w-full max-w-[420px] text-center">
        <CheckCircle2 size={42} className="mx-auto mb-4" style={{ color: "var(--color-gain)" }} />
        <h1 className="mb-2" style={{ fontSize: "24px", fontWeight: 700 }}>
          Demande envoyee
        </h1>
        <p className="mb-6" style={{ fontSize: "14px", color: "var(--text-secondary)" }}>
          Si un compte existe pour cet email, les instructions de recuperation seront envoyees.
        </p>
        <Link href="/connexion">
          <Button variant="primary" className="w-full">
            Retour a la connexion
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

function PasswordField({
  label,
  value,
  onChange,
  show,
  onToggle,
  error,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  show: boolean;
  onToggle: () => void;
  error?: string;
}) {
  return (
    <div className="relative">
      <Input
        label={label}
        type={show ? "text" : "password"}
        placeholder="Minimum 10 caracteres"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        error={error}
        required
      />
      <button
        type="button"
        onClick={onToggle}
        className="absolute right-4 top-[38px]"
        style={{ color: "var(--text-tertiary)" }}
        aria-label={show ? "Masquer le mot de passe" : "Afficher le mot de passe"}
      >
        {show ? <EyeOff size={16} /> : <Eye size={16} />}
      </button>
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
