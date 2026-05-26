# May Semantic Layer - Local Deployment

This Docker Compose stack provides a local environment for the May Semantic Layer.

## Quick Start

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Open `.env` and set a secure password for `IDENTITY_DB_PASSWORD`. Note that the stack will fail to start if this is not provided.

3. Start the stack:
   ```bash
   docker compose up -d
   ```

This will build the required components locally without mounting your entire source tree.
