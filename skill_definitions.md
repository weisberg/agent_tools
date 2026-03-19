# Sample SKILL definitions
Saved in the `./skills/` directory, these files define the tools that agents can use. 

[x] CLAUDE.md Author (`claude-md-author`): Provides detailed documentation for creating and improving the CLAUDE.md file, which serves as the primary reference for agents on how to use the tools in this repo. This skill should include instructions on how to invoke each tool, expected input/output formats, and example use cases.
    [x] SKILL.md
    [ ] references/
[ ] Skill Author (`skill-author`): Responsible for creating and maintaining the individual skill definition files in the `./skills/` directory. This includes defining the tool's interface, expected input/output, and any specific instructions or constraints for its use.
    [ ] SKILL.md
    [ ] references/
    [ ] references/frontmatter-guide.md
    [ ] references/instruction-patterns.md
    [ ] validation-checklist.md

[x] Scrum Master (`scrum-master`): Facilitates the agile development process for this repo, ensuring that tasks are well-defined, prioritized, and completed efficiently. This skill should include capabilities for sprint planning, task assignment, and progress tracking.
    [x] SKILL.md
    [ ] references/
[x] Github Issues (`github-issues`): 
    [x] SKILL.md
    [ ] references/
[ ] Github Wiki (`github-wiki`): Manages the creation and updating of GitHub wiki pages.
    [ ] SKILL.md
    [ ] references/
[ ] Github Pull Requests (`github-pull-requests`): Manages the creation, updating, and merging of GitHub pull requests.
    [ ] SKILL.md
    [ ] references/


## Third-Party Skills

https://github.com/anthropics/skills/tree/main/skills

[x] `skill-creator`
    [x] SKILL.md
    [x] references/
    [x] references/output-patterns.md
    [x] references/workflows.md
    [x] scripts/
    [x] scripts/init_skill.py
    [x] scripts/package_skill.py
    [x] scripts/quick_validate.py
