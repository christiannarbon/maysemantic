# Configuring May Secrets

The `may_secrets` providers are configured via a simple YAML schema that dictates how the semantic layer accesses and authenticates with HashiCorp Vault.

You have two operation modes to choose from:
- **`managed`**: You are running in May Cloud or a fully managed environment. May automatically injects the Vault address and credentials.
- **`byov` (Bring Your Own Vault)**: You are hosting your own semantic layer and want it to connect to your own existing HashiCorp Vault cluster.

---

## 1. Managed Mode

When operating in managed mode, you only need to provide an authentication token. May's infrastructure handles routing to the correct internal Vault instance.

```yaml
# may_secrets.yaml
mode: managed
auth_method: token
token: "hvs.CAESIKx_abcdefgh1234567890..."
```

---

## 2. BYOV (Bring Your Own Vault) Mode

When bringing your own vault, you must provide the Vault's HTTP(S) address. You can authenticate either with a static token or via AppRole.

### With AppRole (Recommended for Production)
Using AppRole ensures that short-lived tokens are dynamically requested and rotated.

```yaml
# may_secrets.yaml
mode: byov
vault_address: "https://vault.mycompany.internal:8200"
vault_mount: "semantic-layer-secrets"
auth_method: approle
role_id: "00000000-0000-0000-0000-000000000000"
secret_id: "11111111-1111-1111-1111-111111111111"
cache_ttl_secs: 600
```

### With Static Token (Development Only)
```yaml
# may_secrets.yaml
mode: byov
vault_address: "http://localhost:8200"
vault_mount: "secret"
auth_method: token
token: "hvs.CAESIKx_abcdefgh1234567890..."
```

---

## Kubernetes ConfigMap Usage

In Kubernetes, you can inject this configuration as a ConfigMap, mounting it as a volume to the May semantic layer deployment.

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: may-secrets-config
data:
  may_secrets.yaml: |
    mode: byov
    vault_address: "https://vault.mycompany.internal:8200"
    vault_mount: "semantic-layer-secrets"
    auth_method: approle
    role_id: "00000000-0000-0000-0000-000000000000"
    secret_id: "11111111-1111-1111-1111-111111111111"
    cache_ttl_secs: 600
```

---

## Helm Chart Usage

If you are deploying the May semantic layer via the official Helm chart, you can populate the secrets configuration block in your `values.yaml` file:

```yaml
# values.yaml
secrets:
  enabled: true
  config:
    mode: byov
    vault_address: "https://vault.mycompany.internal:8200"
    vault_mount: "semantic-layer-secrets"
    auth_method: approle
    role_id: "00000000-0000-0000-0000-000000000000"
    secret_id: "11111111-1111-1111-1111-111111111111"
```
