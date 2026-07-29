import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ProfileClient, ProfileStateSummary } from "./types";
import { ProfileReview } from "./ProfileReview";

const digest = "a".repeat(64);
const state: ProfileStateSummary = {
  installed: [{
    profileId: "ninebot-electronic-archive",
    version: "0.3.0-draft",
    title: "Ninebot document and electronic archive classification",
    status: "draft",
    ruleCount: 0,
    categoryCount: 466,
    taxonomyCounts: {
      level1: 14,
      level2: 94,
      level3: 179,
      level4: 179,
    },
    semanticEvidenceRequired: true,
    uniquePrimaryArchiveCategory: true,
    crossDomainKnowledgeLinks: true,
    provenanceTitle: "九号公司文档与电子档案管理规范（讨论稿 V0.9.0-rc.3）",
  }],
  active: null,
  candidates: [{
    schemaVersion: 1,
    candidateId: "candidate-1",
    importedAtUnixMs: 1_785_245_600_000,
    sourceKind: "localFile",
    sourceBasename: "classification-profile.json",
    sourceByteSize: 1_024,
    locatorIdentity: {
      algorithm: "SHA-256",
      digest: "b".repeat(64),
    },
    sourceIdentity: {
      algorithm: "SHA-256",
      digest,
    },
    profileId: "ninebot-electronic-archive",
    profileVersion: "1.0.0",
    status: "unapproved",
    base: null,
    diff: {
      addedRuleIds: ["rule-9"],
      removedRuleIds: [],
      changedRuleIds: [],
      addedCategoryIds: Array.from(
        { length: 466 },
        (_, index) => `category-${index + 1}`,
      ),
      removedCategoryIds: [],
      changedCategoryIds: [],
    },
    approval: null,
  }],
};

function client(): ProfileClient {
  return {
    inspect: vi.fn().mockResolvedValue(state),
    importLocalCandidate: vi.fn().mockResolvedValue(state.candidates[0]),
    importUrlCandidate: vi.fn().mockResolvedValue(state.candidates[0]),
    decideCandidate: vi.fn().mockResolvedValue({
      ...state,
      active: {
        profileId: "ninebot-electronic-archive",
        version: "1.0.0",
      },
      candidates: [{
        ...state.candidates[0],
        status: "approved",
      }],
    }),
    createClassificationBatch: vi.fn(),
  };
}

describe("ProfileReview", () => {
  test("shows the bundled draft and requires an exact candidate decision", async () => {
    const profileClient = client();
    render(<ProfileReview client={profileClient} />);

    expect(
      await screen.findByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("DRAFT")).toBeInTheDocument();
    expect(screen.getByText("rule-9")).toBeInTheDocument();
    expect(screen.getByText("0 executable rules — semantic review required"))
      .toBeInTheDocument();
    expect(screen.getByText("466 categories · 14 / 94 / 179 / 179"))
      .toBeInTheDocument();
    expect(screen.getByText("One primary archive category"))
      .toBeInTheDocument();
    expect(screen.getByText("Cross-domain knowledge links"))
      .toBeInTheDocument();
    expect(screen.getByText("Discussion draft — not approved or active"))
      .toBeInTheDocument();
    expect(screen.getByText("Taxonomy +466 · ~0 · −0"))
      .toBeInTheDocument();

    const approve = screen.getByRole("button", { name: "Approve profile" });
    expect(approve).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", {
      name: /reviewed SHA-256 digest/i,
    }));
    await waitFor(() => expect(approve).toBeEnabled());
    fireEvent.click(approve);

    await waitFor(() => {
      expect(profileClient.decideCandidate).toHaveBeenCalledWith({
        candidateId: "candidate-1",
        reviewedDigest: digest,
        decision: "approve",
      });
    });
    expect(await screen.findByText("Approved and active · 1.0.0"))
      .toBeInTheDocument();
  });

  test("imports a URL into review without retaining or displaying the locator", async () => {
    const remoteState: ProfileStateSummary = {
      ...state,
      candidates: [{
        ...state.candidates[0],
        candidateId: "remote-candidate",
        sourceKind: "remoteUrl",
        sourceBasename: "remote-profile.json",
        sourceByteSize: 2_048,
      }],
    };
    const profileClient = client();
    vi.mocked(profileClient.inspect)
      .mockResolvedValueOnce({ ...state, candidates: [] })
      .mockResolvedValueOnce(remoteState);
    vi.mocked(profileClient.importUrlCandidate)
      .mockResolvedValue(remoteState.candidates[0]);
    render(<ProfileReview client={profileClient} />);
    await screen.findByText("No candidate awaiting review");

    const input = screen.getByRole("textbox", { name: "Profile URL" });
    fireEvent.change(input, {
      target: {
        value: "https://profiles.example.com/remote-profile.json?signature=synthetic-secret",
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import URL" }));

    await waitFor(() => {
      expect(profileClient.importUrlCandidate).toHaveBeenCalledWith(
        "https://profiles.example.com/remote-profile.json?signature=synthetic-secret",
      );
    });
    expect(input).toHaveValue("");
    expect(await screen.findByText("Remote URL · 2,048 bytes"))
      .toBeInTheDocument();
    expect(screen.getByText(/SHA-256 a{12}/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve profile" }))
      .toBeDisabled();
    expect(document.body).not.toHaveTextContent("profiles.example.com");
    expect(document.body).not.toHaveTextContent("synthetic-secret");
  });

  test("clears a rejected URL attempt without rendering its secret", async () => {
    const profileClient = client();
    vi.mocked(profileClient.inspect)
      .mockResolvedValue({ ...state, candidates: [] });
    vi.mocked(profileClient.importUrlCandidate).mockRejectedValue(
      new Error("Remote profile target is not allowed"),
    );
    render(<ProfileReview client={profileClient} />);
    await screen.findByText("No candidate awaiting review");
    const input = screen.getByRole("textbox", { name: "Profile URL" });
    fireEvent.change(input, {
      target: {
        value: "https://private.example/profile.json?token=synthetic-secret#secret",
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import URL" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Remote profile target is not allowed",
    );
    expect(input).toHaveValue("");
    expect(document.body).not.toHaveTextContent("synthetic-secret");
  });

  test("keeps browser limitations visible without hiding the bundled draft", async () => {
    const unavailable: ProfileClient = {
      inspect: vi.fn().mockRejectedValue(
        new Error("Desktop runtime is required for profile operations."),
      ),
      importLocalCandidate: vi.fn(),
      importUrlCandidate: vi.fn(),
      decideCandidate: vi.fn(),
      createClassificationBatch: vi.fn(),
    };
    render(<ProfileReview client={unavailable} />);

    expect(
      screen.getByText(
        "Ninebot document and electronic archive classification",
      ),
    ).toBeInTheDocument();
    expect(
      await screen.findByText(
        "Desktop runtime is required for profile operations.",
      ),
    ).toBeInTheDocument();
  });
});
