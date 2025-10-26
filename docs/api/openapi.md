# OpenAPI Reference

> Canonical machine-readable description of the Revaer control plane surface.

The generated OpenAPI specification lives alongside the documentation at [`docs/api/openapi.json`](openapi.json). Regenerate it with:

```bash
just api-export
```

Once refreshed, rebuild the documentation (`just docs`) to publish the updated schema to the static site and LLM manifests. API consumers can download the JSON directly from the deployed documentation site or via the repository.
