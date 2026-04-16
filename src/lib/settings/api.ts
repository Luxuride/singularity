import { invokeMatrixCommand } from "../command-client";

import type {
  MatrixGetMediaSettingsResponse,
  MatrixSetMediaSettingsRequest,
  MatrixSetMediaSettingsResponse,
} from "./types";

export async function matrixGetMediaSettings(): Promise<MatrixGetMediaSettingsResponse> {
  return invokeMatrixCommand<MatrixGetMediaSettingsResponse>("matrix_get_media_settings");
}

export async function matrixSetMediaSettings(
  request: MatrixSetMediaSettingsRequest,
): Promise<MatrixSetMediaSettingsResponse> {
  return invokeMatrixCommand<MatrixSetMediaSettingsResponse>("matrix_set_media_settings", {
    request,
  });
}
