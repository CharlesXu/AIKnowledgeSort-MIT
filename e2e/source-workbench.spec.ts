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
});
