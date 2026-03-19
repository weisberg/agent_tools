---
name: scrum-master
description: >
  Load this skill when making any scrum or agile judgment call: sprint planning, backlog
  refinement, story sizing, sprint review, retrospectives, blocker triage, definition of
  done assessment, or any situation where the right scrum move is unclear. This is a
  reference skill — no tools, only doctrine and decision frameworks calibrated for an
  agentic context.
---

# Scrum Master Skill

## The Scrum Compact (What Scrum Actually Is)

Scrum is a framework for delivering value in short, inspectable cycles. It has three
pillars — transparency, inspection, adaptation — and five values: commitment, courage,
focus, openness, respect. Every scrum rule exists to serve one of these. When in doubt,
ask which pillar or value a decision serves.

Scrum does **not** prescribe how to do the work. It prescribes when to plan, inspect,
and adapt. Everything else is up to the team.

---

## The Five Events

### 1. Sprint Planning

**Purpose:** Decide what to pull into the sprint and how to approach it.

**Inputs required before planning:**
- Refined backlog with estimated stories at the top
- Team velocity from last 2–3 sprints (or a calibrated estimate for new teams)
- Sprint goal candidate (what outcome, not what tasks)

**Sprint goal first, stories second.** The sprint goal is a single sentence describing
the business outcome the sprint delivers. Stories are selected to serve the goal — not
the other way around. If a story doesn't serve the sprint goal, it doesn't belong in
the sprint even if there's capacity.

**Capacity vs. velocity:**
- Velocity = story points completed in recent sprints (use rolling average of last 3)
- Capacity = available person-hours this sprint (account for holidays, meetings, PTO)
- Pull stories until velocity is met or capacity is reached — whichever comes first
- Never plan to 100% capacity. Leave 10–15% for unplanned work and interruptions

**Commitment:** The team commits to the sprint goal, not to every story. Individual
stories may slip; the goal should not.

**Agent planning checklist:**
- [ ] Sprint goal written and agreed
- [ ] Stories at the top of backlog are refined (acceptance criteria exist, sized)
- [ ] Pulled stories sum to ≤ team velocity
- [ ] Each story has at least one assignee
- [ ] Unrefined stories are not pulled — send back to backlog with a refinement comment

---

### 2. Daily Standup (Daily Scrum)

**Purpose:** Inspect progress toward the sprint goal and adapt the day's plan.

**The three questions (reframed for outcome focus):**
- What did I complete that moves us toward the sprint goal?
- What will I do today toward the sprint goal?
- What is blocking progress?

**What standup is not:** A status report to management. It is the team synchronizing
with itself.

**Blocker rule:** Any blocker raised in standup must be actioned the same day — either
resolved, escalated, or documented as an impediment for the scrum master to remove.
Blockers that survive 24 hours without action become sprint risks.

**Agent standup behavior:**
- Post standup comment on each active Issue at session start
- If blocked on every active Issue, raise an impediment immediately — do not silently wait
- If sprint goal is at risk (>30% of sprint remaining, >50% of stories not started),
  flag it explicitly in the standup comment

---

### 3. Sprint Review

**Purpose:** Inspect the increment and adapt the backlog based on feedback.

**Who attends:** Team + stakeholders. This is a working session, not a demo theater.

**What gets reviewed:**
- Only Done work is demonstrated. Partially complete work is not shown.
- The sprint goal: was it met? Why or why not?
- Backlog implications: what does this sprint's outcome tell us about what to do next?

**Outputs:**
- Updated backlog (reprioritized based on what was learned)
- Revised product roadmap if needed
- A clear answer to: "Did we deliver the sprint goal?"

**Agent review behavior:**
- Collect all Issues closed this sprint with their closing comments
- Flag any Issues closed without verified acceptance criteria — these don't count as Done
- Report sprint goal achievement: Met / Partially Met / Not Met, with reason
- List carried-over Issues and root cause for each

---

### 4. Sprint Retrospective

**Purpose:** Inspect how the team worked and commit to at least one improvement.

**The retrospective is about process, not output.** The review covers what was built.
The retro covers how the team built it.

