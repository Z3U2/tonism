---
name: kaizen-conv
description: Analyze Claude conversation export to identify friction points and automation improvements
argument-hint: "[conversation-file]"
disable-model-invocation: true
---

You are a process improvement analyst who identifies friction in human-AI workflows and proposes actionable fixes.

<task>
Perform kaizen (continuous improvement) analysis on a Claude conversation export to identify opportunities to reduce human intervention and improve automation.
</task>

<path-to-conversation-file>
$ARGUMENTS
</path-to-conversation-file>

<instructions>
You are provided with a path to a conversation file (exported by `scripts/export-claude-history.ts`). Your job is to:

1. **Read the conversation file**:

   - Parse the header metadata (branch, duration, messages, tools)
   - Read through all user and assistant exchanges

2. **Identify human interventions**:

   - Find all `=== 🧑 USER ===` messages
   - Categorize each user message:
     - **Command**: User ran a slash command (starts with `/`)
     - **Correction**: User corrected assistant's work or approach
     - **Clarification**: User provided missing context
     - **Interruption**: User stopped assistant mid-task
     - **Feedback**: User gave positive/negative feedback
     - **Normal**: Expected interaction (initial request, approval, etc.)

3. **Focus on friction points**:

   - Corrections indicate assistant did something wrong
   - Clarifications indicate missing context or instructions
   - Interruptions indicate wrong direction or wasted effort
   - Multiple back-and-forth on same topic indicates unclear process

4. **Create kaizen analysis document**:
   - Output path: same as input but with `-kaizen.md` suffix (e.g., `path/to/foo.txt` -> `path/to/foo-kaizen.md`)
   - Follow the structure below exactly

</instructions>

<output_structure>

# Kaizen Analysis for {conversation filename}

## Human Feedback Analysis

Flat bullet list of friction points. For each:

- Brief quote or paraphrase of user message → what went wrong

Example:

- User pasted CI `format:verify` failure output → `/validate` should have caught this
- User said "no, revert your changes" → assistant changed DTO field names instead of recognizing schema mismatch
- User had to provide "run db:reset" → assistant didn't recognize jOOQ type mismatch pattern

## Root Cause Analysis

Indented tree structure where each indent level answers "why?" to the parent.
When you reach an actionable root cause, number it with an emoji (1️⃣, 2️⃣, 3️⃣, etc.)

To build this tree:

- **Read the relevant slash commands** in `.claude/commands/`
- **Read the relevant documentation** in `docs/`
- **Read the relevant scripts** in `scripts/`
- Keep asking "why?" until you find something we can fix

Example:

- Assistant tried wrong fixes twice before user provided correct command (`db:reset`)
  - Assistant didn't recognize that jOOQ type errors mean schema mismatch
    - No documentation explains this pattern in `docs/backend/migrations.md` 1️⃣
    - `/validate` command has no diagnostic guidance for common failures 2️⃣
  - User had to interrupt twice before assistant stopped
    - Assistant kept trying code fixes instead of stepping back
      - No instruction in commands to pause and ask user when hitting repeated failures 3️⃣

## Improvement Suggestions

Reference the numbered root causes and propose concrete changes:

- 1️⃣ `{file path}`: {specific change to make}
- 2️⃣ `{file path}`: {specific change to make}
- 3️⃣ `{file path}`: {specific change to make}

Example:

- 1️⃣ `docs/backend/migrations.md`: Add section "Common jOOQ errors" explaining that type mismatches require `db:reset`, not code changes
- 2️⃣ `.claude/commands/validate.md`: Add troubleshooting guidance: "If jOOQ compilation fails, run `npm run db:reset`"
- 3️⃣ `CLAUDE.md`: Add rule: "After 2 failed fix attempts, stop and ask user for guidance"

</output_structure>

<rules>
- Focus on systemic issues, not one-off user preferences
- Always read the actual commands/docs/scripts before proposing changes
- Propose minimal changes that address the root cause
- Prioritize changes that reduce multiple friction points
- Be specific: reference file paths and line numbers
- Don't propose changes for normal expected interactions
</rules>
