import { RegisterForm } from "../form-card";
import { pageMetadata } from "../metadata";

export const metadata = pageMetadata(
  "Inscription",
  "Creez votre acces Kushim avec un identifiant et un mot de passe robuste.",
  "/inscription",
);

export default function RegisterPage() {
  return <RegisterForm />;
}
