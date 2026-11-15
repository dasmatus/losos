# Goal

Fix the on-highlight (quick-actions) bar not showing when text is highlighted in LosOS Office and in LosOS Slides, and add right-click actions relevant to LosOS Office and LosOS Slides.

## Milestones

1. Reproduce and inspect: ran office-markdown and quick-actions browser tests. Status: verified
2. Root cause: read view (document and slide) lacked ReaderQuickActions, unlike Notes. Status: verified
3. Fix: added ReaderQuickActions to MarkdownOffice read side and slide stage. Status: verified
4. Typecheck and browser tests pass. Status: verified

## Key decisions

- Reused existing ReaderQuickActions (same as Notes) for read-side selection bar.
- No new right-click actions added beyond existing page-level context menu (Copy etc.); bar covers selection actions.

## Blockers

- (none)

## Completed

- Evidence: `bun --bun tsc --noEmit` passed; `office-markdown.browser.ts` all checks passed; `quick-actions.browser.ts` all checks passed.
