# Output Contracts

Weather Signal is JSON-first. Human-readable table output and CSV are available
for inspection and export, but agents should prefer JSON unless a downstream
system requires another format.

## CLI JSON

Successful weather commands include the source, resolved location, cache state,
and command-specific payload.

```json
{
  "source": "open-meteo",
  "location": {
    "name": "London",
    "country_code": "GB",
    "latitude": 51.50853,
    "longitude": -0.12574
  },
  "cache": "miss",
  "days": []
}
```

Common fields:

| Field | Meaning |
| --- | --- |
| `source` | Upstream data source, currently `open-meteo` |
| `location` | Resolved place and coordinates used for the request |
| `cache` | `hit`, `miss`, or `refresh` |
| `fetched_at` | Render time in UTC for derived signal envelopes |

On CLI failure with JSON output, the process exits non-zero and writes an error
object to stderr:

```json
{
  "error": "failed to fetch weather data",
  "causes": ["API returned transient status 503"]
}
```

## Batch JSON

`batch signal` returns one item per requested location. A failed location does
not discard successful items.

```json
{
  "source": "open-meteo",
  "items": [
    {
      "input": "London",
      "country": "GB",
      "signal": {}
    },
    {
      "input": "Unknown",
      "error": "no geocoding result found"
    }
  ]
}
```

The item-level `country` field is the requested/default country hint. Use each
item's `signal.location` as the resolved country and coordinate source of truth.

## MCP Tool Results

MCP tools return normal MCP tool results on success. The text content is a JSON
payload matching the CLI command contract where practical.

Tool failures are surfaced as MCP tool errors (`isError: true`) so clients do
not need to parse a successful payload to detect failure.

## Table Output

`--table` is for local inspection. It is stable enough for humans, but scripts
should not parse it.

```bash
weather-signal summary london --country GB --days 7 --table
```

## CSV Output

Use `--output csv` for spreadsheet or feature-store ingestion.

```bash
weather-signal daily "51.5074,-0.1278" --days 7 --output csv
```

CSV rows are derived from the selected command surface. Prefer JSON when nested
fields, cache state, or resolved location metadata are required.
