# Agent Skills

Weather Signal includes one consolidated agent skill under `skills/`:

```text
skills/weather-signal/SKILL.md
```

## Skill

- `weather-signal`: MCP/CLI usage, command selection, demand forecasting
  features, saved-place setup, output modes, and evidence standards.

## References

The skill uses small reference files that agents load only when needed:

- `cli.md`
- `demand-signals.md`
- `locations.md`
- `forecasting-workflows.md`

## Typical Agent Prompt

```text
Use weather-signal to fetch the next 7 days of demand weather signals for
London, GB. Summarize the dates with rain risk, warm days, and any caveats.
```

## Validation

Before publishing a skill update, run:

```bash
cargo test --locked --all-features
mkdocs build --strict
```
