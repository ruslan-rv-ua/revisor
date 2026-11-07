# revisor

- **Мова.** Задачі Backlog.md (назви, описи, критерії приймання, плани, нотатки, підсумки), бриф і документи —
  українською. Код і коментарі в коді — англійською. **Повідомлення комітів — англійською.**
- **Рішення ухвалює користувач.** Під час планування не вирішуй сам: за будь-якого сумніву одразу питай — по одному
  питанню, з варіантами й позначеним рекомендованим.
- **git flow** (git-flow-next, класична схема): працюємо в `feature/*` і `bugfix/*` від `develop`; у `main` напряму
  нічого не комітимо; теги версій — `v1.2.3`.
- **Backlog.md — лише через MCP.** Файли в `backlog/` вручну не редагуємо. Того, чого в MCP немає (decisions,
  архівування документів, `backlog init`), через CLI без згоди користувача не роби.

<!-- BACKLOG.MD MCP GUIDELINES START -->
<!-- backlog.md-instructions-version: 1.53.0 -->
<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

**CRITICAL GUIDANCE**

- If your client supports MCP resources, read `backlog://workflow/overview` to understand when and how to use Backlog for this project.
- If your client only supports tools or the above request fails, call `backlog.get_backlog_instructions()` to load the tool-oriented overview. Use the `instruction` selector when you need `task-creation`, `task-execution`, or `task-finalization`.

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

These guides cover:
- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and finalization
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>
<!-- BACKLOG.MD MCP GUIDELINES END -->
