---
name: claude-md-author
description: >
  Create, edit, and improve CLAUDE.md files for Claude Code projects. Use this skill when asked to
  write, audit, refactor, or optimize a CLAUDE.md (or CLAUDE.local.md) file. Produces files that
  make Claude Code sessions dramatically more effective by encoding institutional knowledge,
  project conventions, and critical context directly into the project's memory layer.
---

# CLAUDE.md Authoring Skill

This skill guides the creation and refinement of CLAUDE.md files — the persistent memory layer that transforms Claude Code from a generic coding assistant into a project-aware expert. A great CLAUDE.md is **dense with signal, stripped of noise, and structured for instant utility**.

The user provides some combination of: a request to create a CLAUDE.md from scratch, an existing CLAUDE.md to improve, a codebase to analyze, or a description of their project.

---

## What CLAUDE.md Files Are (And Why They Matter)

CLAUDE.md files are Markdown files that Claude Code reads **automatically at the start of every session**. They inject project-specific knowledge into Claude's context window before a single message is exchanged. This means Claude arrives already knowing the architecture, conventions, commands, and current state of the project — not because of training, but because the CLAUDE.md told it so.

The magic is simple: **every token in a CLAUDE.md is worth orders of magnitude more than the same token in a chat message**, because it applies to the entire session and shapes every action Claude takes.

A project *without* a CLAUDE.md forces Claude to rediscover the same things over and over. A project *with* a great CLAUDE.md lets Claude hit the ground running, make fewer mistakes, and operate as if it's been on the team for months.

---

## The CLAUDE.md Ecosystem

Understanding the file hierarchy is essential for proper placement and scoping:

**Discovery order** (all are loaded and concatenated):
1. `~/.claude/CLAUDE.md` — User-level, applies to every project on the machine. Ideal for personal conventions, preferred tools, global aliases, API keys patterns, etc.
2. `./CLAUDE.md` (project root) — Project-wide context. The primary file.
3. `./CLAUDE.local.md` (project root) — Local overrides, gitignored by convention. For machine-specific paths, personal API keys, local dev notes that shouldn't be committed.
4. `{subdirectory}/CLAUDE.md` — Subsystem-specific guidance. Loaded when Claude is working in or near that directory. Use for monorepos, distinct modules, or specialized subsystems.

**Import syntax** — Any CLAUDE.md can inline another file:
```
@path/to/file.md
@docs/architecture.md
@.env.example
```
This is powerful: pull in living documentation, actual config files, or schema definitions rather than duplicating them. The referenced file's content appears inline at that position.

**Size guidance**: Aim for the root CLAUDE.md to be under 200 lines. Every line competes for context window space. Ruthless editing is a feature, not a failure.

---

## Anatomy of a Great CLAUDE.md

The best CLAUDE.md files share a consistent structure. Not all sections are required for every project — include what's genuinely useful, omit what isn't.

### 1. Project Identity (2–5 lines)
One tight paragraph: what is this project, what does it do, who uses it. This orients Claude immediately and shapes how it interprets everything that follows.

```markdown
# ProjectName

A TypeScript monorepo powering the customer-facing API and internal admin tooling for Acme Corp's
B2B SaaS platform. Serves ~500 enterprise clients via REST API with ~2M req/day. Built by a team
of 8, deployed to AWS via CDK.
```

**Anti-patterns**: Do not write a marketing pitch. Do not describe business history. One sentence about the domain, one about the technical reality.

### 2. Tech Stack
Explicit enumeration of languages, frameworks, key libraries, and infrastructure. Be specific about versions only when version-specific behavior matters.

```markdown
## Stack
- **Language**: TypeScript 5.3, Node 20 (ESM throughout — no CommonJS)
- **Framework**: Fastify (not Express — different plugin model)
- **ORM**: Drizzle (not Prisma — raw SQL when Drizzle won't do)
- **Testing**: Vitest + @testing-library/react for components
- **DB**: PostgreSQL 15 via RDS; Redis for cache/queues
- **Infra**: AWS CDK (TypeScript), deployed via GitHub Actions
```

Note how each entry calls out what was *chosen over* the obvious alternative. This prevents Claude from recommending Prisma in a Drizzle project.

### 3. Essential Commands
Every command Claude might need to run. Exact, copy-pasteable. No paraphrasing.

```markdown
## Commands

```bash
pnpm install          # install deps (pnpm only — npm/yarn break lockfile)
pnpm dev              # start all services with hot reload (uses Turborepo)
pnpm build            # production build
pnpm test             # run all tests
pnpm test:watch       # watch mode
pnpm lint             # ESLint + Prettier check
pnpm lint:fix         # auto-fix lint errors
pnpm db:migrate       # run pending migrations
pnpm db:generate      # generate Drizzle schema from DB
pnpm typecheck        # tsc --noEmit (no build output)
```
```

