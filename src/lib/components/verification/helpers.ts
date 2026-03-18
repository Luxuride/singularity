import type { MatrixDeviceInfo, MatrixVerificationFlowResponse } from "$lib/chats/types";
import type { MatrixRecoveryState } from "$lib/auth/types";

export function deviceTrustClass(trust: MatrixDeviceInfo["trust"]): string {
  if (trust === "crossSigned" || trust === "locallyVerified") {
    return "bg-success-200-800";
  }

  if (trust === "notVerified") {
    return "bg-warning-200-800";
  }

  return "bg-surface-200-800";
}

export function deviceTrustLabel(trust: MatrixDeviceInfo["trust"]): string {
  if (trust === "crossSigned") return "Cross-signing verified";
  if (trust === "locallyVerified") return "Locally verified";
  if (trust === "ownDevice") return "This device";
  return "Not verified";
}

export function flowStateLabel(flow: MatrixVerificationFlowResponse): string {
  if (flow.isDone) {
    return "Verification complete";
  }

  if (flow.isCancelled) {
    return "Verification cancelled";
  }

  if (flow.sasState) {
    return `SAS ${flow.sasState}`;
  }

  return `Request ${flow.requestState}`;
}

export function recoveryStateLabel(state: MatrixRecoveryState | null): string {
  if (state === "enabled") return "Recovery enabled";
  if (state === "incomplete") return "Recovery incomplete";
  if (state === "disabled") return "Recovery disabled";
  return "Recovery status unknown";
}
