import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ProfileClient, ProfileStateSummary } from "./types";
import { ProfileReview } from "./ProfileReview";

const digest = "a".repeat(64);
const state: ProfileStateSummary = {
  installed: [{
    profileId: "ninebot-electronic-archive",
    version: "0.1.0-draft",
    title: "Ninebot electronic archive",
    status: "draft",
    ruleCount: 0,
    provenanceTitle: "AI Knowledge Sort clean implementation handoff",
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
  };
}

describe("ProfileReview", () => {
  test("shows the bundled draft and requires an exact candidate decision", async () => {
    const profileClient = client();
    render(<ProfileReview client={profileClient} />);

    expect(
      await screen.findByText("Ninebot electronic archive"),
    ).toBeInTheDocument();
    expect(screen.getByText("DRAFT")).toBeInTheDocument();
    expect(screen.getByText("rule-9")).toBeInTheDocument();
    expect(screen.getByText(/0 rules — classification disabled/))
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

  test("keeps browser limitations visible without hiding the bundled draft", async () => {
    const unavailable: ProfileClient = {
      inspect: vi.fn().mockRejectedValue(
        new Error("Desktop runtime is required for profile operations."),
      ),
      importLocalCandidate: vi.fn(),
      importUrlCandidate: vi.fn(),
      decideCandidate: vi.fn(),
    };
    render(<ProfileReview client={unavailable} />);

    expect(screen.getByText("Ninebot electronic archive")).toBeInTheDocument();
    expect(
      await screen.findByText(
        "Desktop runtime is required for profile operations.",
      ),
    ).toBeInTheDocument();
  });
});
