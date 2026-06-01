import { LoginForm } from "../form-card";
import { pageMetadata } from "../metadata";

export const metadata = pageMetadata(
  "Connexion",
  "Connectez-vous a Kushim avec votre email et votre mot de passe.",
  "/connexion",
);

export default function LoginPage() {
  return <LoginForm />;
}
