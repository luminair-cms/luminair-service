# Release Management and Docker Publishing

This document describes the workflow for creating releases, publishing Docker images to GitHub Packages (GHCR), and deploying/running the containers.

---

## 1. Branching Strategy & Workflow Triggers

The CI/CD pipeline is automated via GitHub Actions in [.github/workflows/docker-publish.yml](file:///Users/dmitri.astafiev/luminair/luminair-service/.github/workflows/docker-publish.yml).

- **On Pull Request to `main`**: Runs tests and Clippy lints to verify stability.
- **On Push to `main`**: Runs tests and Clippy. If successful, builds and pushes both the `luminair-service` and `migration-cli` Docker images tagged as `latest` and `sha-<commit_sha>`.
- **On Push of a Version Tag (e.g. `v1.0.0`)**: Runs tests and Clippy. If successful, builds and pushes both images tagged with the semantic version, and automatically creates a new GitHub Release with generated release notes.

---

## 2. How to Create and Publish a Release

To create a new release, follow these steps from your terminal:

### Step 1: Ensure your local branch is clean and up-to-date
```bash
git checkout main
git pull origin main
```

### Step 2: Create a Git Tag
Create a semantic version tag (following `vX.Y.Z` or `X.Y.Z` format):
```bash
git tag v1.0.0
```

### Step 3: Push the Tag to GitHub
```bash
git push origin v1.0.0
```

Once pushed, GitHub Actions will:
1. Validate formatting, lints, and run tests.
2. Build Docker images for both `service` and `migration` targets.
3. Publish them to GHCR.
4. Create a draft/published GitHub Release in your repository.

---

## 3. Pulling and Running Docker Images

Images are published to the GitHub Container Registry (`ghcr.io`).

### Service Image
- Name: `ghcr.io/<owner>/luminair-service`
- Purpose: Serves the Schema-Driven CMS API.

### Migration Image
- Name: `ghcr.io/<owner>/luminair-migration`
- Purpose: Runs database migrations.

### How to Run Locally with Docker

#### Running the Database Migrations
To run migrations, spin up the migration container passing your database connection details:
```bash
docker run --rm \
  -e APP_DATABASE_CONNECTION_HOST="host.docker.internal" \
  -e APP_DATABASE_CONNECTION_PORT="5432" \
  -e APP_DATABASE_CONNECTION_DATABASE="luminair" \
  -e APP_DATABASE_CONNECTION_USERNAME="postgres" \
  -e APP_DATABASE_CONNECTION_PASSWORD="password" \
  ghcr.io/<owner>/luminair-migration:latest
```

#### Running the Service
```bash
docker run -d \
  -p 8080:8080 \
  -e APP_DATABASE_CONNECTION_HOST="host.docker.internal" \
  -e APP_DATABASE_CONNECTION_PORT="5432" \
  -e APP_DATABASE_CONNECTION_DATABASE="luminair" \
  -e APP_DATABASE_CONNECTION_USERNAME="postgres" \
  -e APP_DATABASE_CONNECTION_PASSWORD="password" \
  --name luminair-service \
  ghcr.io/<owner>/luminair-service:latest
```

---

## 4. Multi-Cloud Deployment Compatibility

The Docker images are compiled for Linux (`linux/amd64`) using `debian:bookworm-slim` for maximum compatibility and minimal overhead. They can be deployed to any container orchestration service:
- **AWS ECS (Fargate)** or **EKS**
- **Azure Container Apps**
- **Google Cloud Run**
- **DigitalOcean App Platform**

Configurations are loaded from `/app/config/default.yaml` and overridden via environment variables prefixed with `APP_` (e.g., `APP_SERVER_PORT`, `APP_DATABASE_CONNECTION_HOST`).
