import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test("loads the review-only workbench and supports tri-state source selection", async ({
  page,
}) => {
  await expect(
    page.getByRole("main", { name: "Source workbench" }),
  ).toBeVisible();
  await expect(page.getByText("Demo scan")).toBeVisible();
  await expect(
    page.getByRole("banner", { name: "Application header" }),
  ).toContainText("AI Knowledge Sort");
  await expect(
    page.getByRole("region", { name: "Archive preview" }),
  ).toContainText("Uncommitted");

  const workspace = page.getByRole("checkbox", {
    name: "Select Local workspace directory",
  });
  const excluded = page.getByRole("checkbox", {
    name: "Select reference.zip file",
  });

  await expect(workspace).not.toBeChecked();
  await expect(excluded).toBeDisabled();

  await workspace.check();
  await expect(workspace).toBeChecked();
  await page
    .getByRole("checkbox", { name: "Select Roadmap.md file" })
    .uncheck();
  await expect(workspace).toHaveJSProperty("indeterminate", true);
  await expect(
    page.getByRole("checkbox", { name: "Select Roadmap.md file" }),
  ).not.toBeChecked();
  await workspace.check();
  await expect(
    page.getByRole("checkbox", { name: "Select Roadmap.md file" }),
  ).toBeChecked();
});

test("shows all five proposal counts with no fake mutation action", async ({
  page,
}) => {
  for (const [name, count] of [
    ["Included", "3"],
    ["Excluded", "2"],
    ["Unreadable", "1"],
    ["Symlinks", "1"],
    ["Out of scope", "1"],
  ] as const) {
    await expect(page.getByRole("status", { name })).toContainText(count);
  }

  await expect(page.getByText("No files have been changed")).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Proposal topology" }),
  ).toContainText("Not yet ingested");
  await expect(
    page.getByRole("button", { name: "Play knowledge timeline" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: /import files/i }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: /^(import files|move files|archive files|write files)$/i,
    }),
  ).toHaveCount(0);
});

test("keeps archive operations honest in the browser fixture", async ({
  page,
}) => {
  const archive = page.getByRole("region", { name: "Archive preview" });

  await archive.getByRole("button", { name: "Choose Vault" }).click();

  await expect(archive.getByRole("alert")).toContainText(
    "Desktop runtime is required",
  );
  await expect(archive).toContainText("Uncommitted");
  await expect(archive).not.toContainText("Archive committed");
  await expect(page.getByText("Local draft · not saved")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Create knowledge note" }),
  ).toHaveCount(0);
  await expect(page.getByText(/Saved revision/)).toHaveCount(0);
});

test("keeps canonical naming non-mutating in the browser fixture", async ({
  page,
}) => {
  const archive = page.getByRole("region", { name: "Archive preview" });
  await archive
    .getByRole("checkbox", { name: "Include meeting-notes.md" })
    .check();
  await archive
    .getByRole("textbox", { name: "Subject for meeting-notes.md" })
    .fill("Meeting notes");
  await archive
    .getByRole("textbox", {
      name: "Evidence location for meeting-notes.md",
    })
    .fill("section:summary");

  await archive
    .getByRole("button", { name: "Review canonical names" })
    .click();

  await expect(archive.getByRole("alert")).toContainText(
    "Desktop runtime is required for naming operations.",
  );
  await expect(archive).toContainText("meeting-notes.md");
  await expect(
    archive.getByRole("region", { name: "Exact archive plan" }),
  ).toHaveCount(0);
  await expect(page.getByText("3 eligible · 0 changes")).toBeVisible();
});

test("keeps profile import and activation honest in the browser fixture", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Import Review" }).click();

  await expect(page.getByText("Ninebot electronic archive")).toBeVisible();
  await expect(page.getByText("DRAFT", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Import local profile" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Desktop runtime is required for profile operations.",
  );
  await expect(page.getByText(/Approved and active/)).toHaveCount(0);
  await expect(page.getByText("3 eligible · 0 changes")).toBeVisible();
});

test("collapses and restores the adjustable side panes", async ({ page }) => {
  await expect(
    page.getByRole("separator", { name: "Resize Sources panel" }),
  ).toBeVisible();
  await expect(
    page.getByRole("separator", { name: "Resize import review context" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Collapse Sources panel" }).click();
  await page
    .getByRole("button", { name: "Collapse Import review context" })
    .click();
  await expect(
    page.getByRole("button", { name: "Expand Sources panel" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Expand Import review context" }),
  ).toBeVisible();
});

test("keeps the document workspace usable at a narrower viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 700, height: 760 });

  await expect(
    page.getByRole("region", { name: "Document workspace" }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Sources" }),
  ).toBeHidden();
  await expect(
    page.getByRole("textbox", { name: "Markdown, Mermaid, and code editor" }),
  ).toBeVisible();
  await expect(page.getByRole("tab", { name: "Source" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Live preview" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "Reading" })).toBeVisible();
});

test("preserves one mixed draft across all document modes", async ({ page }) => {
  const draft =
    '# Browser note\n\nRendered safely.\n\n```mermaid\ngraph TD\nclick A href "https://example.com"\n```';
  const editor = page.getByRole("textbox", {
    name: "Markdown, Mermaid, and code editor",
  });
  await editor.fill(draft);

  await page.getByRole("tab", { name: "Live preview" }).click();
  await expect(editor).toHaveValue(draft);
  await expect(
    page.getByRole("region", { name: "Document preview" }),
  ).toContainText("Browser note");
  await expect(
    page.getByRole("alert", { name: "Mermaid diagnostic" }),
  ).toContainText("click directives are disabled");

  await page.getByRole("tab", { name: "Reading" }).click();
  await expect(editor).toHaveCount(0);
  await expect(
    page.getByRole("region", { name: "Document preview" }),
  ).toContainText("Rendered safely");

  await page.getByRole("tab", { name: "Source" }).click();
  await expect(
    page.getByRole("textbox", {
      name: "Markdown, Mermaid, and code editor",
    }),
  ).toHaveValue(draft);

  const validDraft =
    "# Valid diagram\n\n```mermaid\nflowchart LR\nSource --> Review --> Archive\n```";
  await page
    .getByRole("textbox", {
      name: "Markdown, Mermaid, and code editor",
    })
    .fill(validDraft);
  await page.getByRole("tab", { name: "Live preview" }).click();
  await expect(
    page.getByRole("img", { name: "Rendered Mermaid diagram" }),
  ).toBeVisible();
  await expect(page.getByText("Mermaid source")).toBeVisible();
});
