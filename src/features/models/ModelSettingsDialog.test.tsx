import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { ModelSettingsDialog } from "./ModelSettingsDialog";
import type { ModelRuntimeClient, ModelRuntimeState } from "./types";

const state: ModelRuntimeState = {
  schemaVersion: 1,
  configs: [{
    configId: "remote-reasoner",
    label: "Remote Reasoner",
    location: "remote",
    endpointUrl: "https://models.example.com/v1/chat/completions",
    model: "reasoner-v1",
    timeoutMs: 60_000,
    authenticated: true,
    credentialEnvironment: "AIKS_MODEL_API_KEY_REMOTE_REASONER",
  }],
};

function client(): ModelRuntimeClient {
  return {
    inspect: vi.fn().mockResolvedValue(state),
    upsert: vi.fn().mockResolvedValue(state),
    remove: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
    runComparison: vi.fn(),
  };
}

describe("ModelSettingsDialog", () => {
  test("lists configs and submits only secret-free model fields", async () => {
    const modelClient = client();
    const trigger = { current: null };
    render(
      <ModelSettingsDialog
        client={modelClient}
        onClose={vi.fn()}
        triggerRef={trigger}
      />,
    );

    expect(await screen.findByText("Remote Reasoner")).toBeInTheDocument();
    expect(screen.getByText("AIKS_MODEL_API_KEY_REMOTE_REASONER"))
      .toBeInTheDocument();
    expect(screen.queryByLabelText(/api key|password|secret/i)).toBeNull();

    fireEvent.change(screen.getByLabelText("Configuration ID"), {
      target: { value: "local-ollama" },
    });
    fireEvent.change(screen.getByLabelText("Label"), {
      target: { value: "Local Ollama" },
    });
    fireEvent.change(screen.getByLabelText("Location"), {
      target: { value: "local" },
    });
    fireEvent.change(screen.getByLabelText("Endpoint URL"), {
      target: { value: "http://127.0.0.1:11434/v1/chat/completions" },
    });
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "qwen3:8b" },
    });
    fireEvent.change(screen.getByLabelText("Timeout (seconds)"), {
      target: { value: "30" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save model config" }));

    await waitFor(() => expect(modelClient.upsert).toHaveBeenCalledWith({
      configId: "local-ollama",
      label: "Local Ollama",
      location: "local",
      endpointUrl: "http://127.0.0.1:11434/v1/chat/completions",
      model: "qwen3:8b",
      timeoutMs: 30_000,
      authenticated: false,
    }));
  });

  test("preserves entered values after a native failure", async () => {
    const modelClient = client();
    modelClient.upsert = vi.fn().mockRejectedValue(new Error("Native write failed"));
    render(
      <ModelSettingsDialog
        client={modelClient}
        onClose={vi.fn()}
        triggerRef={{ current: null }}
      />,
    );
    await screen.findByText("Remote Reasoner");
    fireEvent.change(screen.getByLabelText("Configuration ID"), {
      target: { value: "kept-config" },
    });
    fireEvent.change(screen.getByLabelText("Label"), {
      target: { value: "Kept label" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save model config" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Native write failed");
    expect(screen.getByLabelText("Configuration ID")).toHaveValue("kept-config");
    expect(screen.getByLabelText("Label")).toHaveValue("Kept label");
  });

  test("closes with Escape and restores focus to the Settings button", async () => {
    function Harness() {
      const triggerRef = useRef<HTMLButtonElement>(null);
      const [open, setOpen] = useState(true);
      return (
        <>
          <button ref={triggerRef} type="button">Settings</button>
          {open ? (
            <ModelSettingsDialog
              client={client()}
              onClose={() => setOpen(false)}
              triggerRef={triggerRef}
            />
          ) : null}
        </>
      );
    }
    render(<Harness />);
    await screen.findByRole("dialog", { name: "Model runtime settings" });
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(screen.getByRole("button", { name: "Settings" })).toHaveFocus();
    });
  });
});
