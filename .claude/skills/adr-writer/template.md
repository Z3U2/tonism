# {Title}

Owner: {Owner}

Last update: {Date DD/MM/YYYY}

## Context

---

{10 lines max. Describe:

- The problem being solved
- Current state (brief - details go in Appendices)
- What triggered the need for this decision

Do NOT include constraints (those inform evaluation) or mention candidate technologies (stay neutral).}

## Selection criteria

---

| #   | Criteria         | Why this criteria ?                                                                 |
| --- | ---------------- | ----------------------------------------------------------------------------------- |
| 1   | {Criterion name} | {Business/project REASON for caring about this - explains WHY, not WHAT to measure} |
| 2   | {Criterion name} | {Justification}                                                                     |
| 3   | {Criterion name} | {Justification}                                                                     |

Criteria are numbered by priority (1 = most important). This order is reflected in the Evaluation table columns.

## Evaluation

---

| Solution         | 1. {Criterion}                                               | 2. {Criterion}              | 3. {Criterion} |
| ---------------- | ------------------------------------------------------------ | --------------------------- | -------------- |
| **{Solution 1}** | 🟢🟢🟢<br>- Justification point<br>- [Source](url) if needed | 🟠🟠<br>- Justification     | ...            |
| **{Solution 2}** | 🟢🟢<br>- Justification                                      | 🟢🟢🟢🟢<br>- Justification | ...            |
| **{Solution 3}** | 🔴<br>- Why it fails                                         | 🟢🟢🟢<br>- Justification   | ...            |

### Scoring Legend

**Scale notation (for non-binary criteria):**

- 🟢🟢🟢🟢 = Excellent (4/4)
- 🟢🟢🟢 = Very Good (3/4)
- 🟢🟢 = Good (2/4)
- 🟠🟠 = Medium (2/4 with reservations)
- 🟠 = Poor (1/4)
- 🔴 = Bad / Does not meet criterion (0/4)

**Binary notation (for yes/no criteria):**

- ✅ = Yes / Meets criterion
- ❌ = No / Does not meet criterion
- ⚠️ = Partial / With caveats

## Recommendation

---

{5-8 lines max. Include:

- Clear statement of the recommended solution
- 2-3 key reasons (don't repeat the evaluation table)
- Brief acknowledgment of the main tradeoff accepted}

## Appendices

---

{Optional but recommended when Context references existing systems. Include:

- Schema/table definitions (columns, types, relationships)
- API signatures or data structures
- Code snippets showing current implementation patterns
- Supporting research that didn't fit in evaluation cells}
