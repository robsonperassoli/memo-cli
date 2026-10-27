# Skill

The agent skill is [skill/SKILL.md](../skill/SKILL.md). Its name is `memo`. Install it by placing that file in the agent's skills directory. Copy the directory, or symlink it, so the destination is `.../skills/memo/SKILL.md`:

From the repository root, symlink or copy `skill/` so the destination contains `SKILL.md`. Replace `./agent/skills` with the agent's skills directory:

```sh
mkdir -p ./agent/skills
ln -s "$PWD/skill" ./agent/skills/memo
test -f ./agent/skills/memo/SKILL.md
```

```sh
rm -rf ./agent-copy
mkdir -p ./agent-copy/skills
cp -R skill ./agent-copy/skills/memo
test -f ./agent-copy/skills/memo/SKILL.md
```

The skill tells an agent to search before a task and to record durable facts, decisions, conventions, gotchas, and infrastructure facts. It does not ask the agent to store secrets.

Check that the installed binary matches the skill:

```sh
memo --db ./memo.db search --help
memo --db ./memo.db schema
memo --db ./memo.db search deploy --scope my-project
```

`memo search --scope` includes that scope and `global`. `memo schema` lists the `memories` view and the `memories_fts` table. Run `memo schema` before `memo sql`. Command details are in [Commands](commands.md).
