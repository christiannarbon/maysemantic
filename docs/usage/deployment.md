# Deployment Guide

The May Semantic Layer offers two primary deployment methods: Docker Compose for local environments and Helm for Kubernetes production clusters.

Both methods are configured to prioritize security by requiring explicit credentials during setup. Hardcoded defaults have been removed to prevent accidental insecure deployments.

## Docker Compose

The Docker Compose stack is located in `deployments/docker-compose/`.

### Configuration
1. Navigate to the `deployments/docker-compose/` directory.
2. Copy the sample environment file:
   ```bash
   cp .env.example .env
   ```
3. Open `.env` and provide a strong password for the Identity DB:
   ```env
   IDENTITY_DB_PASSWORD=your_secure_password_here
   ```
   *Note: The stack will fail to start if this password is not provided.*

### Starting the Stack
```bash
docker compose up -d
```

## Helm Chart (Kubernetes)

The Helm chart is located in `deployments/helm/may/`.

### Installation
During installation, you must explicitly pass the `identityDb.password` value. The chart uses the `required` function to prevent rendering if this value is omitted.

```bash
helm upgrade --install may ./deployments/helm/may \
  --set identityDb.password="your_secure_password_here" \
  --namespace may-semantic --create-namespace
```

Alternatively, you can provide the password via a custom `values.yaml` overlay.

### Migrations
The Helm deployment runs an initialization Job that applies the latest database migrations. It uses a standalone `may-migrate` Docker image built from `deployments/helm/may/migrate.Dockerfile`.

## Environment Variables

| Variable | Description | Default | Supported Values |
| --- | --- | --- | --- |
| `MAY_DIALECT` | Target SQL dialect for semantic compilation | `postgres` | `postgres`, `snowflake`, `bigquery` |
| `MAY_MODEL_PATH` | Path to directory containing semantic model YAML files | *(unset)* | File system directory path |
| `MAY_JWT_SECRET` | Secret key used to sign and verify JWT tokens | *(required)* | Any secure string |
| `IDENTITY_DB_PASSWORD` | Password for the identity PostgreSQL database | *(required)* | Any secure password |
| `MAY_ADMIN_PASSWORD` | Initial password for the seeded admin user | `changeme` | Any secure password |

