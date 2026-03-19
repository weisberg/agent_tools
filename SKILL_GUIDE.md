# Skill Guide for Claude

A complete reference for building, testing, and distributing Claude skills. Extracted from Anthropic's official guide.

---

## Table of Contents

1. [What is a Skill?](#what-is-a-skill)
2. [Core Design Principles](#core-design-principles)
3. [Planning and Design](#planning-and-design)
4. [Technical Requirements](#technical-requirements)
5. [Writing Effective Skills](#writing-effective-skills)
6. [Testing and Iteration](#testing-and-iteration)
7. [Distribution and Sharing](#distribution-and-sharing)
8. [Patterns](#patterns)
9. [Troubleshooting](#troubleshooting)
10. [Quick Checklist](#quick-checklist)
11. [Resources](#resources)

---

## What is a Skill?

A skill is a **folder** containing:

| File/Dir | Required | Purpose |
|---|---|---|
| `SKILL.md` | Yes | Instructions in Markdown with YAML frontmatter |
| `scripts/` | No | Executable code (Python, Bash, etc.) |
| `references/` | No | Documentation loaded as needed |
| `assets/` | No | Templates, fonts, icons used in output |

Skills teach Claude how to handle specific tasks or workflows **once**, so you benefit every time — no re-explaining needed. They work with Claude's built-in capabilities (code execution, document creation) and complement MCP integrations.

**Skills are portable:** they work identically across Claude.ai, Claude Code, and the API.

---

## Core Design Principles

### Progressive Disclosure (Three-Level System)

- **Level 1 — YAML frontmatter:** Always loaded in Claude's system prompt. Just enough info to know when to use the skill.
- **Level 2 — SKILL.md body:** Loaded when Claude thinks the skill is relevant. Full instructions and guidance.
- **Level 3 — Linked files:** Files in `references/` and `assets/` that Claude discovers only as needed.

This minimizes token usage while maintaining specialized expertise.

### Composability

Claude can load multiple skills simultaneously. Design your skill to work well alongside others — don't assume it's the only capability available.

### Skills + MCP: The Kitchen Analogy

| MCP (Connectivity) | Skills (Knowledge) |
|---|---|
| Connects Claude to your service | Teaches Claude how to use your service effectively |
| Provides real-time data access and tool invocation | Captures workflows and best practices |
| **What** Claude can do | **How** Claude should do it |

**Without skills:** Users connect your MCP but don't know what to do next. Inconsistent results. Each conversation starts from scratch.

**With skills:** Pre-built workflows activate automatically. Consistent, reliable tool usage. Best practices embedded in every interaction.

---

## Planning and Design

### Start with Use Cases

Before writing any code, identify 2-3 concrete use cases your skill should enable.

**Good use case definition:**
```
Use Case: Project Sprint Planning
Trigger: User says "help me plan this sprint" or "create sprint tasks"
Steps:
1. Fetch current project status from Linear (via MCP)
2. Analyze team velocity and capacity
3. Suggest task prioritization
4. Create tasks in Linear with proper labels and estimates
Result: Fully planned sprint with tasks created
```

**Ask yourself:**
- What does a user want to accomplish?
- What multi-step workflows does this require?
- Which tools are needed (built-in or MCP)?
- What domain knowledge or best practices should be embedded?

### Common Use Case Categories

**Category 1: Document & Asset Creation**
- Creating consistent, high-quality output: documents, presentations, apps, designs, code
- Key techniques: embedded style guides, template structures, quality checklists, no external tools needed

**Category 2: Workflow Automation**
- Multi-step processes with consistent methodology, including coordination across multiple MCP servers
- Key techniques: step-by-step workflows with validation gates, templates for common structures, iterative refinement loops

**Category 3: MCP Enhancement**
- Workflow guidance to enhance the tool access an MCP server provides
- Key techniques: coordinates multiple MCP calls in sequence, embeds domain expertise, provides context users would otherwise need to specify, error handling for common MCP issues

### Define Success Criteria

**Quantitative metrics (aspirational targets):**
- Skill triggers on 90% of relevant queries — test with 10-20 queries that should trigger it
- Completes workflow in X tool calls — compare with and without skill enabled
- 0 failed API calls per workflow — monitor MCP server logs during test runs

**Qualitative metrics:**
- Users don't need to prompt Claude about next steps
- Workflows complete without user correction — run the same request 3-5 times, compare outputs
- Consistent results across sessions — can a new user accomplish the task on first try?

---

## Technical Requirements

### File Structure

```
your-skill-name/
├── SKILL.md                  # Required - main skill file
├── scripts/                  # Optional - executable code
│   ├── process_data.py
│   └── validate.sh
├── references/               # Optional - documentation
│   ├── api-guide.md
│   └── examples/
└── assets/                   # Optional - templates, etc.
    └── report-template.md
```

### Critical Rules

**SKILL.md naming:**
- Must be exactly `SKILL.md` (case-sensitive)
- No variations accepted (`SKILL.MD`, `skill.md`, etc.)

**Skill folder naming:**
- Use kebab-case: `notion-project-setup` ✓
- No spaces: `Notion Project Setup` ✗
- No underscores: `notion_project_setup` ✗
- No capitals: `NotionProjectSetup` ✗

**No README.md inside skill folder:**
- All documentation goes in `SKILL.md` or `references/`
- When distributing via GitHub, put the README at the repo level (outside skill folder)

**Security restrictions (forbidden in frontmatter):**
- XML angle brackets (`<` `>`)
- Skills with "claude" or "anthropic" in the name (reserved)
- Reason: frontmatter appears in Claude's system prompt; malicious content could inject instructions

### YAML Frontmatter

**Minimal required format:**
```yaml
---
name: your-skill-name
description: What it does. Use when user asks to [specific phrases].
---
```

**Field requirements:**

| Field | Required | Rules |
|---|---|---|
| `name` | Yes | kebab-case only, no spaces or capitals, should match folder name |
| `description` | Yes | MUST include WHAT it does + WHEN to use it; under 1024 chars; no XML tags; include specific tasks users might say |
| `license` | No | Use if open source; common: MIT, Apache-2.0 |
| `compatibility` | No | 1-500 chars; indicates environment requirements |
| `metadata` | No | Any custom key-value pairs; suggested: author, version, mcp-server |

**Metadata example:**
```yaml
metadata:
    author: ProjectHub
    version: 1.0.0
    mcp-server: projecthub
```

---

## Writing Effective Skills

### The Description Field

This is how Claude decides whether to load your skill — get it right. Structure:

```
[What it does] + [When to use it] + [Key capabilities]
```

**Good descriptions:**
```yaml
# Specific and actionable
description: Analyzes Figma design files and generates developer handoff
  documentation. Use when user uploads .fig files, asks for "design specs",
  "component documentation", or "design-to-code handoff".

# Includes trigger phrases
description: Manages Linear project workflows including sprint planning, task
  creation, and status tracking. Use when user mentions "sprint", "Linear
  tasks", "project planning", or asks to "create tickets".

# Clear value proposition
description: End-to-end customer onboarding workflow for PayFlow. Handles
  account creation, payment setup, and subscription management. Use when user
  says "onboard new customer", "set up subscription", or "create PayFlow account".
```

**Bad descriptions:**
```yaml
# Too vague
description: Helps with projects.

# Missing triggers
description: Creates sophisticated multi-page documentation systems.

# Too technical, no user triggers
description: Implements the Project entity model with hierarchical relationships.
```

### Recommended SKILL.md Structure

```markdown
---
name: your-skill
description: [...]
---

# Your Skill Name

## Instructions

### Step 1: [First Major Step]
Clear explanation of what happens.

```bash
python scripts/fetch_data.py --project-id PROJECT_ID
Expected output: [describe what success looks like]
```

(Add more steps as needed)

## Examples

### Example 1: [common scenario]
User says: "Set up a new marketing campaign"
Actions:
1. Fetch existing campaigns via MCP
2. Create new campaign with provided parameters
Result: Campaign created with confirmation link

## Troubleshooting

### Error: [Common error message]
**Cause:** [Why it happens]
**Solution:** [How to fix]
```

### Best Practices for Instructions

**Be specific and actionable:**
```markdown
# Good
Run `python scripts/validate.py --input {filename}` to check data format.
If validation fails, common issues include:
- Missing required fields (add them to the CSV)
- Invalid date formats (use YYYY-MM-DD)

# Bad
Validate the data before proceeding.
```

**Include error handling:**
```markdown
## Common Issues

### MCP Connection Failed
If you see "Connection refused":
1. Verify MCP server is running: Check Settings > Extensions
2. Confirm API key is valid
3. Try reconnecting: Settings > Extensions > [Your Service] > Reconnect
```

**Reference bundled resources clearly:**
```markdown
Before writing queries, consult `references/api-patterns.md` for:
- Rate limiting guidance
- Pagination patterns
- Error codes and handling
```

**Use progressive disclosure:** Keep `SKILL.md` focused on core instructions. Move detailed documentation to `references/` and link to it.

**Avoid model "laziness"** by adding explicit encouragement in instructions:
```markdown
## Performance Notes
- Take your time to do this thoroughly
- Quality is more important than speed
- Do not skip validation steps
```
*(Note: Adding this to user prompts is more effective than in SKILL.md)*

---

## Testing and Iteration

### Testing Approaches

| Approach | Best for |
|---|---|
| **Manual testing in Claude.ai** | Fast iteration, no setup required |
| **Scripted testing in Claude Code** | Repeatable validation across changes |
| **Programmatic testing via skills API** | Systematic evaluation suites |

**Pro tip:** Iterate on a single challenging task until Claude succeeds, then extract the winning approach into a skill. Provides faster signal than broad testing.

### Three Testing Areas

**1. Triggering Tests** — Ensure your skill loads at the right times

Test cases:
- Triggers on obvious tasks
- Triggers on paraphrased requests
- Does NOT trigger on unrelated topics

```
Should trigger:
- "Help me set up a new ProjectHub workspace"
- "I need to create a project in ProjectHub"
- "Initialize a ProjectHub project for Q4 planning"

Should NOT trigger:
- "What's the weather in San Francisco?"
- "Help me write Python code"
- "Create a spreadsheet" (unless skill handles sheets)
```

**2. Functional Tests** — Verify the skill produces correct outputs

Test cases:
- Valid outputs generated
- API calls succeed
- Error handling works
- Edge cases covered

```
Test: Create project with 5 tasks
Given: Project name "Q4 Planning", 5 task descriptions
When: Skill executes workflow
Then:
  - Project created in ProjectHub
  - 5 tasks created with correct properties
  - All tasks linked to project
  - No API errors
```

**3. Performance Comparison** — Prove the skill improves results vs. baseline

```
Without skill:
- User provides instructions each time
- 15 back-and-forth messages
- 3 failed API calls requiring retry
- 12,000 tokens consumed

With skill:
- Automatic workflow execution
- 2 clarifying questions only
- 0 failed API calls
- 6,000 tokens consumed
```

### Using the `skill-creator` Skill

Available in Claude.ai (plugin directory) and Claude Code. Can build and test a functional skill in 15-30 minutes.

**Creating:** Generate skills from natural language descriptions. Produces properly formatted SKILL.md with frontmatter. Suggests trigger phrases and structure.

**Reviewing:** Flags common issues (vague descriptions, missing triggers, structural problems). Identifies potential over/under-triggering risks.

**Iterative improvement:** Bring edge cases or failures back to skill-creator: "Use the issues & solution identified in this chat to improve how the skill handles [specific edge case]"

To use: `"Use the skill-creator skill to help me build a skill for [your use case]"`

### Iteration Signals

**Undertriggering** (skill doesn't load when it should):
- Solution: Add more detail and keywords to description, particularly for technical terms

**Overtriggering** (skill loads for irrelevant queries):
- Solution: Add negative triggers, be more specific in description

**Execution issues** (inconsistent results, API failures, user corrections needed):
- Solution: Improve instructions, add error handling

---

## Distribution and Sharing

### How Users Get Skills (Current Model, January 2026)

**Individual users:**
1. Download the skill folder
2. Zip the folder (if needed)
3. Upload to Claude.ai via Settings > Capabilities > Skills
4. Or place in Claude Code skills directory

**Organization-level (shipped December 18, 2025):**
- Admins can deploy skills workspace-wide
- Automatic updates
- Centralized management

### Using Skills via API

For programmatic use cases — applications, agents, automated workflows:

- `/v1/skills` endpoint for listing and managing skills
- Add skills to Messages API requests via the `container.skills` parameter
- Version control and management through the Claude Console
- Works with the Claude Agent SDK for building custom agents

**Note:** Skills in the API require the Code Execution Tool beta for the secure environment skills need to run.

| Use Case | Best Surface |
|---|---|
| End users interacting with skills directly | Claude.ai / Claude Code |
| Manual testing and iteration during development | Claude.ai / Claude Code |
| Individual, ad-hoc workflows | Claude.ai / Claude Code |
| Applications using skills programmatically | API |
| Production deployments at scale | API |
| Automated pipelines and agent systems | API |

### Recommended Distribution Approach

**1. Host on GitHub**
- Public repo for open-source skills
- Clear README with installation instructions (at repo level, NOT inside skill folder)
- Example usage and screenshots

**2. Document in Your MCP Repo**
- Link to skills from MCP documentation
- Explain the value of using both together
- Provide quick-start guide

**3. Create an Installation Guide**
```markdown
## Installing the [Your Service] skill

1. Download the skill:
   - Clone repo: `git clone https://github.com/yourcompany/skills`
   - Or download ZIP from Releases

2. Install in Claude:
   - Open Claude.ai > Settings > Skills
   - Click "Upload skill"
   - Select the skill folder (zipped)

3. Enable the skill:
   - Toggle on the [Your Service] skill
   - Ensure your MCP server is connected

4. Test:
   - Ask Claude: "Set up a new project in [Your Service]"
```

### Positioning Your Skill

**Focus on outcomes, not features:**
```
# Good
"The ProjectHub skill enables teams to set up complete project workspaces
in seconds — including pages, databases, and templates — instead of
spending 30 minutes on manual setup."

# Bad
"The ProjectHub skill is a folder containing YAML frontmatter and
Markdown instructions that calls our MCP server tools."
```

**Highlight the MCP + Skills story:**
```
"Our MCP server gives Claude access to your Linear projects.
Our skills teach Claude your team's sprint planning workflow.
Together, they enable AI-powered project management."
```

---

## Patterns

These patterns emerged from early adopters and internal teams.

### Choosing Your Approach

- **Problem-first:** "I need to set up a project workspace" → skill orchestrates the right MCP calls in the right sequence
- **Tool-first:** "I have Notion MCP connected" → skill teaches Claude optimal workflows and best practices

### Pattern 1: Sequential Workflow Orchestration

**Use when:** Your users need multi-step processes in a specific order.

```markdown
## Workflow: Onboard New Customer

### Step 1: Create Account
Call MCP tool: `create_customer`
Parameters: name, email, company

### Step 2: Setup Payment
Call MCP tool: `setup_payment_method`
Wait for: payment method verification

### Step 3: Create Subscription
Call MCP tool: `create_subscription`
plan_id, customer_id (from Step 1)

### Step 4: Send Welcome Email
Call MCP tool: `send_email`
Template: welcome_email_template
```

Key techniques: explicit step ordering, dependencies between steps, validation at each stage, rollback instructions for failures.

### Pattern 2: Multi-MCP Coordination

**Use when:** Workflows span multiple services.

```markdown
### Phase 1: Design Export (Figma MCP)
1. Export design assets from Figma
2. Generate design specifications
3. Create asset manifest

### Phase 2: Asset Storage (Drive MCP)
1. Create project folder in Drive
2. Upload all assets
3. Generate shareable links

### Phase 3: Task Creation (Linear MCP)
1. Create development tasks
2. Attach asset links to tasks
3. Assign to engineering team

### Phase 4: Notification (Slack MCP)
1. Post handoff summary to #engineering
2. Include asset links and task references
```

Key techniques: clear phase separation, data passing between MCPs, validation before moving to next phase, centralized error handling.

### Pattern 3: Iterative Refinement

**Use when:** Output quality improves with iteration.

```markdown
## Iterative Report Creation

### Initial Draft
1. Fetch data via MCP
2. Generate first draft report
3. Save to temporary file

### Quality Check
1. Run validation script: `scripts/check_report.py`
2. Identify issues:
   - Missing sections
   - Inconsistent formatting
   - Data validation errors

### Refinement Loop
1. Address each identified issue
2. Regenerate affected sections
3. Re-validate
4. Repeat until quality threshold met

### Finalization
1. Apply final formatting
2. Generate summary
3. Save final version
```

Key techniques: explicit quality criteria, iterative improvement, validation scripts, know when to stop iterating.

### Pattern 4: Context-Aware Tool Selection

**Use when:** Same outcome, different tools depending on context.

```markdown
## Smart File Storage

### Decision Tree
1. Check file type and size
2. Determine best storage location:
   - Large files (>10MB): Use cloud storage MCP
   - Collaborative docs: Use Notion/Docs MCP
   - Code files: Use GitHub MCP
   - Temporary files: Use local storage

### Execute Storage
Based on decision:
- Call appropriate MCP tool
- Apply service-specific metadata
- Generate access link

### Provide Context to User
Explain why that storage was chosen
```

Key techniques: clear decision criteria, fallback options, transparency about choices.

### Pattern 5: Domain-Specific Intelligence

**Use when:** Your skill adds specialized knowledge beyond tool access.

```markdown
## Payment Processing with Compliance

### Before Processing (Compliance Check)
1. Fetch transaction details via MCP
2. Apply compliance rules:
   - Check sanctions lists
   - Verify jurisdiction allowances
   - Assess risk level
3. Document compliance decision

### Processing
IF compliance passed:
  - Call payment processing MCP tool
  - Apply appropriate fraud checks
  - Process transaction
ELSE:
  - Flag for review
  - Create compliance case

### Audit Trail
- Log all compliance checks
- Record processing decisions
- Generate audit report
```

Key techniques: domain expertise embedded in logic, compliance before action, comprehensive documentation, clear governance.

---

## Troubleshooting

### Skill Won't Upload

**Error: "Could not find SKILL.md in uploaded folder"**
- Cause: File not named exactly `SKILL.md`
- Solution: Rename to `SKILL.md` (case-sensitive); verify with `ls -la` showing `SKILL.md`

**Error: "Invalid frontmatter"**
- Cause: YAML formatting issue
- Common mistakes:
```yaml
# Wrong - missing --- delimiters
name: my-skill
description: Does things

# Wrong - unclosed quotes
name: my-skill
description: "Does things

# Correct
---
name: my-skill
description: Does things
---
```

**Error: "Invalid skill name"**
- Cause: Name has spaces or capitals
```yaml
# Wrong
name: My Cool Skill

# Correct
name: my-cool-skill
```

### Skill Doesn't Trigger

**Symptom:** Skill never loads automatically

**Fix:** Revise your description field.

Quick checklist:
- Is it too generic? ("Helps with projects" won't work)
- Does it include trigger phrases users would actually say?
- Does it mention relevant file types if applicable?

**Debugging approach:** Ask Claude: "When would you use the [skill name] skill?" Claude will quote the description back. Adjust based on what's missing.

### Skill Triggers Too Often

**Symptom:** Skill loads for unrelated queries

**Solutions:**
1. Add negative triggers:
```yaml
description: Advanced data analysis for CSV files. Use for statistical modeling,
  regression, clustering. Do NOT use for simple data exploration
  (use data-viz skill instead).
```
2. Be more specific (e.g., "Processes PDF legal documents" not "Processes documents")
3. Clarify scope (add "Use specifically for X, not for general Y queries")

### MCP Connection Issues

**Symptom:** Skill loads but MCP calls fail

**Checklist:**
1. Verify MCP server is connected: Claude.ai: Settings > Extensions > [Your Service] — should show "Connected"
2. Check authentication: API keys valid and not expired, proper permissions/scopes granted, OAuth tokens refreshed
3. Test MCP independently: Ask Claude to call MCP directly (without skill) — "Use [Service] MCP to fetch my projects" — if this fails, issue is MCP not skill
4. Verify tool names: Skill references correct MCP tool names (case-sensitive); check MCP server documentation

### Instructions Not Followed

**Symptom:** Skill loads but Claude doesn't follow instructions

**Common causes:**

1. **Instructions too verbose** — Keep concise, use bullet points and numbered lists, move detailed reference to separate files
2. **Instructions buried** — Put critical instructions at top, use `## Important` or `## Critical` headers, repeat key points if needed
3. **Ambiguous language:**
```markdown
# Bad
Make sure to validate things properly

# Good
CRITICAL: Before calling create_project, verify:
- Project name is non-empty
- At least one team member assigned
- Start date is not in the past
```
4. **Model "laziness"** — Add explicit encouragement in performance notes section (more effective in user prompts than SKILL.md)

**Advanced technique:** For critical validations, consider bundling a script that performs checks programmatically rather than relying on language instructions. Code is deterministic; language interpretation isn't.

### Large Context Issues

**Symptom:** Skill seems slow or responses degraded

**Causes:** Skill content too large, too many skills enabled simultaneously, all content loaded instead of progressive disclosure

**Solutions:**
1. Optimize SKILL.md size: move detailed docs to `references/`, link to references instead of inline, keep SKILL.md under 5,000 words
2. Reduce enabled skills: evaluate if you have more than 20-50 skills enabled simultaneously, consider skill "packs" for related capabilities

---

## Quick Checklist

### Before You Start
- [ ] Identified 2-3 concrete use cases
- [ ] Tools identified (built-in or MCP)
- [ ] Reviewed guide and example skills
- [ ] Planned folder structure

### During Development
- [ ] Folder named in kebab-case
- [ ] SKILL.md file exists (exact spelling)
- [ ] YAML frontmatter has `---` delimiters
- [ ] `name` field: kebab-case, no spaces, no capitals
- [ ] `description` includes WHAT and WHEN
- [ ] No XML tags (`< >`) anywhere
- [ ] Instructions are clear and actionable
- [ ] Error handling included
- [ ] Examples provided
- [ ] References clearly linked

### Before Upload
- [ ] Tested triggering on obvious tasks
- [ ] Tested triggering on paraphrased requests
- [ ] Verified doesn't trigger on unrelated topics
- [ ] Functional tests pass
- [ ] Tool integration works (if applicable)
- [ ] Compressed as .zip file

### After Upload
- [ ] Test in real conversations
- [ ] Monitor for under/over-triggering
- [ ] Collect user feedback
- [ ] Iterate on description and instructions
- [ ] Update version in metadata

---

## Resources

### Official Documentation
- Best Practices Guide
- Skills Documentation
- API Reference
- MCP Documentation

### Blog Posts
- Introducing Agent Skills
- Engineering Blog: Equipping Agents for the Real World
- Skills Explained
- How to Create Skills for Claude
- Building Skills for Claude Code
- Improving Frontend Design through Skills

### Example Skills
- Public skills repository: `anthropics/skills` on GitHub — contains Anthropic-created skills you can customize

### Tools and Utilities
- **skill-creator skill:** Built into Claude.ai and available for Claude Code. Generates skills from descriptions, produces properly formatted SKILL.md, reviews and provides recommendations. Use: "Help me build a skill using skill-creator"

### Getting Support
- Technical questions: Claude Developers Discord (community forums)
- Bug reports: `anthropics/skills/issues` on GitHub — include skill name, error message, steps to reproduce
