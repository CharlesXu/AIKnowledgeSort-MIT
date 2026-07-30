import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { ModelSettingsDialog } from "./ModelSettingsDialog";
import type { ModelRuntimeClient, ModelRuntimeState } from "./types";

const TEST_CREDENTIAL = ["test", "credential", "fixture"].join("-");

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
    providerProtocol: "openAi",
    credentialSource: "environment",
    credentialEnvironment: "AIKS_MODEL_API_KEY_REMOTE_REASONER",
    credentialStored: false,
  }],
};

function client(): ModelRuntimeClient {
  return {
    inspect: vi.fn().mockResolvedValue(state),
    discoverModels: vi.fn().mockResolvedValue({
      providerProtocol: "openAi",
      modelsEndpointUrl: "https://models.example.com/v1/models",
      completionEndpointUrl: "https://models.example.com/v1/chat/completions",
      models: ["reasoner-v1", "reasoner-v2"],
    }),
    upsert: vi.fn().mockResolvedValue(state),
    remove: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
    runComparison: vi.fn(),
    runFileSemanticComparison: vi.fn(),
  };
}

describe("ModelSettingsDialog", () => {
  test("automatically discovers models after URL and API key entry", async () => {
    const modelClient = client();
    render(
      <ModelSettingsDialog
        client={modelClient}
        onClose={vi.fn()}
        triggerRef={{ current: null }}
      />,
    );
    await screen.findByText("Remote Reasoner");
    fireEvent.change(screen.getByLabelText("Configuration ID"), {
      target: { value: "automatic" },
    });
    fireEvent.change(screen.getByLabelText("Location"), {
      target: { value: "remote" },
    });
    fireEvent.change(screen.getByLabelText("Endpoint URL"), {
      target: { value: "https://models.example.com" },
    });
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Use bearer authentication",
    }));
    fireEvent.change(screen.getByLabelText("Credential source"), {
      target: { value: "keychain" },
    });
    fireEvent.change(screen.getByLabelText("API Key"), {
      target: { value: TEST_CREDENTIAL },
    });

    await waitFor(() => expect(modelClient.discoverModels).toHaveBeenCalledWith({
      configId: "automatic",
      location: "remote",
      endpointUrl: "https://models.example.com",
      timeoutMs: 30_000,
      authenticated: true,
      credentialSource: "keychain",
      apiKey: TEST_CREDENTIAL,
    }), { timeout: 1_500 });
    expect(await screen.findByRole("option", { name: "reasoner-v2" }))
      .toBeInTheDocument();
  });

  test("accepts an API key, refreshes models, and selects a discovered model", async () => {
    const modelClient = client();
    render(
      <ModelSettingsDialog
        client={modelClient}
        onClose={vi.fn()}
        triggerRef={{ current: null }}
      />,
    );
    await screen.findByText("Remote Reasoner");

    fireEvent.change(screen.getByLabelText("Configuration ID"), {
      target: { value: "remote-reasoner" },
    });
    fireEvent.change(screen.getByLabelText("Location"), {
      target: { value: "remote" },
    });
    fireEvent.change(screen.getByLabelText("Endpoint URL"), {
      target: { value: "https://models.example.com/v1" },
    });
    fireEvent.click(screen.getByRole("checkbox", {
      name: "Use bearer authentication",
    }));
    fireEvent.change(screen.getByLabelText("Credential source"), {
      target: { value: "keychain" },
    });
    fireEvent.change(screen.getByLabelText("API Key"), {
      target: { value: TEST_CREDENTIAL },
    });
    fireEvent.click(screen.getByRole("button", { name: "Refresh models" }));

    await waitFor(() => expect(modelClient.discoverModels).toHaveBeenCalledWith({
      configId: "remote-reasoner",
      location: "remote",
      endpointUrl: "https://models.example.com/v1",
      timeoutMs: 30_000,
      authenticated: true,
      credentialSource: "keychain",
      apiKey: TEST_CREDENTIAL,
    }));
    expect(await screen.findByRole("option", { name: "reasoner-v2" }))
      .toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "reasoner-v2" },
    });
    expect(screen.getByLabelText("Model")).toHaveValue("reasoner-v2");
  });

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
    expect(screen.queryByLabelText("API Key")).toBeNull();

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
      providerProtocol: "openAi",
      credentialSource: "environment",
      apiKey: null,
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
