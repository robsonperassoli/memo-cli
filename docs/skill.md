# Skill

Install the agent skill in one step. Copy or symlink the `skill` directory into the agent's skills directory so the destination contains `SKILL.md`:

```
ln -s "$PWD/skill" /path/to/agent/skills/memo
```

Verify that this binary matches [skill/SKILL.md](../skill/SKILL.md):

```
memo search --help
memo schema
memo search deploy --scope my-project
```

`memo search --scope` includes that scope and `global`. `memo schema` lists the `memories` view and `memories_fts`. Run `memo schema` before `memo sql`.