**Critical**: Include the "don't do this" commands too:
```markdown
> ⚠️ Never run `pnpm db:reset` in production. Never commit `.env.local`.
```

### 4. Architecture
How the pieces fit together. This section has the highest ROI of anything in the file — it's the map Claude uses to navigate every task.

```markdown
## Architecture

### Monorepo Structure
```
apps/
  api/          # Fastify REST API — the core backend
  admin/        # Internal React app (Vite)
  worker/       # BullMQ job processors
packages/
  db/           # Drizzle schema + migrations (source of truth for all DB types)
  shared/       # Types/utils shared across apps (no runtime deps allowed here)
  ui/           # Shared React component library
```

### Request Lifecycle
Client → ALB → api/src/routes → service layer (src/services) → db package → PostgreSQL.
Auth happens in a Fastify hook before routes execute. Never do auth logic inside route handlers.

### Key Patterns
- Services are plain async functions, not classes.
- Route handlers are thin — they validate input (Zod), call a service, return the result.
- All DB access goes through `packages/db`. No raw `pg` calls in app code.
- Errors: throw typed errors from services; the global error handler in `api/src/app.ts` formats them.
```

### 5. Code Conventions
The house style. What's enforced, what's preferred, what's forbidden. This is where you encode the decisions that never appear in config files.

```markdown
## Conventions

### TypeScript
- `strict: true` — no `any`, no `// @ts-ignore` without a comment explaining why
- Prefer `type` over `interface` except for public API contracts
- Zod schemas live next to the route that uses them (not in a central schema file)

### Naming
- Files: `kebab-case.ts`
- React components: `PascalCase.tsx`
- DB tables: `snake_case` (Drizzle convention)
- Env vars: `SCREAMING_SNAKE_CASE`, always validated at startup via `src/config.ts`

### Testing
- Unit tests: `*.test.ts` alongside the source file
- Integration tests: `tests/integration/` — require a running DB
- Tests use real DB with per-test transactions that roll back. No mocking the DB layer.
- Mock external HTTP calls using `msw`, not Jest mocks

### Git
- Commits: Conventional Commits (`feat:`, `fix:`, `chore:`, etc.)
- PRs require passing CI + 1 review
- Never force-push to `main` or `develop`
```

### 6. Critical Context (The Secret Weapon)
This is the most valuable section and the most neglected. It captures **non-obvious knowledge** — the things that only a senior team member would know, the gotchas, the things that look wrong but are intentional, the dragons.

```markdown
## Critical Context

### Known Gotchas
- **ESM + Drizzle**: Drizzle's `migrate()` must be called with `import.meta.url` not `__dirname`.
  Already handled in `packages/db/src/migrate.ts` — don't change this pattern.
- **Worker service account**: The `worker` app runs as a different IAM role than `api`.
  It has S3 write access but NOT SQS receive (uses BullMQ/Redis instead).
- **Timestamps**: All timestamps stored as UTC in PostgreSQL. The admin app displays in user's
  timezone using `date-fns-tz`. Never use `new Date()` for insertion — use `sql\`NOW()\``.
- **CORS**: The `allowedOrigins` config in `api/src/app.ts` reads from env. In dev, `*` is fine.
  In prod, it must exactly match the CloudFront distribution URL (no trailing slash).

### Currently In Progress
- The `packages/ui` component library is being extracted from `apps/admin`. Some components
  still live in admin — check both places before creating new ones.
- Auth is mid-migration from JWT-in-cookie to Clerk. Both systems are live simultaneously.
  New features should use Clerk (`src/auth/clerk.ts`). Don't touch `src/auth/legacy-jwt.ts`.

