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
    page.getByRole("button", { name: "Add relation" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("region", { name: "Relation evidence" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("img", { name: "Knowledge network" }),
  ).toHaveCount(0);
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

test("keeps classification and canonical naming non-mutating in the browser fixture", async ({
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
    .getByRole("textbox", {
      name: "Classification evidence for meeting-notes.md",
    })
    .fill("Meeting notes semantic evidence");

  await archive
    .getByRole("button", { name: "Review classification" })
    .click();

  await expect(archive.getByRole("alert")).toContainText(
    "Desktop runtime is required for profile operations.",
  );
  await expect(
    archive.getByRole("button", { name: "Review canonical names" }),
  ).toBeDisabled();
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

  await expect(
    page.getByText(
      "Ninebot document and electronic archive classification",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(page.getByText("DRAFT", { exact: true })).toBeVisible();
  await expect(page.getByText("466 categories · 14 / 94 / 179 / 179"))
    .toBeVisible();
  await expect(
    page.getByText("0 executable rules — semantic review required"),
  ).toBeVisible();
  await expect(
    page.getByText("Discussion draft — not approved or active"),
  ).toBeVisible();
  const url = page.getByRole("textbox", { name: "Profile URL" });
  await url.fill(
    "https://profiles.example.com/ninebot.json?signature=synthetic-secret#review",
  );
  await page.getByRole("button", { name: "Import URL" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Desktop runtime is required for profile operations.",
  );
  await expect(url).toHaveValue("");
  await expect(page.getByText("synthetic-secret")).toHaveCount(0);
  await expect(page.getByText(/Remote URL ·/)).toHaveCount(0);
  await expect(page.getByText(/SHA-256/)).toHaveCount(0);
  await page.getByRole("button", { name: "Import local profile" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Desktop runtime is required for profile operations.",
  );
  const compile = page.getByRole("button", { name: "Compile local source" });
  await expect(compile).toBeDisabled();
  await page.getByRole("textbox", { name: "Model configuration ID" })
    .fill("local-compiler");
  await page.getByRole("textbox", { name: "Candidate version" })
    .fill("0.4.0-candidate");
  await page.getByRole("textbox", { name: "Source title" })
    .fill("Formal notice");
  await expect(compile).toBeEnabled();
  await compile.click();
  await expect(page.getByRole("alert")).toContainText(
    "Desktop runtime is required for profile operations.",
  );
  await expect(page.getByText("Model generated")).toHaveCount(0);
  await expect(page.getByText(/Approved and active/)).toHaveCount(0);
  await expect(page.getByText("3 eligible · 0 changes")).toBeVisible();
});

test("keeps model Settings secret-free and non-persistent in the browser fixture", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("tab", { name: "Model runtime" }))
    .toHaveAttribute("aria-selected", "true");
  await expect(dialog.getByRole("alert")).toContainText(
    "Desktop runtime is required for model runtime operations.",
  );
  await expect(dialog.getByLabel(/api key|password|secret/i)).toHaveCount(0);

  await dialog.getByLabel("Configuration ID").fill("browser-model");
  await dialog.getByLabel("Label").fill("Browser Model");
  await dialog.getByRole("textbox", { name: "Model", exact: true })
    .fill("browser-only");
  await dialog.getByRole("button", { name: "Save model config" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Desktop runtime is required for model runtime operations.",
  );
  await expect(dialog.getByText("No model configurations.")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Edit Browser Model" }))
    .toHaveCount(0);
});

test("keeps Agent access local, unissued, and non-mutating in the browser fixture", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("tab", { name: "Agent access" }).click();

  await expect(dialog.getByRole("alert")).toContainText(
    "Desktop runtime is required for Agent access operations.",
  );
  await dialog.getByRole("button", { name: "Choose directories" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Desktop runtime is required for Agent access operations.",
  );
  await expect(dialog.getByText("No Agent grants.")).toBeVisible();
  await expect(dialog.getByText(/one-time grant token/i)).toHaveCount(0);
  await expect(dialog.getByText("STOPPED", { exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Start local MCP" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Desktop runtime is required for Agent access operations.",
  );
  await expect(
    dialog.getByRole("region", { name: "Local MCP broker" })
      .getByText(/http:\/\/127\.0\.0\.1:/),
  ).toHaveCount(0);
  await expect(dialog.getByLabel("Direct HTTP configuration")).toHaveCount(0);
  await expect(dialog.getByLabel("stdio relay configuration")).toHaveCount(0);
  await expect(dialog.getByRole("button", {
    name: /cleanup execution|move|rename|delete|archive commit/i,
  })).toHaveCount(0);
});

test("never fabricates Agent comparison or mutation actions for a browser draft", async ({
  page,
}) => {
  await page.getByRole("tab", { name: "Agent Review" }).click();
  await expect(page.getByText(/saved Vault revision is required/i)).toBeVisible();
  await expect(page.getByRole("button", { name: "Run comparison" })).toHaveCount(0);
  await expect(
    page.getByRole("region", { name: "Model comparison result" }),
  ).toHaveCount(0);
  await expect(page.getByText(/Semantic advice · no operation authorized/))
    .toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: /apply|move|rename|delete|cleanup|write graph/i,
    }),
  ).toHaveCount(0);
});

test("drags, collapses, and restores the adjustable side panes", async ({ page }) => {
  const sourceSeparator = page.getByRole("separator", {
    name: "Resize Sources panel",
  });
  await expect(sourceSeparator).toBeVisible();
  await expect(
    page.getByRole("separator", { name: "Resize import review context" }),
  ).toBeVisible();

  const sourceBox = await sourceSeparator.boundingBox();
  expect(sourceBox).not.toBeNull();
  await page.mouse.move(
    sourceBox!.x + sourceBox!.width / 2,
    sourceBox!.y + sourceBox!.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(
    sourceBox!.x + sourceBox!.width / 2 + 60,
    sourceBox!.y + sourceBox!.height / 2,
  );
  await page.mouse.up();
  await expect(sourceSeparator).toHaveAttribute("aria-valuenow", "308");

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
