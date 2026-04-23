# **Technical Specification and Functional Paradigm of notionli: An Agent-First Command-Line Interface for the Notion Ecosystem**

The digital workspace has transitioned from a document-centric repository to an autonomous operating system where the primary users are increasingly non-human entities.1 As organizational productivity frameworks integrate artificial intelligence (AI) at the core of their workflows, the traditional browser-based interface has become a bottleneck for efficiency, latency, and reliability. The development of notionli addresses this fundamental shift by providing a high-performance, low-latency command-line interface (CLI) specifically optimized for tool-using AI agents, while maintaining the ergonomics required for human power users. This report provides a comprehensive architectural and functional roadmap for notionli, grounded in the latest 2025 and 2026 Notion API updates.

## **Architectural Philosophy: Why CLI Supersedes Traditional Protocols**

The emergence of the Model Context Protocol (MCP) in late 2024 initially suggested a universal standard for connecting AI agents to external services. However, by 2026, industry benchmarks and deployment cycles have demonstrated that CLI-based orchestration offers significant advantages over MCP in professional environments.3 The primary driver for this shift is "context rot"—the degradation of agent performance when context windows are flooded with the massive JSON-RPC schemas and tool definitions inherent in MCP.5

Comparative research into agentic performance indicates that CLI operations are ten to thirty-two times cheaper in terms of token consumption than their MCP counterparts.3 This efficiency stems from the "stateless" nature of CLI commands; instead of loading an entire service schema into the agent's memory, the agent executes a targeted command, receives a structured JSON response, and moves to the next logical step.3 Furthermore, LLMs have been trained on decades of shell scripting and Unix piping conventions, making the "composability grammar" of a CLI natively intuitive to modern models.3

| Performance Metric | CLI-Based Agents | MCP-Based Agents |
| :---- | :---- | :---- |
| Reliability (Average) | \~100% | \~72% 3 |
| Token Overhead | Minimal (Command/Output) | Massive (Schema/Handshake) 3 |
| Execution Latency | \<50ms (Local/Stateless) | \>150ms (Remote/Stateful) 4 |
| Composability | Native Unix Pipes (grep, jq) | Protocol-Specific Handlers 3 |

notionli is designed as a single binary tool, written in a memory-safe language like Rust or Go, to eliminate the dependency fragility common in Python-based environments.7 By avoiding the "minefield" of missing packages and virtual environment conflicts, notionli ensures that agents operating in arbitrary or sandboxed environments can reliably interact with the Notion workspace.8

## **Authentication and Security Models in Headless Environments**

Security is the foremost concern for any tool that grants programmatic access to an organizational knowledge base. notionli implements a tiered authentication model that supports human interactive login and headless agent execution.

### **Tiered Authentication Strategies**

For human users, the auth login command utilizes the OAuth 2.0 flow, redirecting to the Notion dashboard to grant explicit permissions.9 This process generates an access token that is persisted locally in \~/.config/notionli/config.json.7 However, for AI agents and CI/CD pipelines, notionli prioritizes internal integration tokens via environment variables (NOTION\_API\_KEY) to bypass the need for a local browser.9

| Auth Method | Use Case | Mechanism | Credentials Life |
| :---- | :---- | :---- | :---- |
| OAuth 2.0 | Human Interactive Use | Device Flow / Redirect | 1 Hour (Auto-refresh) 13 |
| Internal Token | Headless/Serverless Agents | Static API Token | Long-lived (Static) 9 |
| Environment Var | CI/CD & Ephemeral Scripts | NOTION\_API\_KEY | Runtime Only 12 |

### **The Secret Management Paradigm**

To mitigate the risk of plain-text credential leaks, notionli integrates with the AgentSecrets pattern, ensuring that keys are not stored on disk in the standard configuration file.15 By leveraging OS-level keychains or secure secret injection at runtime, the CLI prevents malicious processes from harvesting Notion access tokens from the local file system.15 For enterprise deployments, notionli supports Client ID Metadata Document (CIMD) per the MCP 2025-11-25 specification, allowing clients to use an HTTPS URL as their client\_id for verified organizational access.2