### Known Broken / Deferred
- `pnpm test` in `apps/worker` fails on CI due to a Docker networking issue. Tests pass locally.
  Skip with `pnpm test --ignore-fail` in CI (tracked in GH issue #482).
```

### 7. External Services & Integrations
What APIs and services this project talks to, where credentials live, what the dev/prod split looks like.

```markdown
## External Services

| Service | Purpose | Dev credential source | Prod credential |
|---------|---------|----------------------|-----------------|
| Stripe | Billing | `.env.local` (test mode key) | AWS Secrets Manager |
| SendGrid | Email | Mocked via `msw` in tests | AWS Secrets Manager |
| Datadog | APM/Logs | Not configured locally | Set via CDK |
| OpenAI | Embeddings | `.env.local` | AWS Secrets Manager |

All secrets accessed via `src/config.ts`. Never access `process.env` directly outside that file.
```

---

## Authoring Principles

### Signal-to-Noise Ratio Is Everything
Every line must earn its place. Ask: "Would Claude do the wrong thing without this?" If no, delete it.

**High signal** (include): Non-obvious choices, things that contradict common defaults, current state of in-progress work, commands that aren't in package.json scripts, things that broke before and could break again.

**Low signal** (omit): Things Claude already knows (React is a UI library, git is version control), things obvious from the directory structure, things that live in config files Claude can read, marketing/mission statements.

### Write for a Smart New Hire, Not a Robot
Imagine explaining the project to a senior engineer on their first day. They're smart — skip the basics. They're new — don't skip the tribal knowledge. Write the things you'd say at the whiteboard, not the things in the README.

### Use Opinions, Not Just Facts
"We use Fastify" is fine. "We use Fastify (not Express) — the plugin system is fundamentally different; don't suggest Express patterns" is better. CLAUDE.md should encode *why* decisions were made, not just *what* they are.

### Keep It Fresh
A stale CLAUDE.md is worse than none — it actively misleads. Establish a team norm: when you change a major convention, you update CLAUDE.md in the same PR. When you complete a migration, remove the in-progress warning.

### Structure for Skimming
Claude processes the file linearly but needs key facts to be findable quickly within a session. Use consistent H2/H3 headers. Lead sections with the most important information. Put warnings and gotchas in blockquotes or bold text so they read as emphatic.

---

## CLAUDE.local.md — The Personal Layer

This file is for machine-specific and developer-specific context. Gitignore it by default:

```gitignore
CLAUDE.local.md
```

Appropriate content:
```markdown
# Local Overrides

## My Environment
- DB runs on port 5433 (not 5432 — conflicting local Postgres instance)
- I use `mise` for runtime management, not `nvm`
- Local dev uses the staging Stripe key (not test mode) — careful with actual charges

## Personal Workflow Notes
- I keep a running `scratch.ts` in the root for experiments — never commit this
- Turborepo remote cache is disabled on this machine (run `pnpm build --no-cache`)
```

---

## Monorepo Subdirectory CLAUDE.md Pattern

For large monorepos, put subsystem-specific files in subdirectories:

```
packages/db/CLAUDE.md        # Drizzle schema conventions, migration workflow
apps/admin/CLAUDE.md         # React-specific patterns, component library usage
apps/api/CLAUDE.md           # Route structure, middleware order, error handling
```

These are loaded *in addition to* the root CLAUDE.md when Claude is working in that subtree. They should assume the root file has been read — don't repeat project-wide context.

---

## Audit Checklist (for Improving Existing CLAUDE.md Files)

When given a CLAUDE.md to improve, evaluate each section:

**Completeness**
- [ ] Can a new dev run the project with only this file? (Commands section)
- [ ] Does it explain the architecture in enough detail to navigate the codebase?
- [ ] Does it cover the top 5 most common mistakes someone would make?
- [ ] Does it reflect the *current* state of the project, not a past state?

**Concision**
- [ ] Is every line adding information Claude wouldn't have otherwise?
- [ ] Are there paragraphs that could be cut without losing meaning?
- [ ] Are config-file-readable facts being re-stated here?

**Structure**
- [ ] Are headers consistent and descriptive?
- [ ] Are warnings visually distinct?
- [ ] Is the most important information near the top?

**Currency**
- [ ] Is the "in progress" work actually in progress?
- [ ] Are there references to old libraries, old patterns, or old team members?
- [ ] Do the commands actually work?

---

## Anti-Patterns to Actively Avoid

**The README Clone**: Don't copy-paste the README into CLAUDE.md. README is for humans on GitHub. CLAUDE.md is for Claude in a coding session. Different audiences, different content.

**The Philosophy Essay**: "We believe in clean code and separation of concerns" — useless. Tell Claude what *specifically* that means in this codebase.

**The Comprehensive Encyclopedia**: A 1,000-line CLAUDE.md that documents every possible edge case is self-defeating. Claude has a context window. Put the rare stuff in linked files using `@imports`.

**The Aspirational CLAUDE.md**: Documents how the codebase *should* work, not how it *does* work. "We follow clean architecture" — but the actual code doesn't. This causes Claude to write code that doesn't fit the existing patterns.

**Missing the Gotchas**: Most CLAUDE.md files document the happy path. The real value is in the edges. What fails silently? What seems like a bug but is intentional? What will waste two hours to debug?

**Stale In-Progress Sections**: The "currently migrating from X to Y" note that's been there for 18 months because the migration was abandoned. This is actively harmful.

---

## Output Format

When creating a CLAUDE.md, produce a complete, ready-to-commit file in Markdown. Structure it with H2 section headers. Use code blocks for all commands and code snippets. Use blockquotes (`>`) or bold for warnings and critical notes.

When auditing an existing CLAUDE.md, provide:
1. A diagnosis of what's missing, stale, or noisy
2. The revised file in full
3. A brief changelog of what changed and why

Always optimize for the engineer who reads it six months from now knowing nothing about today's context.