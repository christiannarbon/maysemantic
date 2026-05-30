#!/bin/sh
set -e

export VAULT_ADDR='http://may-vault:8200'
export VAULT_TOKEN=${VAULT_DEV_ROOT_TOKEN_ID}

echo "Waiting for Vault to be ready..."
until wget --spider -q ${VAULT_ADDR}/v1/sys/health; do
    echo "Waiting for vault..."
    sleep 2
done
echo "Vault is ready."

# Enable KV v2 secrets engine if not already enabled at secret/
vault secrets list | grep -q '^secret/' || vault secrets enable -path=secret kv-v2

# Write test development secrets
echo "Writing secrets..."

# PostgreSQL credentials
vault kv put secret/dev/pagila \
    host="postgres.internal" \
    user="postgres" \
    password="mysecretpassword" \
    database="pagila"

# GCP service account key
vault kv put secret/dev/bigquery \
    type="service_account" \
    project_id="may-analytics-dev" \
    private_key_id="dummy-key-id" \
    private_key="-----BEGIN PRIVATE KEY-----\ndummy\n-----END PRIVATE KEY-----\n" \
    client_email="dev-sa@may-analytics-dev.iam.gserviceaccount.com"

# Snowflake credentials
vault kv put secret/dev/snowflake \
    account="xyz12345.us-east-1" \
    user="may_service" \
    password="my-snowflake-password" \
    warehouse="COMPUTE_WH" \
    database="ANALYTICS" \
    role="SYSADMIN"

echo "Vault seed completed successfully."