## **Data Source Orchestration: Navigating the 2025 Data Model**

The most significant architectural shift in Notion’s history occurred in September 2025 with the introduction of "Data Sources".16 Historically, a Notion database was a single table with one set of properties. The 2025 update redefined the "Database" as a container block that can house multiple "Data Sources," each with its own independent schema and relationship logic.17

### **The Container-Source Relationship**

In the notionli paradigm, developers and agents must distinguish between the container (database\_id) and the data provider (data\_source\_id). The CLI's database commands are optimized to handle this distinction automatically, introspecting the database container to identify the correct data source for a given operation.16

| Entity | Role in notionli | API Version (2025-09-03) |
| :---- | :---- | :---- |
| **Database** | Container/Hub | Repurposed to return an array of data sources. 19 |
| **Data Source** | Actual Table/Schema | New home for property definitions and page entries. 16 |
| **Relation** | Cross-Source Link | Now requires data\_source\_id for precise identification. 19 |

This architectural change enables "Massive Workspace Consolidation".17 An agent using notionli can manage a "Project Hub" database where a "Task" data source (with due dates and assignees) lives alongside a "Financials" data source (with currency and tax properties), all under one unified root block.17 The CLI supports moving data sources between containers without breaking linked relations, a feature critical for scaling teams that frequently restructure their workspaces.17

## **Enhanced Markdown: The Core of Agentic Interaction**

The release of the Notion API version 2026-03-11 introduced "Enhanced Markdown" (also known as Notion-flavored Markdown), which serves as the primary data exchange format for notionli.21 This format allows agents to read and write entire pages as single strings, bypassing the need to construct complex JSON block trees.21

### **Technical Specification of Enhanced Markdown Tags**

Enhanced Markdown extends standard Markdown with XML-like tags and attribute lists to represent Notion-specific features such as callouts, toggles, and block-level colors.22

| Feature | Enhanced Markdown Syntax | Architectural Benefit |
| :---- | :---- | :---- |
| **Callout** | \<callout icon="emoji" color="blue"\>Text\</callout\> | Allows multi-block children within a callout. 22 |
| **Toggle** | \<details\>\<summary\>Title\</summary\>Content\</details\> | Native representation of toggle blocks. 22 |
| **Columns** | \<columns\>\<column\>... \</column\>\</columns\> | Simplified multi-column layout management. 22 |
| **Table** | \<table\>\<tr\>\<td\>Cell\</td\>\</tr\>\</table\> | Standardized table structure with color support. 22 |
| **Mentions** | \<mention-user url="URL"/\> | Accurate user and date referencing. 22 |

The page view \<id\> \--md command retrieves the full content of a page, including child blocks, recursively rendered as Markdown.7 This enables a "Read-Reason-Update" cycle for agents: an agent reads the page, modifies the Markdown string, and uses the page update \<id\> \--md command to apply the changes.21 This approach is significantly more efficient than the legacy block-based API, which required multiple GET requests to retrieve nested children and multiple PATCH requests to update specific blocks.25

### **Handling Large-Scale Page Content**

Notion's data volume is update-heavy, with 90% of upserts being updates to existing blocks.26 As workspaces grow to encompass hundreds of billions of blocks, notionli implements optimizations to handle very large pages (those with several thousand blocks).21 If a page's Markdown output is truncated, the CLI returns a truncated: true flag along with an unknown\_block\_ids array, allowing agents to fetch specific missing blocks via the block-based API endpoints.21

## **Functional Specification: The 39 Core Commands**

To provide complete coverage of the Notion API, notionli implements 39 primary commands organized into logical groups.7

### **Page and Database Management**

The page and db (database) command groups handle the lifecycle of the most critical Notion entities.7

* **page create**: Supports creating pages directly from Markdown files or stdin. It auto-extracts the first \# H1 as the page title if none is specified.21  
* **page move**: Relocates pages between parents or to the workspace root, a new capability in the 2025 API version.19  
* **db query**: Supports human-friendly filtering (e.g., \--filter 'Status=Done') and schema-awareness, which translates string inputs into the correct property types (select, multi-select, number, etc.).7  
* **db add-bulk**: Accepts NDJSON via stdin to perform high-speed database entry creation, essential for data ingestion and synchronization tasks.5

