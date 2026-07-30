import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nContext";
import type { DiscoveryProposal } from "../drop/types";
import type { GraphClient } from "../graph/types";
import type { ProfileClient } from "../profiles/types";
import type { ModelRuntimeClient } from "../models/types";
import { ContextPane } from "./ContextPane";
import type { ContextMode } from "./ContextPane";

const proposal: DiscoveryProposal = {
  proposalId: "context-proposal",
  items: [{
    itemId: "context-guide",
    path: "/review/guide.md",
    name: "guide.md",
    byteSize: 1024,
    identity: {
      algorithm: "SHA-256",
      digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0",
    },
  }],
  counts: {
    included: 2,
    excluded: 3,
    unreadable: 1,
    symlink: 4,
    outOfScope: 5,
  },
  diagnostics: [],
};

describe("ContextPane", () => {
  test("renders all context tabs in Simplified Chinese", () => {
    const profileClient = {
      inspect: vi.fn().mockResolvedValue({ installed: [], active: null, candidates: [] }),
      importLocalCandidate: vi.fn(),
      importUrlCandidate: vi.fn(),
      compileLocalCandidate: vi.fn(),
      decideCandidate: vi.fn(),
      createClassificationBatch: vi.fn(),
    } as ProfileClient;
    const graphClient = {
      inspect: vi.fn(),
      propose: vi.fn(),
      importComparison: vi.fn(),
      decide: vi.fn(),
    } as GraphClient;
    const modelRuntimeClient = {
      inspect: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
    } as unknown as ModelRuntimeClient;

    render(
      <I18nProvider initialLanguage="zh-CN">
        <ContextPane
          collapsed={false}
          document={null}
          graphClient={graphClient}
          mode="graph"
          modelRuntimeClient={modelRuntimeClient}
          onCollapsedChange={vi.fn()}
          onModeChange={vi.fn()}
          profileClient={profileClient}
          proposal={proposal}
        />
      </I18nProvider>,
    );

    expect(screen.getByRole("tab", { name: "知识图谱" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "导入审查" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Agent 审查" })).toBeInTheDocument();
    expect(screen.getByText("尚未入库")).toBeInTheDocument();
  });

  test("keeps graph and governed profile review as separate right-pane tabs", async () => {
    const profileClient: ProfileClient = {
      inspect: vi.fn().mockResolvedValue({
        installed: [],
        active: null,
        candidates: [],
      }),
      importLocalCandidate: vi.fn(),
      importUrlCandidate: vi.fn(),
      compileLocalCandidate: vi.fn(),
      decideCandidate: vi.fn(),
      createClassificationBatch: vi.fn(),
    };
    const graphClient: GraphClient = {
      inspect: vi.fn(),
      propose: vi.fn(),
      importComparison: vi.fn(),
      decide: vi.fn(),
    };
    const modelRuntimeClient: ModelRuntimeClient = {
      inspect: vi.fn().mockResolvedValue({ schemaVersion: 1, configs: [] }),
      discoverModels: vi.fn(),
      upsert: vi.fn(),
      remove: vi.fn(),
      runComparison: vi.fn(),
      runFileSemanticComparison: vi.fn(),
    };
    function Harness() {
      const [mode, setMode] = useState<ContextMode>("graph");
      return (
        <ContextPane
          collapsed={false}
          document={null}
          graphClient={graphClient}
          mode={mode}
          modelRuntimeClient={modelRuntimeClient}
          onCollapsedChange={vi.fn()}
          onModeChange={setMode}
          profileClient={profileClient}
          proposal={proposal}
        />
      );
    }

    render(<Harness />);

    expect(screen.getByRole("tab", { name: "Knowledge Graph" }))
      .toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Import Review" }));
    expect(
      await screen.findByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("No candidate awaiting review"))
      .toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Agent Review" }));
    expect(await screen.findByText(/saved Vault revision is required/i))
      .toBeInTheDocument();
  });
});
