import { RegisterForm } from "../form-card";
import { pageMetadata } from "../metadata";

export const metadata = pageMetadata(
  "Inscription",
  "Creez votre acces Kushim avec validation email et mot de passe robuste.",
  "/inscription",
);

export default function RegisterPage() {
  return <RegisterForm />;
}
