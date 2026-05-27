# Helm Chart Test Cases

This document outlines the manual test cases required to validate the deployment, hooks, and security properties of the May Semantic Layer Helm chart on a Kubernetes cluster.

## Prerequisites
- A running Kubernetes cluster (e.g., Minikube, kind, or a managed cloud cluster).
- `helm` CLI installed.
- `kubectl` configured to communicate with your cluster.

## Test Case 1: Secure Credential Enforcement

**Objective:** Verify that the Helm chart refuses to render or install if the Identity DB password is not explicitly provided.

**Steps:**
1. Navigate to the chart directory: `cd deployments/helm/may/`.
2. Run a dry-run install without providing a password:
   ```bash
   helm upgrade --install may . --dry-run
   ```

**Expected Result:**
- Helm returns an execution error: `execution error at (may/templates/identity-db-secret.yaml:X:Y): identityDb.password must be set`.

## Test Case 2: Successful Chart Rendering

**Objective:** Verify that the chart renders properly when required values are provided.

**Steps:**
1. Run a template command with the password provided:
   ```bash
   helm template may . --set identityDb.password="SecureK8sPass123"
   ```

**Expected Result:**
- The template renders completely without errors.
- The generated YAML includes a `Secret`, `StatefulSet`, `Service`, and a `Job`.

## Test Case 3: Initial Deployment & Migration Hook

**Objective:** Verify that the database migrations run as a `pre-install` hook before the main database comes online for consumers.

**Steps:**
1. Create a namespace: `kubectl create namespace may-test`.
2. Install the chart:
   ```bash
   helm upgrade --install may . -n may-test \
     --set identityDb.password="SecureK8sPass123" \
     --wait
   ```
3. Check the jobs in the namespace:
   ```bash
   kubectl get jobs -n may-test
   ```
4. View the logs of the migration pod:
   ```bash
   kubectl logs job/may-identity-db-migrate -n may-test
   ```

**Expected Result:**
- The migration job completes successfully.
- The logs show `sqlx` applying the schema correctly to the internal Postgres instance.

## Test Case 4: StatefulSet & Probes Validation

**Objective:** Verify that the Postgres database comes online and satisfies both readiness and liveness probes.

**Steps:**
1. Check the StatefulSet pods:
   ```bash
   kubectl get pods -n may-test -l app=may-identity-db
   ```
2. Describe the pod to check probe events:
   ```bash
   kubectl describe pod may-identity-db-0 -n may-test
   ```

**Expected Result:**
- The `may-identity-db-0` pod achieves a `1/1` Ready state.
- No continuous `Unhealthy` events appear in the pod's event log regarding the readiness or liveness probes.

## Test Case 5: Secret Mounting

**Objective:** Verify that the Kubernetes Secret was correctly generated and mounted into the pod.

**Steps:**
1. Execute into the Postgres container:
   ```bash
   kubectl exec -it may-identity-db-0 -n may-test -- sh
   ```
2. Run `env | grep POSTGRES_PASSWORD` inside the shell.

**Expected Result:**
- The environment variable outputs `SecureK8sPass123`, confirming the secure hand-off from Helm `--set` to the Kubernetes Secret to the Container env.

## Test Case 6: Uninstall and Cleanup

**Objective:** Verify that Helm successfully tracks and tears down the deployed resources.

**Steps:**
1. Uninstall the release:
   ```bash
   helm uninstall may -n may-test
   ```
2. Verify resources are removed:
   ```bash
   kubectl get all -n may-test
   ```

**Expected Result:**
- All pods, services, statefulsets, and jobs belonging to the release are terminated and removed from the namespace.
