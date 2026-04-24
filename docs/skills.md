# Skills Guide

Skills are folders that teach an agent how to perform a specific workflow. They
are most useful when a task has repeatable domain knowledge, sequencing,
validation, or tool-use patterns that should not be re-explained every time.

## Structure

```text
skill-name/
├── SKILL.md
├── scripts/
├── references/
└── assets/
```

Only `SKILL.md` is required. Use `scripts/` for deterministic helper code,
`references/` for detailed docs that should load only when needed, and `assets/`
for templates, fonts, icons, or other output materials.

## Required Frontmatter

`SKILL.md` must begin with YAML frontmatter:

```yaml
---
name: your-skill-name
description: What the skill does. Use when the user asks to do specific tasks,
  mentions concrete trigger phrases, or provides relevant file types.
---
```

Rules:

- The file must be named exactly `SKILL.md`.
- The folder and `name` must use lowercase kebab-case.
- Do not use spaces, underscores, capitals, or XML angle brackets in
  frontmatter.
- The description must explain both what the skill does and when to use it.
- Keep the description under 1024 characters.
- Optional fields include `license`, `compatibility`, and custom `metadata`.

## Progressive Disclosure

Design skills in three layers:

| Layer | Loaded When | Content |
|---|---|---|
| YAML frontmatter | Always | Minimal trigger information |
| `SKILL.md` body | Skill is selected | Core workflow instructions |
| Linked files | Explicitly opened | Detailed references, examples, assets |

Keep `SKILL.md` concise. Move API details, style guides, long examples, and
edge-case matrices into `references/`.

## Planning A Skill

Start with two or three concrete use cases:

```text
Use case: Project sprint planning
Trigger: "help me plan this sprint" or "create sprint tasks"
Steps:
1. Fetch current project status.
2. Analyze velocity and capacity.
3. Prioritize tasks.
4. Create or update tasks with labels and estimates.
Result: A planned sprint with traceable task updates.
```

Define success before writing:

- The skill triggers on obvious and paraphrased requests.
- It does not trigger on unrelated work.
- It completes the workflow with fewer clarifying turns.
- Tool calls succeed or fail with clear recovery steps.
- Outputs are consistent across sessions.

## Writing Good Instructions

Prefer concrete actions over vague advice:

```markdown
Run `python scripts/validate.py --input {filename}` before creating records.
If validation fails, fix missing required fields and invalid dates first.
```

Avoid instructions like "validate the data before proceeding" unless the exact
validation behavior is defined elsewhere.

A useful `SKILL.md` body usually contains:

- A short purpose statement.
- Required preflight checks.
- Numbered workflow steps.
- Tool or script commands with expected outputs.
- Error handling for common failures.
- Examples for the main user scenarios.
- Quality gates or final verification steps.

For critical behavior, prefer scripts over prose. A validation script is more
reliable than a paragraph asking the agent to remember every condition.

## Trigger Quality

Good descriptions combine scope and user language:

```yaml
description: Manages sprint planning workflows including backlog review,
  task creation, status updates, and retrospective prep. Use when the user
  says "plan this sprint", "refine backlog", "create sprint tasks", or asks
  for scrum planning help.
```

Avoid descriptions that are too broad, purely technical, or missing trigger
phrases:

```yaml
description: Helps with projects.
description: Implements Project entity relationships.
```

If a skill under-triggers, add domain terms, file types, and phrases users
actually say. If it over-triggers, narrow the scope and add negative triggers.

## Testing

Test three areas:

| Area | What To Check |
|---|---|
| Triggering | Loads for obvious and paraphrased requests; stays quiet otherwise |
| Functionality | Produces valid outputs, handles edge cases, and recovers from errors |
| Performance | Reduces turns, failed calls, and repeated user instructions |

Example trigger tests:

```text
Should trigger:
- "Help me plan this sprint."
- "Create sprint tasks for the Q4 backlog."
- "Prepare a retrospective."

Should not trigger:
- "Create a spreadsheet."
- "Summarize this unrelated article."
```

Iterate on one challenging workflow until it succeeds reliably, then generalize.

## Distribution

For local use, place the skill folder in the environment's skills directory. For
sharing, host the folder in a repository with a top-level README outside the
skill folder. Organization-wide deployment should include versioning,
installation instructions, and a short test prompt.

For API-backed systems, manage skills through the skills API and attach them to
requests via the container skills configuration. API use requires the secure code
execution environment that skills depend on.

## Troubleshooting

Skill will not upload:

- Confirm the file is named `SKILL.md`.
- Confirm frontmatter starts and ends with `---`.
- Confirm the `name` is lowercase kebab-case.
- Remove forbidden XML angle brackets from frontmatter.

Skill does not trigger:

- Make the description less generic.
- Add common user phrases.
- Mention relevant file types or systems.

Skill triggers too often:

- Narrow the domain.
- Add "Do not use..." scope language.
- Split broad skills into smaller skills.

Instructions are not followed:

- Put critical steps near the top.
- Replace ambiguous language with explicit checks.
- Move long detail into references.
- Add scripts for deterministic checks.

Large context or slow responses:

- Keep `SKILL.md` short.
- Move examples and specs into `references/`.
- Reduce the number of enabled skills when possible.

## This Repo's Skills

Current local skills:

| Skill | Status | Purpose |
|---|---|---|
| `claude-md-author` | Active | Author and improve `CLAUDE.md` / `AGENTS.md` project instruction files. |
| `github-issues` | Active | Manage the GitHub Issues lifecycle with `gh`. |
| `scrum-master` | Active | Support sprint planning, backlog refinement, reviews, retrospectives, and scrum decisions. |
| `skill-creator` | Active | Create, package, validate, and improve skills. |

Planned or previously proposed skills:

| Skill | Status | Notes |
|---|---|---|
| `skill-author` | Superseded | Covered by `skill-creator`. |
| `github-wiki` | Planned | Would manage GitHub wiki pages. |
| `github-pull-requests` | Planned | Would manage PR creation, updates, reviews, and merge prep. |

Reference: the Anthropic skills repository contains third-party examples that can
be adapted for local workflows.
