export interface CommentInput {
  platform: string;
  changed: string[];
  skipped: boolean;
  skipReason?: string;
  pushed: boolean;
  owner: string;
  repo: string;
  baseSha: string;
  headSha: string;
}

export function rawUrl(
  owner: string,
  repo: string,
  sha: string,
  path: string,
): string {
  return `https://raw.githubusercontent.com/${owner}/${repo}/${sha}/${path}`;
}

export function buildPrScreenshotComment(input: CommentInput): string {
  const {
    platform,
    changed,
    skipped,
    skipReason = "",
    pushed,
    owner,
    repo,
    baseSha,
    headSha,
  } = input;
  if (skipped) {
    return [
      `### Handbook screenshots (${platform})`,
      "",
      `Skipped: ${skipReason}`,
      "",
      `Artifact: \`handbook-screens-${platform}\` (if produced).`,
    ].join("\n");
  }
  if (changed.length === 0) {
    return `### Handbook screenshots (${platform})\n\nNo changed screenshots.`;
  }
  if (pushed) {
    const rows = changed.map((f) => {
      const p = `handbook/docs/img/${platform}/${f}`;
      return `| \`${f}\` | ![${f} before](${rawUrl(owner, repo, baseSha, p)}) | ![${f} after](${rawUrl(owner, repo, headSha, p)}) |`;
    });
    return [
      `### Handbook screenshots (${platform})`,
      "",
      "<details>",
      "<summary>Before / after (changed screenshots)</summary>",
      "",
      "| Screen | Before | After |",
      "| --- | --- | --- |",
      ...rows,
      "",
      `Artifact fallback: \`handbook-screens-${platform}\`.`,
      "",
      "</details>",
    ].join("\n");
  }
  return [
    `### Handbook screenshots (${platform})`,
    "",
    "<details>",
    "<summary>Changed screenshots (images not embedded)</summary>",
    "",
    "The bot could not push rendered screenshots to the PR branch (fork PR or ruleset), so images are not embedded here.",
    "",
    ...changed.map((f) => `- \`${f}\``),
    "",
    `Artifact: \`handbook-screens-${platform}\`.`,
    "",
    "</details>",
  ].join("\n");
}

export function buildMainScreenshotPrBody(input: {
  platform: string;
  changed: string[];
  headSha: string;
  sourceSha: string;
  owner: string;
  repo: string;
}): string {
  const { platform, changed, headSha, sourceSha, owner, repo } = input;
  return [
    `### Handbook screenshots (${platform})`,
    "",
    `Automated refresh from the \`handbook-screens\` workflow for commit ${sourceSha}.`,
    "",
    ...changed.flatMap((f) => [
      `![${platform}/${f}](${rawUrl(owner, repo, headSha, `handbook/docs/img/${platform}/${f}`)})`,
      "",
    ]),
    "Merging lands the images the workflow could not push to main directly.",
  ].join("\n");
}
