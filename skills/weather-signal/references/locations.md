# Location Setup

CLI-only for writes. MCP can list but cannot add or remove.

## Geocode

```bash
weather-signal geocode "London" --country GB --count 3 --table
weather-signal geocode "Paris" --country FR --count 3
```

## Saved places

```bash
weather-signal places add store-london "London" --country GB
weather-signal places list --table
weather-signal places remove store-london
```

## Alias naming

Prefer specific business aliases:
- `store-london-west`
- `warehouse-birmingham`
- `region-uk-south`

Avoid ambiguous aliases:
- `london`
- `north`
- `store1`

## Smoke test

```bash
weather-signal signal store-london --days 1 --table
```

## Guardrails

- Do not save a location if geocoding returns multiple plausible candidates without country/admin confirmation.
- Use `--config <path>` for temporary or test setups.
- Mention config path when creating or changing saved places.
