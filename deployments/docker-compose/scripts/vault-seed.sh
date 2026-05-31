#!/bin/sh
set -eu

VAULT_ADDR="${VAULT_ADDR:-http://may-vault:8200}"
export VAULT_ADDR
export VAULT_TOKEN=${VAULT_DEV_ROOT_TOKEN_ID}

echo "Waiting for Vault to be ready..."
RETRIES=0
MAX_RETRIES=30
until wget --spider -q "${VAULT_ADDR}/v1/sys/health"; do
    RETRIES=$((RETRIES + 1))
    if [ "$RETRIES" -ge "$MAX_RETRIES" ]; then
        echo "ERROR: Vault did not become ready after ${MAX_RETRIES} attempts. Aborting."
        exit 1
    fi
    echo "Waiting for vault... ($RETRIES/$MAX_RETRIES)"
    sleep 2
done
echo "Vault is ready."

# Enable KV v2 secrets engine if not already enabled at secret/
vault secrets list | grep -q '^secret/' || vault secrets enable -path=secret kv-v2

# Write test development secrets
echo "Writing secrets..."

# PostgreSQL credentials
echo "Writing pagila secrets..."
vault kv put secret/dev/pagila \
    host="postgres.internal" \
    user="postgres" \
    password="mysecretpassword" \
    database="pagila"

# GCP service account key
echo "Writing bigquery secrets..."
vault kv put secret/dev/bigquery \
    type="service_account" \
    project_id="may-analytics-dev" \
    private_key_id="dummy-key-id" \
    private_key="-----BEGIN PRIVATE KEY-----\ndummy\n-----END PRIVATE KEY-----\n" \
    client_email="dev-sa@may-analytics-dev.iam.gserviceaccount.com"

# Snowflake credentials
echo "Writing snowflake secrets..."
vault kv put secret/dev/snowflake \
    account="xyz12345.us-east-1" \
    user="may_service" \
    password="my-snowflake-password" \
    warehouse="COMPUTE_WH" \
    database="ANALYTICS" \
    role="SYSADMIN"

echo "Vault seed completed successfully."
