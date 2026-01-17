# OpenAPI Reference

> Canonical machine-readable description of the Revaer control plane surface.

The generated OpenAPI specification lives alongside the documentation at `docs/api/openapi.json` and is served by the API at `/docs/openapi.json`.

Regenerate it with:

```bash
just api-export
```

After refreshing the file, rebuild the documentation (`just docs`) to publish the updated schema and LLM manifests.
