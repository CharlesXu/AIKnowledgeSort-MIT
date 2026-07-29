import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useRef, useState } from "react";
import { describe, expect, test, vi } from "vitest";
import type { AgentAccessClient } from "../agentAccess/types";
import type { ModelRuntimeClient } from "../models/types";
import { SettingsDialog } from "./SettingsDialog";

const modelClient: ModelRuntimeClient = {
  inspect: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
  upsert: vi.fn(),
  remove: vi.fn(),
  runComparison: vi.fn(),
  runFileSemanticComparison: vi.fn(),
};

const agentClient: AgentAccessClient = {
  selectDirectories: vi.fn(),
  inspect: vi.fn().mockResolvedValue({
    schemaVersion: 1,
    toolCatalogVersion: "agent-tools-v1",
    tools: [],
    grants: [],
  }),
  createGrant: vi.fn(),
  revokeGrant: vi.fn(),
  inspectTransport: vi.fn().mockResolvedValue({
    running: false,
    url: null,
    executablePath: "/Applications/AI Knowledge Sort.app/Contents/MacOS/ai-knowledge-sort",
  }),
  startTransport: vi.fn(),
  stopTransport: vi.fn(),
};

describe("SettingsDialog", () => {
  test("switches between model runtime and Agent access without nested dialogs", async () => {
    render(
      <SettingsDialog
        agentAccessClient={agentClient}
        modelRuntimeClient={modelClient}
        onClose={vi.fn()}
        triggerRef={{ current: null }}
      />,
    );
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByLabelText("Configuration ID")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Agent access" }));
    expect(await screen.findByLabelText("Agent ID")).toBeInTheDocument();
    expect(screen.queryByLabelText("Configuration ID")).toBeNull();
  });

  test("closes with Escape and restores focus to Settings", async () => {
    function Harness() {
      const triggerRef = useRef<HTMLButtonElement>(null);
      const [open, setOpen] = useState(true);
      return (
        <>
          <button ref={triggerRef} type="button">Settings</button>
          {open ? (
            <SettingsDialog
              agentAccessClient={agentClient}
              modelRuntimeClient={modelClient}
              onClose={() => setOpen(false)}
              triggerRef={triggerRef}
            />
          ) : null}
        </>
      );
    }
    render(<Harness />);
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(screen.getByRole("button", { name: "Settings" })).toHaveFocus();
    });
  });
});
