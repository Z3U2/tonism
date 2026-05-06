---
name: validate
description: Run typecheck, lint, and tests in parallel, then fix issues until all green
disable-model-invocation: true
---

Run 1 subagent as a preparation task:

1. `pnpm lint:fix`: this does nothing else than executing the command, it doesn't do any other tool calls to investigate more.

Then run 3 QA subagents to run in parallel these quality tasks and compile a concise list of issues to address. These subagents do nothing else than executing the command, they don't perform any other tool call to investigate more.

1. `pnpm typecheck`
2. `pnpm lint`
3. `pnpm test`

Then fix all issues.

When all is fixed, re-run all the above. Repeat until all is green.

For maximum efficiency, whenever you need to perform multiple independent operations, invoke all relevant tools simultaneously rather than sequentially.
