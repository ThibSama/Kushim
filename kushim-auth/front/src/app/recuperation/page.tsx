import { RecoveryForm } from "../form-card";
import { pageMetadata } from "../metadata";

export const metadata = pageMetadata(
  "Recuperation",
  "Reinitialiser votre mot de passe Kushim avec votre phrase de recuperation.",
  "/recuperation",
);

export default function RecoveryPage() {
  return <RecoveryForm />;
}
