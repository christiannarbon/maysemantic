# May Semantic Layer — Helm Chart

This Helm chart deploys the infrastructure for the May Semantic Layer on Kubernetes, including the `may-identity-db` (PostgreSQL) and its associated schema migrations.

## Prerequisites

To test and validate this Helm chart locally, you will need:
- [Docker](https://docs.docker.com/get-docker/) installed and running.
- A local Kubernetes cluster tool like [minikube](https://minikube.sigs.k8s.io/docs/start/) or [kind](https://kind.sigs.k8s.io/docs/user/quick-start/).
- [Helm 3](https://helm.sh/docs/intro/install/)
- `kubectl`

---

## Testing & Validation Guide

### 1. Start a Local Cluster
If you don't have a cluster running, start one using Minikube:
```bash
minikube start
```

### 2. Install the Chart
From within this directory (`deployments/helm/may`), deploy the chart to your local cluster. We'll name the release `may-local`:

```bash
helm install may-local .
```

### 3. Verify the Deployment
Once installed, Helm will trigger the `pre-install` hook to run the SQLx migrations before marking the deployment as fully successful.

**Check the pods:**
You should see the Postgres `StatefulSet` pod spinning up.
```bash
kubectl get pods -l app=may-local-identity-db
```

**Check the Migration Job:**
You can verify that the migration job ran successfully:
```bash
kubectl get jobs
```

*Note: Since we configured the job with `helm.sh/hook-delete-policy: hook-succeeded`, the job pod will automatically delete itself once migrations are complete to save resources.*

### 4. Validate Database Connectivity
To ensure Postgres is accepting connections and that the credentials from the Secret are working, you can port-forward the service to your local machine:

```bash
# Forward the internal 5432 port to your local 5433 port
kubectl port-forward svc/may-local-identity-db 5433:5432
```

In a new terminal window, connect using `psql` and the default credentials from `values.yaml`:
```bash
PGPASSWORD=changeme psql -h 127.0.0.1 -p 5433 -U may_admin -d may_identity
```
*(If successful, you'll be dropped into the `may_identity=>` prompt!)*

### 5. Clean Up
To tear down the stack and delete the persistent volume claim:
```bash
helm uninstall may-local

# Note: Helm does not automatically delete Persistent Volume Claims (PVCs) by default to prevent data loss.
# To clean up the data volume, manually delete the PVC:
kubectl delete pvc data-may-local-identity-db-0
```

---

## Configuration

The default configurations are stored in `values.yaml`. For production deployments, **never** use the default passwords. Override them during installation:

```bash
helm install may-prod . \
  --set identityDb.password="your-secure-password"
```