**Standard format (Start / Stop / Continue):**
- **Start:** What should we begin doing that we aren't?
- **Stop:** What are we doing that isn't helping?
- **Continue:** What's working that we should protect?

**The single most important retro rule:** Every retro must produce at least one
concrete action item with an owner and a due date. A retro with only observations
and no commitments is a waste of time.

**Anti-patterns to call out:**
- Repeating the same retro items sprint after sprint (means action items aren't being
  followed through)
- Blaming individuals rather than examining processes
- Retro items that are vague ("better communication") rather than actionable
  ("add a PR review SLA of 24 hours")

**Agent retro behavior:**
- Post retro summary as a GitHub Discussion or pinned Issue comment
- Carry unresolved action items forward as backlog Issues with label `retro-action`
- At next sprint planning, check status of prior retro action items before planning

---

### 5. Backlog Refinement (Grooming)

**Purpose:** Keep the top of the backlog ready for the next sprint.

**Not a formal scrum event** but essential for planning to work. Target: top 2 sprints'
worth of backlog always refined.

**Definition of "refined":**
- [ ] Story has a clear, testable goal statement
- [ ] Acceptance criteria are written and unambiguous
- [ ] Story is sized (estimated)
- [ ] Dependencies are identified and noted
- [ ] Story is small enough to complete in one sprint (if not, split it)

**Rule:** Unrefined stories are never pulled into a sprint. If a story reaches sprint
planning unrefined, return it to the backlog and select the next refined story.

---

## Story Sizing

### Fibonacci scale
Use: 1, 2, 3, 5, 8, 13. Stop at 13 — anything larger must be split.

| Points | Meaning |
|--------|---------|
| 1 | Trivial. Fully understood, minimal unknowns. Under an hour. |
| 2 | Small. Clear scope, low risk. Half a day. |
| 3 | Medium. Some complexity or unknowns. One day. |
| 5 | Large. Meaningful complexity, some investigation needed. 2–3 days. |
| 8 | Very large. Significant unknowns or cross-cutting concerns. Most of a sprint. |
| 13 | Spike or epic fragment. Must be split before pulling into a sprint. |

### When to split a story

Split when any of the following are true:
- Estimated at 13 points
- Has multiple distinct acceptance criteria that could be delivered independently
- Contains both a "happy path" and significant error handling that could ship separately
- Depends on another story that isn't done yet (split the dependency out)
- One part is well-understood and another part requires investigation — split the spike

**Split patterns:**
- By workflow step (create / read / update / delete as separate stories)
- By happy path vs. edge cases
- By data source (story A handles table X, story B handles table Y)
- Spike + implementation (story A: investigate approach; story B: implement)

### Spikes

A spike is a time-boxed investigation with a fixed output: a decision or a document,
not a feature. Spikes are always 1–3 days maximum. If you can't answer the question
in that time, the question is wrong.

Spike output goes in the Issue as a comment. The Issue closes when the investigation
is complete, not when the feature is built.

---

## Definition of Done

The Definition of Done (DoD) is the shared standard that determines when work is
complete. It applies to every story, every sprint. It is not negotiable per-story.

**Minimum DoD for an analytics agent:**
- [ ] All acceptance criteria checked off in the Issue body
- [ ] Outputs are persisted at the agreed location with the agreed naming convention
- [ ] A closing comment documents: what was delivered, where outputs live, key findings, caveats
- [ ] Any newly discovered Issues are filed and linked
- [ ] The Issue is labeled `done` before closing

**What DoD is not:**
- A checklist that gets rubber-stamped
- Negotiable when the sprint is running out of time
- Different for "small" stories

If a story can't meet DoD, it is not Done. It is carried over, and its points are not
counted toward velocity.

---

## Blocker Triage

Not all blockers are equal. Triage before escalating.

### Blocker severity levels

**P1 — Sprint goal at risk:**
- Story is on the critical path to the sprint goal
- Blocker has no workaround
- Action: Escalate immediately, same day, to whoever can unblock

**P2 — Story blocked, sprint goal not at risk:**
- Story is not on the critical path
- Another story can be worked in parallel
- Action: Document blocker in Issue, move to next story, revisit in 24 hours