### **Block-Level Operations**

For fine-grained content manipulation, the block group provides direct access to the individual components of a page.7

| Command | Action | 2026 API Update |
| :---- | :---- | :---- |
| block list | Lists children of a block. | Supports deep recursion with \--depth. 7 |
| block insert | Adds blocks at a specific position. | Uses new position object (start/end/after\_block). 28 |
| block update | Modifies block content. | Handles meeting\_notes and synced\_block updates. 25 |
| block delete | Trashes a block. | Replaces archived: true with in\_trash: true. 28 |

### **Discussion and User Intelligence**

The comment and user groups allow agents to participate in workspace communication and identify team structures.7

* **comment add**: Supports adding comments to pages or specific blocks. The 2026 update allows for real Notion user mentions within comment bodies via the \--mention-user flag.2  
* **user me**: Retrieves the metadata for the integration's bot, including its max\_file\_upload\_size and workspace-level permissions.25  
* **team list**: A new 2026 command that lists the teamspaces available to the integration, facilitating organizational-level content discovery.12

## **Agentic Interface Design: The Eight Rules of notionli**

For a CLI to be "agent-ready," it must prioritize machine-readability and deterministic control flow.5 notionli adheres to eight core design rules derived from industry best practices for agentic tool calling.

### **1\. Structured Output is Non-Negotiable**

All primary output is directed to stdout as JSON. Human-friendly formatting, progress spinners, and diagnostic warnings are sent to stderr.5 This allows an agent to pipe notionli output directly into a JSON parser without filtering out text noise.7

### **2\. Meaningful Exit Codes**

The CLI uses specific exit codes to signal the nature of a failure, enabling the agent's orchestrator to decide whether to retry, escalate, or fail-fast.5

| Exit Code | Meaning | Agent Action |
| :---- | :---- | :---- |
| 0 | Success | Proceed to next step. |
| 1 | General Error | Log and abort. |
| 3 | Not Found | Check ID/Permissions. 12 |
| 4 | Validation Error | Correct input schema. 19 |
| 5 | Rate Limited | Exponential backoff. 12 |

### **3\. Idempotency and Conflict Detection**

Destructive or additive commands are designed to be idempotent where possible. If a command cannot be made idempotent (e.g., creating a page with a specific title), notionli provides a 5 (already exists) exit code so the agent can handle the conflict programmatically.5

### **4\. Self-Documenting through \--help and Schema**

Agents are trained to use \--help as their first step when encountering a new tool.5 notionli provides comprehensive, machine-parsable help text for every command and sub-command.31 Additionally, it includes a schema command that allows agents to introspect the CLI's own command structure and parameter requirements.12

### **5\. Composability and Unix Grammar**

Commands are designed to be chained. The \--quiet flag outputs only the raw ID of a created resource, which can then be piped into a subsequent command (e.g., notionli page create \--quiet | xargs notionli comment add \--text "Initial Check").5

### **6\. Dry-Run and Non-Interactive Modes**

AI agents cannot respond to "Y/N" confirmation prompts.30 The \--non-interactive (or \--yes) flag bypasses all prompts, while the \--dry-run flag returns a JSON diff of the changes that *would* occur, allowing the agent to "preview" its actions before committing them to the Notion workspace.12

### **7\. Actionable Error Messages**

Instead of opaque error strings, notionli provides error messages that include the failing input and suggested next steps.5 For example, an object\_not\_found error for a page ID will explicitly suggest checking if the page has been shared with the integration.7

### **8\. Consistent Noun-Verb Grammar**

The CLI maintains a strict notionli \<category\> \<action\> pattern (e.g., notion page get, notion db query). This consistency reduces the cognitive load on both human users and LLM reasoning engines.5

## **Human Power-User Enhancements: Bridging UI and Terminal**

