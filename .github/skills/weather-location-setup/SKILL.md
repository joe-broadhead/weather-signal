---
name: weather-location-setup
description: Configure and validate saved business locations for Weather Signal using geocoding, country hints, aliases, and cache/config checks. Use when users ask to set up stores, regions, warehouses, or recurring locations for weather-signal workflows.
license: MIT
allowed-tools: "Bash Read"
metadata:
  owner: "weather-signal"
  version: "0.0.0"
---

# Weather Location Setup Skill

## Mission

Create repeatable, unambiguous location aliases for agent weather workflows.

## Required workflow

1. Run `geocode` with `--country` when the place name is ambiguous.
2. Confirm the resolved `country_code`, `admin1`, latitude, longitude, and timezone.
3. Save the location with a stable business alias.
4. List places to verify the config.
5. Run a one-day `signal` smoke test for the alias.
6. Use `--config <path>` and isolated `XDG_CONFIG_HOME` for temporary setup or
   tests.

## Commands

```bash
weather-signal geocode "London" --country GB --count 3 --table
weather-signal places add store-london "London" --country GB
weather-signal places list --table
weather-signal signal store-london --days 1 --table
```

## Alias naming

Prefer specific aliases:

- `store-london-west`
- `warehouse-birmingham`
- `region-uk-south`

Avoid aliases that are ambiguous across business contexts:

- `london`
- `north`
- `store1`

## Guardrails

- Do not save a location if geocoding returns multiple plausible candidates and
  the user has not specified country/admin area.
- Do not hardcode customer-specific private location data in docs or tests.
- Use `--config <path>` for temporary or test setups.
- Mention the config path when creating or changing saved places.
- Saved place mutation is CLI-only; MCP can list places but does not add or
  remove them.

## When more detail is needed

Read `.github/skills/weather-signal/references/transport-cli.md`.
