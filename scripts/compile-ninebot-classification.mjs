import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

const EXPECTED_TREE_SHA256 =
  "d6da06749d71bea2e48693528c1c8fb1a7ffb9337294bd25a04f3d7a2295e23d";
const EXPECTED_COUNTS = Object.freeze({ 1: 14, 2: 94, 3: 179, 4: 179 });

const [sourcePath, outputPath] = process.argv.slice(2);
if (!sourcePath || !outputPath || process.argv.length !== 4) {
  throw new Error(
    "Usage: node scripts/compile-ninebot-classification.mjs <classification_tree.md> <output.json>",
  );
}

const sourceBytes = await readFile(sourcePath);
const sourceDigest = createHash("sha256").update(sourceBytes).digest("hex");
if (sourceDigest !== EXPECTED_TREE_SHA256) {
  throw new Error(`Unexpected classification tree SHA-256: ${sourceDigest}`);
}
const lines = sourceBytes.toString("utf8").split(/\r?\n/u);

const l1Labels = new Map();
for (const line of lines) {
  const match = line.match(/^\| (SN-\d{2}) \| ([^|]+?) \|/u);
  if (match) {
    const [, categoryId, title] = match;
    if (l1Labels.has(categoryId)) {
      throw new Error(`Duplicate L1 table row: ${categoryId}`);
    }
    l1Labels.set(categoryId, `${categoryId} ${title.trim()}`);
  }
}
if (l1Labels.size !== EXPECTED_COUNTS[1]) {
  throw new Error(`Expected 14 L1 table rows, found ${l1Labels.size}`);
}

const categories = [];
const categoryById = new Map();
let currentL1 = null;

function addCategory(category) {
  if (categoryById.has(category.categoryId)) {
    throw new Error(`Duplicate category id: ${category.categoryId}`);
  }
  categoryById.set(category.categoryId, category);
  categories.push(category);
}

function parentIdFor(categoryId, depth) {
  if (depth === 2) return currentL1;
  return categoryId.split(".").slice(0, -1).join(".");
}

for (const line of lines) {
  const heading = line.match(/^## (SN-\d{2})_/u);
  if (heading) {
    currentL1 = heading[1];
    const label = l1Labels.get(currentL1);
    if (!label) throw new Error(`Missing L1 table label: ${currentL1}`);
    addCategory({
      categoryId: currentL1,
      label,
      depth: 1,
      parentId: null,
      path: [label],
      aliases:
        currentL1 === "SN-02" ? ["SN-02 IPMS 管理营销闭环"] : [],
    });
    continue;
  }

  const bullet = line.match(/^( {0}| {2}| {4})- `([^`]+)`$/u);
  if (!bullet) continue;
  if (!currentL1) throw new Error(`Category found before L1 heading: ${line}`);

  const depth = bullet[1].length / 2 + 2;
  const label = bullet[2];
  const categoryId = label.split("_", 1)[0];
  const expectedId = new RegExp(`^\\d{2}(?:\\.\\d{2}){${depth - 1}}$`, "u");
  if (!expectedId.test(categoryId)) {
    throw new Error(`Invalid L${depth} category id: ${categoryId}`);
  }
  if (!categoryId.startsWith(`${currentL1.slice(3)}.`)) {
    throw new Error(`${categoryId} is outside ${currentL1}`);
  }
  const parentId = parentIdFor(categoryId, depth);
  const parent = categoryById.get(parentId);
  if (!parent || parent.depth !== depth - 1) {
    throw new Error(`Missing L${depth - 1} parent ${parentId} for ${categoryId}`);
  }
  addCategory({
    categoryId,
    label,
    depth,
    parentId,
    path: [...parent.path, label],
    aliases: [],
  });
}

const counts = categories.reduce(
  (result, category) => ({
    ...result,
    [category.depth]: (result[category.depth] ?? 0) + 1,
  }),
  {},
);
for (const [depth, expected] of Object.entries(EXPECTED_COUNTS)) {
  if (counts[depth] !== expected) {
    throw new Error(`Expected ${expected} L${depth} categories, found ${counts[depth] ?? 0}`);
  }
}

const profile = {
  schemaVersion: 2,
  profileId: "ninebot-electronic-archive",
  version: "0.3.0-draft",
  title: "Ninebot document and electronic archive classification",
  status: "draft",
  provenance: {
    sourceTitle: "九号公司文档与电子档案管理规范（讨论稿 V0.9.0-rc.3）",
    ownership: "firstPartyAuthorized",
    evidence: [
      "authorization:AUTH-2026-07-29-NINEBOT-DRAFT",
      "classificationDraftVersion:0.3.0",
      `sha256:classification_tree.md:${EXPECTED_TREE_SHA256}`,
      "sha256:discussion-html:30d636b9fa7618a86907889ac929a46c256df6be51f70ba86937411526839488",
      "sha256:nimble-kb-org-package:2e34b4b75fc39194a9c609e6df6552a2602563f670c9d23666eb5ca6520e823c",
      "sha256:usage-manual:e166b533213ec42d8c9a58ea05935305235c34ee7adbe998a2b409f73f3525cd",
      "sha256:cross-domain-analysis:3434374e7fb8aaa352f61efa7d81ab0dc42aed937d4a90a314203f327ea4cf9e",
      "status:discussion-draft-not-emt-approved",
    ],
  },
  categories,
  governance: {
    maximumDepth: 4,
    uniquePrimaryArchiveCategory: true,
    semanticEvidenceRequired: true,
    metadataOnlyDimensions: [
      "chineseLibraryClass",
      "businessUnit",
      "productLine",
      "project",
      "ipdStage",
      "productElement",
      "ownerScope",
    ],
    insufficientEvidenceDisposition: "importantIndexed",
    conflictingEvidenceDisposition: "classificationReview",
    archiveFirst: true,
    crossDomainKnowledgeLinks: true,
    independentNodeTriggers: ["highValue", "crossDomain", "userRequested"],
    generatedIndexesLinkOnly: true,
  },
  rules: [],
};

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(profile, null, 2)}\n`, "utf8");
console.log(
  `Wrote ${categories.length} categories (${Object.values(counts).join("/")}) to ${outputPath}`,
);
