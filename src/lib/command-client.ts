import { invoke } from "@tauri-apps/api/core";
import { toMessage } from "./errors";

/**
 * Generic wrapper for invoking Matrix Tauri commands with unified error handling.
 * Catches any invoke errors and converts them to readable error messages using `toMessage`.
 *
 * @template T The expected return type of the command.
 * @param commandName The name of the Tauri command to invoke.
 * @param params Optional parameters object to pass to the command.
 * @returns A promise resolving to the command result of type T.
 * @throws Error with a readable message if the command fails.
 */
export async function invokeMatrixCommand<T>(
  commandName: string,
  params?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(commandName, params);
  } catch (error) {
    throw new Error(toMessage(error));
  }
}
