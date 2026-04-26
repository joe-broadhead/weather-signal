# Agent Skills

Weather Signal includes three agent skills under `.github/skills/`:

```text
.github/skills/weather-signal/SKILL.md
.github/skills/weather-demand-signals/SKILL.md
.github/skills/weather-location-setup/SKILL.md
```

## Skills

- `weather-signal`: core MCP/CLI usage, command selection, output modes, and
  evidence standards.
- `weather-demand-signals`: using weather outputs as demand forecasting features
  without overstating causal claims.
- `weather-location-setup`: saved-place setup for stores, warehouses, regions,
  and repeatable business locations.

## References

The core Weather Signal skill uses small reference files that agents load only when needed:

- `transport-mcp.md`
- `transport-cli.md`
- `output-contracts.md`
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