While agents are the primary target, notionli provides unique features that address long-standing UI pain points for human power users.32

### **Advanced Terminal Rendering**

To make the terminal a viable alternative to the browser, notionli integrates with libraries like Rich and Glow to render Notion content with high fidelity.35

* **TUI Mode**: When used interactively, notionli page view provides a full-terminal interface with syntax highlighting for code blocks, styled headings, and mouse-supported navigation.7  
* **Markdown Streaming**: For long pages, the CLI implements "Streaming Markdown" rendering. Instead of waiting for the full page to download, it renders finalized blocks (headers, paragraphs) as they arrive via the API, minimizing perceived latency.37  
* **Color and Layout**: Terminal output respects the color and background\_color attributes of Notion blocks, utilizing the 16 or 256 colors available in modern terminal emulators.22

### **The Global "Find and Replace" Solution**

A major frustration for Notion users is the lack of a native "Find and Replace" tool across pages and databases.34 notionli fills this gap with the page edit command.12 This utility can perform literal or regex-based text replacement across a single page, a database, or an entire workspace branch.12 This is achieved through a high-speed tree traversal that identifies matches and issues targeted PATCH requests to the block-level API.26

| Human UI Pain Point | notionli Solution | mechanism |
| :---- | :---- | :---- |
| No Find and Replace | page edit command | Regex-based block tree traversal. 12 |
| Slow Page Loading | page view \--raw | Bypasses heavy JS rendering; direct API-to-text. 14 |
| Difficult Data Entry | db add-bulk | Batch import via CSV/JSON pipe. 12 |
| Crowded Sidebar | search command | Fast, query-based discovery without folders. 7 |

## **Productivity and Automations: The notionli Ecosystem**

The true power of notionli lies in its ability to facilitate complex, automated workflows that span personal and professional contexts.

### **The P.A.R.A. and Repository Methods**

Power users often implement organizational frameworks like the P.A.R.A. (Projects, Areas, Resources, Archives) method.27 notionli includes specialized commands to automate this structure, such as para move which handles the logic of archiving completed projects by updating their parent database or data source and clearing relevant status properties.27 Similarly, the "Repository" method for developers can be automated by scripts that "Quick Add" ideas from a terminal prompt and automatically tag them by priority and category based on keyword analysis.39

### **Meeting Intelligence and Action Routing**

With the 2026 introduction of the meeting\_notes (formerly transcription) block type, Notion has become a hub for meeting intelligence.2 notionli provides a meeting list command that retrieves recent transcripts and AI-generated summaries.2 An agentic script can then parse these summaries for action items and use the notionli page create command to route them into the appropriate "Tasks" database across different teamspaces.1

### **The Label Registry for Email Integration**

For users leveraging Notion Mail, notionli enables advanced "Label Registry" logic.41 An agent can query a central Notion database for labeling rules (e.g., "Any email from 'Client: Acme' should be filed as 'Priority High'") and then use the gws (Google Workspace) or AgentMail CLI to execute those rules.3 This "determinism bypass" saves credits and improves reliability by moving organizational logic into a manageable Notion database.41

## **Scaling Architecture: Handling Hundreds of Billions of Blocks**

Notion’s block table has grown to more than two hundred billion rows, with data volume doubling every six to twelve months.26 Operating at this scale requires notionli to handle expensive computations like tree traversal and permission data construction, which are performed on the fly in the Notion backend.26

### **Optimized Content Retrieval**

The search endpoint is not designed for exhaustive enumeration of all documents in a workspace.42 To handle large-scale data discovery, notionli implements the "Scout" pattern:

1. **Phase 1 (Search)**: Use the search command with specific filter\[value\]="data\_source" parameters to find relevant containers.19  
2. **Phase 2 (Query)**: Once the data\_source\_id is identified, use the db query command, which is optimized for filtering within a particular set of records.42  
3. **Phase 3 (Fetch)**: Retrieve individual page content as Enhanced Markdown, ensuring that large pages are handled via paginated requests to the pages/markdown endpoint.21

### **Permission and Trash Semantics**

