# Revaer Helm Chart

This chart deploys the Revaer application container and assumes PostgreSQL is
managed separately. Revaer requires a `DATABASE_URL`; provide it directly in
values for development or reference an existing Kubernetes Secret for real
deployments.

## Install

```bash
helm install revaer oci://ghcr.io/revaer/charts/revaer \
  --version 0.1.0 \
  --set database.url=postgres://revaer:revaer@postgres.default.svc.cluster.local:5432/revaer
```

Using an existing secret:

```bash
kubectl create secret generic revaer-db \
  --from-literal=DATABASE_URL=postgres://revaer:revaer@postgres.default.svc.cluster.local:5432/revaer

helm install revaer oci://ghcr.io/revaer/charts/revaer \
  --version 0.1.0 \
  --set database.existingSecret=revaer-db
```

## First-Run Setup

Revaer starts in setup mode and the API bind guard remains loopback-only until
the instance is activated. The chart therefore uses in-container exec probes,
but cluster Services will not reach the API until the bind address is changed
out of setup mode.

Use a pod or deployment port-forward for the initial setup flow:

```bash
kubectl port-forward deployment/revaer 7070:7070 8080:8080
```

After completing setup, update the Revaer app profile bind address through the
UI or CLI so the Service can route traffic normally.

## Signature Verification

Published GitHub releases include:

- `revaer-<version>.tgz`
- `revaer-<version>.tgz.prov`
- `revaer-helm-public.asc`
- `revaer-helm-public.gpg`

Verify a release package before installation:

```bash
curl -LO https://github.com/VannaDii/Revaer/releases/download/v0.1.0/revaer-0.1.0.tgz
curl -LO https://github.com/VannaDii/Revaer/releases/download/v0.1.0/revaer-0.1.0.tgz.prov
curl -LO https://github.com/VannaDii/Revaer/releases/download/v0.1.0/revaer-helm-public.gpg

helm verify ./revaer-0.1.0.tgz --keyring ./revaer-helm-public.gpg
helm install revaer ./revaer-0.1.0.tgz \
  --verify \
  --keyring ./revaer-helm-public.gpg \
  --set database.existingSecret=revaer-db
```

Artifact Hub verified-publisher and official badges remain manual Artifact Hub
control-plane steps after the OCI repository is registered. Use
`revaer-logo.png` as the Artifact Hub repository and organization logo when
completing that setup.

## Key Values

- `database.url`: Inline PostgreSQL connection string used to create a Secret.
- `database.existingSecret`: Existing Secret containing `DATABASE_URL`.
- `image.repository`: Image repository to deploy.
- `image.tag`: Optional override. When omitted, the chart uses `appVersion`.
- `service.type`: Kubernetes Service type for the API/UI service.
- `configPersistence.*`: Persistent volume controls for `/config`.
- `dataPersistence.*`: Persistent volume controls for `/data`.
