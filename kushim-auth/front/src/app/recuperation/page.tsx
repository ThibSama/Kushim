import { RecoveryForm } from "../form-card";
import { pageMetadata } from "../metadata";

export const metadata = pageMetadata(
  "Recuperation",
  "Demandez les instructions de recuperation de votre acces Kushim.",
  "/recuperation",
);

export default function RecoveryPage() {
  return <RecoveryForm />;
}