The 2026 API update consolidated the "Trash" and "Archive" concepts. notionli reflects this by replacing all legacy archived fields with the in\_trash boolean in its command structure.28 This ensures that agents can accurately distinguish between active content and content that is in the "trash" (and thus hidden from search and AI responses by default).2

## **Skill Standards and AI Agent Discovery**

To be fully integrated into the modern agentic stack (Claude Code, Cursor, Codex), notionli provides a directory of "Agent Skills".43

### **The SKILL.md Standard**

A skill is a directory containing a SKILL.md file with YAML frontmatter, providing instructions that an agent loads on demand.43

## ---

**name: notion-workspace-manager description: Manage Notion pages, databases, and blocks using the notionli CLI. Use when the user asks to "create a note", "query tasks", or "summarize a workspace".**

## **Core Workflows**

1. Search: Use notionli search \<query\> to find IDs.  
2. Read: Use notionli page view \<id\> \--md to fetch content.  
3. Update: Use notionli page update \<id\> \--md to save changes.

This format ensures "progressive disclosure"—the agent only loads the full set of CLI instructions when it decides the current task requires them, thus preserving the primary context window for the user's actual request.6

### **Specialized Subagents**

The notionli ecosystem supports a collection of specialized subagents, each covering a specific slice of development or organizational work.41

* **Knowledge Capture Agent**: Transforms conversational fragments into structured Notion documentation.49  
* **Spec-to-Implementation Agent**: Analyzes tech specs in Notion and creates a sequence of concrete database tasks.49  
* **Meeting Intelligence Agent**: Gathers context from previous meetings to prepare materials for upcoming sessions.49

## **Future Outlook: The Convergence of Agents and UI**

The trajectory of the Notion platform suggests a future where the distinction between "Human UI" and "Agent CLI" continues to blur.2 The introduction of "Custom Agents" in the Notion app allows teams to build their own internal agents for email triage, task routing, and reporting.1

notionli serves as the bridge for these custom agents to interact with the broader terminal-based toolchain. By providing a "contract between agents"—where one agent publishes a clean summary page in Notion and another reads that page via the CLI—organizations can build resilient, modular agent teams that do not care about each other's internals.41

| Roadmap Tier | Feature | Timeline |
| :---- | :---- | :---- |
| **Connectivity** | Slack/Google Drive/Salesforce Connectors | Q2 2026 2 |
| **Intelligence** | AI Autofill for multi-source databases | Q2 2026 2 |
| **Experience** | Native "Tabs" block and Dashboard views | Beta Q1 2026 38 |
| **Pricing** | Credit-based scaling for custom agents | May 2026 2 |

## **Conclusions: The Architectural Imperative of notionli**

The development of notionli is not merely a utility for terminal users but a fundamental architectural response to the AI-native workspace. By prioritizing agent-first design patterns—structured output, meaningful exit codes, and Enhanced Markdown support—notionli addresses the critical token-efficiency and reliability gaps of existing protocols.

For the professional peer, the adoption of notionli enables:

1. **Exponentially Lower Credit Consumption**: Moving from high-token MCP handshakes to lean CLI execution.  
2. **Increased Workflow Reliability**: Leveraging the native shell scripting capabilities of LLMs.  
3. **Scalable Workspace Organization**: Navigating the complex multi-source data model of modern Notion.  
4. **Enhanced Developer Productivity**: Resolving UI gaps like global find-and-replace and high-speed data entry.

As Notion continues to grow into a multi-hundred-billion-block environment, the CLI remains the most robust and performant interface for both the human engineer and the autonomous agent. notionli is the definitive tool for this new era of collaborative computing.

#### **Works cited**

