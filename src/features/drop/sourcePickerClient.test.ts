import { describe, expect, test, vi } from "vitest";
import {
  createBrowserSourcePickerClient,
  createTauriSourcePickerClient,
} from "./sourcePickerClient";

describe("source picker client", () => {
  test("requests opaque grants without receiving local paths", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({ grantId: "files-grant" })
      .mockResolvedValueOnce(null);
    const client = createTauriSourcePickerClient(invoke);

    await expect(client.chooseFiles()).resolves.toEqual({
      grantId: "files-grant",
    });
    await expect(client.chooseFolders()).resolves.toBeNull();
    expect(invoke).toHaveBeenNthCalledWith(1, "choose_local_files");
    expect(invoke).toHaveBeenNthCalledWith(2, "choose_local_folders");
  });

  test("does not simulate filesystem access in a browser", async () => {
    const client = createBrowserSourcePickerClient();

    await expect(client.chooseFiles()).rejects.toThrow(
      "Adding local sources requires the desktop app.",
    );
    await expect(client.chooseFolders()).rejects.toThrow(
      "Adding local sources requires the desktop app.",
    );
  });
});
