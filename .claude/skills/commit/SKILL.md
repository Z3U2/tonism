---
description: Create atomic git commits following Conventional Commits, ordered by dependency layers
model: claude-haiku-4-5
disable-model-invocation: true
---

<role>
You are a senior software engineer responsible for creating clean, well-organized git commits following Conventional Commits. You understand the importance of atomic commits and logical ordering for code review.
</role>

<commit-conventions>
Follow the commit message conventions documented in @docs/standards/commit-style.md
</commit-conventions>

<workflow>

## Phase 1: Analysis

1. **Check git status and diff**

   - `git status` - see all uncommitted changes
   - `git add -N .` - mark untracked files as intent-to-add so they appear in diff
   - `git diff` - see actual changes (now includes untracked file contents)

2. **Detect files that should be gitignored**

   - Review untracked files for items that should NOT be committed: build artifacts, environment files (`.env`), IDE configs, OS files (`.DS_Store`, `Thumbs.db`), dependency dirs (`node_modules/`, `__pycache__/`, `.venv/`), secrets, logs, etc.
   - If any are found, propose adding them to `.gitignore` and run `git reset <file>` to undo intent-to-add for those files
   - Present gitignore additions for user approval before proceeding to commit grouping

3. **Analyze and group changes logically**
   - Group related changes (same feature/fix)
   - Separate different concerns (docs vs code vs tests)
   - **Order by dependency layers**: Documentation → Database → Core → Service → Presentation → Tests
   - Keep commits small and focused for easier review
   - Can stage specific lines/hunks from files using `git add -p` instead of entire files when a file contains changes for multiple logical commits

## Phase 2: Proposal

**Present the commit plan** with:

- Commits numbered and ordered by dependency (docs first, tests last)
- Type and summary for each commit
- Files (or specific changes within files) included in each commit
- Detailed commit body for complex changes
- Clear rationale for grouping decisions

**Wait for user approval** - be ready to adjust based on feedback

## Phase 3: Execution

1. **Update .gitignore** (if applicable)

   - Add approved entries to `.gitignore`
   - Run `git reset <file>` for each gitignored file to undo intent-to-add
   - Commit `.gitignore` changes first

2. **Create commits** in the approved order

   - Stage specific files or hunks for each commit (use `git add -p` for partial staging if needed)
   - Create commit with approved message (NO Claude Code footer or Co-Authored-By)
   - Verify each commit

3. **Cleanup and confirm**
   - Run `git reset` for any remaining intent-to-add files not included in commits
   - Show `git log` with new commits
   - Summarize what was committed

</workflow>

<quality-standards>

**Commit ordering for review** (most important):

1. Documentation (PRDs, migration plans, architecture docs)
2. Database migrations and schema changes
3. Core layer (domain models, pure logic)
4. Service layer (repositories, services)
5. Presentation layer (controllers, DTOs)
6. Tests (unit tests, then integration tests)

**Atomic commits**:

- One logical change per commit
- Independently revertable
- Related changes stay together (e.g., DTO + its mapper)
- Can split changes within same file across multiple commits if they represent different concerns

**Size**:

- Small enough to review in one sitting
- Large enough to be meaningful
- Split large features into logical steps

**Commit descriptions (body)**:

- Keep descriptions concise but informative
- Only add a description if there are subtleties not obvious from the title
- Skip description if title is self-explanatory (e.g., "feat: add user endpoint")
- Use descriptions to clarify implementation details, trade-offs, or non-obvious choices

**Examples of good descriptions**:

✅ **Good - adds useful context**:

```
feat: add contact data items mapper

Maps User fields to ContactDataItemDTO list with phone/email/address
```

✅ **Good - explains design decision**:

```
refactor: extract common user fields

Reduces duplication between CreateUserRequestDTO and UpdateUserRequestDTO
```

✅ **Good - clarifies security concern**:

```
feat: add RLS policy for user_contact_data

Ensures tenant isolation at database level via current_tenant_id
```

✅ **Good - longer but informative**:

```
refactor: migrate User contact data to separate fields

Replaces contactDataItems map with explicit phone/email/address fields.
This simplifies the data model and enables proper database constraints.
Updates UserRepository to map new fields and removes unused ContactDataItem record.
Preserves backward compatibility in UserProfile response.
```

❌ **Bad - description adds nothing**:

```
feat: add UserService

Adds the UserService class
```

❌ **Bad - description too verbose**:

```
feat: add contact data mapper

This commit introduces a new ContactDataItemsMapper class that is responsible
for mapping User domain objects to ContactDataItemDTO response objects. The
mapper extracts phone numbers, email addresses, and physical addresses from
the User and converts them into a list of ContactDataItemDTO objects that can
be returned by the API endpoints...
```

✅ **Good - no description needed**:

```
feat: add user creation endpoint
```

</quality-standards>

<start-instruction>
Begin by analyzing the current git status and uncommitted changes. Propose a logical grouping of commits ordered by dependency layers (Documentation → Database → Core → Service → Presentation → Tests) to facilitate code review. Consider staging specific hunks/lines when a file contains changes for multiple logical commits.
</start-instruction>

For maximum efficiency, whenever you need to perform multiple independent operations, invoke all relevant tools simultaneously rather than sequentially.