1. Notion: The AI workspace that works for you., accessed April 21, 2026, [https://www.notion.com/](https://www.notion.com/)  
2. Notion Release Notes \- April 2026 Latest Updates \- Releasebot, accessed April 21, 2026, [https://releasebot.io/updates/notion](https://releasebot.io/updates/notion)  
3. 10 Must-have CLIs for your AI Agents in 2026 | by unicodeveloper \- Medium, accessed April 21, 2026, [https://medium.com/@unicodeveloper/10-must-have-clis-for-your-ai-agents-in-2026-51ba0d0881df](https://medium.com/@unicodeveloper/10-must-have-clis-for-your-ai-agents-in-2026-51ba0d0881df)  
4. CLI Based AI Agent : Tool Calling with CLI | by Vishal Mysore | Mar, 2026 | Medium, accessed April 21, 2026, [https://medium.com/@visrow/cli-based-ai-agent-tool-calling-with-cli-19d773add372](https://medium.com/@visrow/cli-based-ai-agent-tool-calling-with-cli-19d773add372)  
5. Writing CLI Tools That AI Agents Actually Want to Use \- DEV Community, accessed April 21, 2026, [https://dev.to/uenyioha/writing-cli-tools-that-ai-agents-actually-want-to-use-39no](https://dev.to/uenyioha/writing-cli-tools-that-ai-agents-actually-want-to-use-39no)  
6. Progressive Disclosure in AI Agents: How to Load Context Without Killing Output Quality, accessed April 21, 2026, [https://www.mindstudio.ai/blog/progressive-disclosure-ai-agents-context-management](https://www.mindstudio.ai/blog/progressive-disclosure-ai-agents-context-management)  
7. 4ier/notion-cli: Work seamlessly with Notion from the ... \- GitHub, accessed April 21, 2026, [https://github.com/4ier/notion-cli](https://github.com/4ier/notion-cli)  
8. Agent-Friendly CLI Tools for AI inference | by Michael Yuan | Feb, 2026, accessed April 21, 2026, [https://medium.com/@michaelyuan\_88928/agent-friendly-cli-tools-for-ai-inference-8fb1018fbea4](https://medium.com/@michaelyuan_88928/agent-friendly-cli-tools-for-ai-inference-8fb1018fbea4)  
9. Overview \- Notion Docs \- Notion API, accessed April 21, 2026, [https://developers.notion.com/guides/get-started/overview](https://developers.notion.com/guides/get-started/overview)  
10. Stripe CLI Reference, accessed April 21, 2026, [https://docs.stripe.com/cli](https://docs.stripe.com/cli)  
11. Login command · stripe/stripe-cli Wiki \- GitHub, accessed April 21, 2026, [https://github.com/stripe/stripe-cli/wiki/login-command](https://github.com/stripe/stripe-cli/wiki/login-command)  
12. jjovalle99/notion-cli: Agent-friendly CLI for the Notion API ... \- GitHub, accessed April 21, 2026, [https://github.com/jjovalle99/notion-cli](https://github.com/jjovalle99/notion-cli)  
13. The developer's guide to CLI authentication \- WorkOS, accessed April 21, 2026, [https://workos.com/blog/cli-authentication-guide](https://workos.com/blog/cli-authentication-guide)  
14. lox/notion-cli: CLI for Notion using the Model Context ... \- GitHub, accessed April 21, 2026, [https://github.com/lox/notion-cli](https://github.com/lox/notion-cli)  
15. The Stripe CLI Stores Your API Key in Plaintext. Here's the Fix. \- DEV Community, accessed April 21, 2026, [https://dev.to/the\_seventeen/the-stripe-cli-stores-your-api-key-in-plaintext-heres-the-fix-3imi](https://dev.to/the_seventeen/the-stripe-cli-stores-your-api-key-in-plaintext-heres-the-fix-3imi)  
16. FAQs \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/guides/get-started/upgrade-faqs-2025-09-03](https://developers.notion.com/guides/get-started/upgrade-faqs-2025-09-03)  
17. Notion Data Sources Explained (2025): The Future of Databases in Notion \- NotionApps, accessed April 21, 2026, [https://www.notionapps.com/blog/notion-data-sources-update-2025](https://www.notionapps.com/blog/notion-data-sources-update-2025)  
18. Notion Databases Explained \+ API Changes (New Notion Data Model) \- Simone Smerilli, accessed April 21, 2026, [https://www.simonesmerilli.com/life/notion-database-data-source](https://www.simonesmerilli.com/life/notion-database-data-source)  
19. Upgrade guide \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/guides/get-started/upgrade-guide-2025-09-03](https://developers.notion.com/guides/get-started/upgrade-guide-2025-09-03)  
20. Database \- Notion Docs \- Notion API, accessed April 21, 2026, [https://developers.notion.com/reference/database](https://developers.notion.com/reference/database)  
21. Working with markdown content \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/guides/data-apis/working-with-markdown-content](https://developers.notion.com/guides/data-apis/working-with-markdown-content)  
22. Enhanced markdown format \- Notion Docs \- Notion API, accessed April 21, 2026, [https://developers.notion.com/guides/data-apis/enhanced-markdown](https://developers.notion.com/guides/data-apis/enhanced-markdown)  
23. Retrieve a page as markdown \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/reference/retrieve-page-markdown](https://developers.notion.com/reference/retrieve-page-markdown)  
24. Progressive Disclosure: The Core Engineering Philosophy of the LLM Era | by shuai zhang | Feb, 2026 | Medium, accessed April 21, 2026, [https://medium.com/@dyzsasd/progressive-disclosure-the-core-engineering-philosophy-of-the-llm-era-0a6328774404](https://medium.com/@dyzsasd/progressive-disclosure-the-core-engineering-philosophy-of-the-llm-era-0a6328774404)  
25. Block \- Notion Docs \- Notion API, accessed April 21, 2026, [https://developers.notion.com/reference/block](https://developers.notion.com/reference/block)  
26. Building and scaling Notion's data lake, accessed April 21, 2026, [https://www.notion.com/blog/building-and-scaling-notions-data-lake](https://www.notion.com/blog/building-and-scaling-notions-data-lake)  
27. I built a full CLI for Notion — 39 commands, human-friendly filters, Markdown I/O, one binary, accessed April 21, 2026, [https://www.reddit.com/r/Notion/comments/1rd9g53/i\_built\_a\_full\_cli\_for\_notion\_39\_commands/](https://www.reddit.com/r/Notion/comments/1rd9g53/i_built_a_full_cli_for_notion_39_commands/)  
28. Upgrade guide \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/guides/get-started/upgrade-guide-2026-03-11](https://developers.notion.com/guides/get-started/upgrade-guide-2026-03-11)  
29. Notion API | Documentation | Postman API Network, accessed April 21, 2026, [https://www.postman.com/notionhq/notion-s-api-workspace/documentation/y28pjg6/notion-api](https://www.postman.com/notionhq/notion-s-api-workspace/documentation/y28pjg6/notion-api)  
30. Making your CLI agent-friendly \- Speakeasy, accessed April 21, 2026, [https://www.speakeasy.com/blog/engineering-agent-friendly-cli](https://www.speakeasy.com/blog/engineering-agent-friendly-cli)  
31. Introducing the CLI Generator \- Postman Blog, accessed April 21, 2026, [https://blog.postman.com/introducing-the-cli-generator/](https://blog.postman.com/introducing-the-cli-generator/)  
32. Notion SE: A Beginner's Guide To Streamline Your Workflow \- Ftp, accessed April 21, 2026, [https://ftp.bills.com.au/lunar-tips/notion-se-a-beginners-guide-to-streamline-your-workflow-1767648965](https://ftp.bills.com.au/lunar-tips/notion-se-a-beginners-guide-to-streamline-your-workflow-1767648965)  
33. Notion CLI \- speed up your workflow : r/PKMS \- Reddit, accessed April 21, 2026, [https://www.reddit.com/r/PKMS/comments/1qw401c/notion\_cli\_speed\_up\_your\_workflow/](https://www.reddit.com/r/PKMS/comments/1qw401c/notion_cli_speed_up_your_workflow/)  
34. Is Notion ever going to get "find and replace"? \- Reddit, accessed April 21, 2026, [https://www.reddit.com/r/Notion/comments/1443zz8/is\_notion\_ever\_going\_to\_get\_find\_and\_replace/](https://www.reddit.com/r/Notion/comments/1443zz8/is_notion_ever_going_to_get_find_and_replace/)  
35. 7 lessons from building a modern TUI framework | Talk Python To Me Podcast, accessed April 21, 2026, [https://talkpython.fm/episodes/show/380/7-lessons-from-building-a-modern-tui-framework](https://talkpython.fm/episodes/show/380/7-lessons-from-building-a-modern-tui-framework)  
36. Introduction \- Glow \- Mintlify, accessed April 21, 2026, [https://mintlify.com/charmbracelet/glow/introduction](https://mintlify.com/charmbracelet/glow/introduction)  
37. Efficient streaming of Markdown in the terminal \- Will McGugan, accessed April 21, 2026, [https://willmcgugan.github.io/streaming-markdown/](https://willmcgugan.github.io/streaming-markdown/)  
38. March 26, 2026 – Notion 3.4, part 1, accessed April 21, 2026, [https://www.notion.com/releases/2026-03-26](https://www.notion.com/releases/2026-03-26)  
39. From CLI to Notion: My Developer Productivity Toolkit | by Mayank Aggarwal | Medium, accessed April 21, 2026, [https://medium.com/@mayank0255/from-cli-to-notion-my-developer-productivity-toolkit-5d1f1addb2f5](https://medium.com/@mayank0255/from-cli-to-notion-my-developer-productivity-toolkit-5d1f1addb2f5)  
40. Notion AI agents \- Reddit, accessed April 21, 2026, [https://www.reddit.com/r/Notion/comments/1qwh059/notion\_ai\_agents/](https://www.reddit.com/r/Notion/comments/1qwh059/notion_ai_agents/)  
41. I Built 11 Coordinated Notion Agents. Here's What Actually Matters. \- Reddit, accessed April 21, 2026, [https://www.reddit.com/r/Notion/comments/1rex1ze/i\_built\_11\_coordinated\_notion\_agents\_heres\_what/](https://www.reddit.com/r/Notion/comments/1rex1ze/i_built_11_coordinated_notion_agents_heres_what/)  
42. Search optimizations and limitations \- Notion Docs, accessed April 21, 2026, [https://developers.notion.com/reference/search-optimizations-and-limitations](https://developers.notion.com/reference/search-optimizations-and-limitations)  
43. Agent Skills – Codex | OpenAI Developers, accessed April 21, 2026, [https://developers.openai.com/codex/skills](https://developers.openai.com/codex/skills)  
44. AI Agent Skills \- Atmos, accessed April 21, 2026, [https://atmos.tools/ai/agent-skills](https://atmos.tools/ai/agent-skills)  
45. Agent Skills: The Universal Standard Transforming How AI Agents Work | by Rick Hightower | Spillwave Solutions, accessed April 21, 2026, [https://medium.com/spillwave-solutions/agent-skills-the-universal-standard-transforming-how-ai-agents-work-fc7397406e2e](https://medium.com/spillwave-solutions/agent-skills-the-universal-standard-transforming-how-ai-agents-work-fc7397406e2e)  
46. The SKILL.md Pattern: How to Write AI Agent Skills That Actually Work | by Bibek Poudel, accessed April 21, 2026, [https://bibek-poudel.medium.com/the-skill-md-pattern-how-to-write-ai-agent-skills-that-actually-work-72a3169dd7ee](https://bibek-poudel.medium.com/the-skill-md-pattern-how-to-write-ai-agent-skills-that-actually-work-72a3169dd7ee)  
47. skill.md explained: How to structure your product for AI agents – GitBook Blog, accessed April 21, 2026, [https://www.gitbook.com/blog/skill-md](https://www.gitbook.com/blog/skill-md)  
48. VoltAgent \- GitHub, accessed April 21, 2026, [https://github.com/voltagent](https://github.com/voltagent)  
49. heilcheng/awesome-agent-skills: Tutorials, Guides and Agent Skills Directories \- GitHub, accessed April 21, 2026, [https://github.com/heilcheng/awesome-agent-skills](https://github.com/heilcheng/awesome-agent-skills)