**P3 — Soft blocker:**
- Work can proceed partially but not completely
- Action: Note in Issue, proceed with available work, do not escalate yet

### Blocker escalation format

```
**BLOCKED — P[1/2/3]**
- Issue: #NUMBER — [title]
- What is blocked: [specific task or decision]
- Why: [root cause]
- What is needed to unblock: [specific ask]
- Who needs to act: @mention
- Workaround available: Yes / No
- Sprint goal impact: Yes / No
```

### When a blocker kills a sprint item

If a P1 blocker is not resolved within 48 hours:
1. Move the story back to the backlog with the blocker documented
2. Pull the next refined story from the backlog if capacity allows
3. Note the swap in the sprint board with a comment on the original Issue
4. Raise as a retro item

---

## Velocity & Forecasting

**Velocity** = story points completed in a sprint (only fully Done stories count).

**Computing team velocity:**
- Use a rolling average of the last 3 completed sprints
- Exclude outlier sprints (team significantly under/over capacity) with a note
- Never inflate velocity by counting partially done work

**Forecasting:**
- Sprints to complete backlog = total backlog points ÷ average velocity
- This is a forecast, not a commitment — communicate it as a range, not a date
- Reforecast every sprint after review

**Velocity anti-patterns:**
- Counting stories as Done that haven't met DoD ("we're 90% there")
- Splitting stories mid-sprint to inflate point counts
- Carrying velocity targets as performance metrics — velocity is a planning tool, not a KPI

---

## Backlog Prioritization

The backlog is ordered by value, not by urgency, seniority of requester, or recency.

**Prioritization factors (in order):**
1. Business value / impact
2. Risk reduction (dependencies, unknowns that block future work)
3. Effort (lower effort = higher priority when value is equal)
4. Strategic alignment

**WSJF (Weighted Shortest Job First)** — a useful heuristic:
```
Priority = (Business Value + Time Criticality + Risk Reduction) ÷ Job Size
```
Higher score = pull sooner.

**Rules:**
- The product owner (or human directing the agent) sets priority. The agent does not
  reprioritize on its own initiative.
- If priority is unclear, ask — do not infer.
- Technical debt and refactoring belong in the backlog as explicit stories, sized and
  prioritized like any other work. "We'll do it when we have time" means it never happens.

---

## Common Anti-Patterns & How to Handle Them

| Anti-pattern | Symptom | Correct response |
|---|---|---|
| Scope creep | New requirements added to in-flight story | File a new Issue, do not expand the current story |
| Zombie stories | Same Issue carries over 3+ sprints | Split, descope, or explicitly deprioritize with a comment explaining why |
| Ghost points | Closed Issues that didn't meet DoD | Reopen, remove `done` label, return to backlog |
| Heroics | One person finishing everything in the last two days | Flag in retro; redistribute in next sprint planning |
| Vanity velocity | Inflating points by counting partial work | Recount — only fully Done stories contribute |
| Sprint without a goal | Sprint is just a list of tasks | Rewrite as outcome: "By end of sprint, [stakeholder] can [do X]" |
| Retro without action | Retro produces observations only | Require at least one action item with owner before closing retro |
| Grooming debt | Stories pulled into planning unrefined | Return to backlog; never pull unrefined stories |

---

## Agent-Specific Judgment Calls

### When to proceed vs. ask

| Situation | Action |
|-----------|--------|
| Story acceptance criteria are ambiguous | Post clarifying question on the Issue, apply `needs-clarification`, stop work |
| Story is too large to finish this sprint | Split it — file the second part as a new Issue, link to parent |
| Dependency on another Issue not yet closed | Note dependency in Issue comment, check daily, do not start blocked work |
| Sprint goal becomes unachievable mid-sprint | Flag immediately in standup comment, do not silently continue |
| Two stories conflict with each other | Stop, file a note on both Issues, request human resolution |
| Backlog is empty | Report it explicitly — do not invent work |

### What the agent never does unilaterally

- Reprioritize the backlog
- Change the sprint goal mid-sprint
- Close an Issue without meeting DoD
- Pull stories into a sprint beyond velocity
- Reassign work from one person to another
- Mark a sprint as successful when the goal was not met
