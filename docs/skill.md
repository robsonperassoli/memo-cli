# Skill

The agent skill is [skill/SKILL.md](../skill/SKILL.md). Its name is `memo`. Install it by placing that file in the agent's skills directory. Copy the directory, or symlink it, so the destination is `.../skills/memo/SKILL.md`:

```sh
ln -s "$PWD/skill" /path/to/agent/skills/memo
```

```sh
mkdir -p /path/to/agent/skills
cp -R skill /path/to/agent/skills/memo
```

The skill tells an agent to search before a task and to record durable facts, decisions, conventions, gotchas, and infrastructure facts. It does not ask the agent to store secrets.

Check that the installed binary matches the skill:

```sh
memo search --help
memo schema
memo search deploy --scope my-project
```

`memo search --scope` includes that scope and `global`. `memo schema` lists the `memories` view and the `memories_fts` table. Run `memo schema` before `memo sql`. Command details are in [Commands](commands.md).
