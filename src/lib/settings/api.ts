import { invoke } from "@tauri-apps/api/core";

import { toMessage } from "$lib/errors";
import type {
  MatrixGetMediaSettingsResponse,
  MatrixSetMediaSettingsRequest,
  MatrixSetMediaSettingsResponse,
} from "./types";

export async function matrixGetMediaSettings(): Promise<MatrixGetMediaSettingsResponse> {
  try {
    return await invoke<MatrixGetMediaSettingsResponse>("matrix_get_media_settings");
  } catch (error) {
    throw new Error(toMessage(error));
  }
}

export async function matrixSetMediaSettings(
  request: MatrixSetMediaSettingsRequest,
): Promise<MatrixSetMediaSettingsResponse> {
  try {
    return await invoke<MatrixSetMediaSettingsResponse>("matrix_set_media_settings", {
      request,
    });
  } catch (error) {
    throw new Error(toMessage(error));
  }
}
