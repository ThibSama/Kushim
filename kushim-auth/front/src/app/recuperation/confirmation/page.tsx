import { RecoveryConfirmation } from "../../form-card";
import { pageMetadata } from "../../metadata";

export const metadata = pageMetadata(
  "Confirmation",
  "Confirmation de demande de recuperation Kushim.",
  "/recuperation/confirmation",
);

export default function RecoveryConfirmationPage() {
  return <RecoveryConfirmation />;
}
