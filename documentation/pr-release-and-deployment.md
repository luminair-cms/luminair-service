# Pull Requests, Releases, and Deployment Guide

This document outlines the development workflow, pull request guidelines, version releasing process, and containerized deployment workflow for the Luminair backend services.

---

## 1. Pull Request Guidelines & Pre-Push Checks

Before pushing any changes to GitHub or opening a pull request, you **must** ensure your code conforms to the project's standards. Run the following validation commands in order:

### Code Formatting (Mandatory)
All Rust code must be formatted using the workspace settings. Execute this command before pushing:
```bash
cargo fmt --all
```
*Note: The CI pipeline will automatically reject any pull requests containing unformatted code.*

### Static Analysis & Lints
Verify that your changes do not introduce new compiler warnings or common anti-patterns:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

### Build Check
Ensure that the entire workspace compiles cleanly:
```bash
cargo check --workspace
```

### Running Tests
Execute the unit and integration tests to prevent regressions:
```bash
cargo test
```

---

## 2. Branching Strategy & Workflow Triggers

The CI/CD pipeline is managed via GitHub Actions in [.github/workflows/docker-publish.yml](file:///Users/dmitri.astafiev/luminair/luminair-service/.github/workflows/docker-publish.yml).

- **Pull Requests to `main` (or pushes)**: Triggers validation checks (`cargo fmt --check`, Clippy lints, and the test suite). It does *not* build or publish Docker images.
- **Pushes of Version Tags (e.g. `v1.0.0`)**: Triggers full validation. Upon success, it builds and pushes two Docker images (`luminair-service` and `luminair-migration`) tagged with the version and `latest`, and automatically creates a new GitHub Release with release notes.

---

## 3. How to Create and Publish a Release

To create a new release, follow these steps from your terminal:

### Step 1: Ensure your local branch is clean and up-to-date
```bash
git checkout main
git pull origin main
```

### Step 2: Create a Git Tag
Create a semantic version tag:
```bash
git tag v1.0.0
```

### Step 3: Push the Tag to GitHub
```bash
git push origin v1.0.0
```

Once pushed, the automated pipeline will:
1. Validate code formatting, lints, and test suites.
2. Build Docker images for both `service` and `migration` targets.
3. Publish the images to the GitHub Container Registry (GHCR).
4. Generate a new GitHub Release draft/publication in the repository.

---

## 4. Pulling and Running Docker Images

Images are published to the GitHub Container Registry (`ghcr.io`).

### Service Image
- Name: `ghcr.io/<owner>/luminair-service`
- Purpose: Serves the Schema-Driven CMS API.

### Migration Image
- Name: `ghcr.io/<owner>/luminair-migration`
- Purpose: Runs database schema migrations.

### How to Run Locally with Docker

#### Running Database Migrations
To run migrations against your target database, run:
```bash
docker run --rm \
  -e APP_DATABASE_CONNECTION_HOST="host.docker.internal" \
  -e APP_DATABASE_CONNECTION_PORT="5432" \
  -e APP_DATABASE_CONNECTION_DATABASE="luminair" \
  -e APP_DATABASE_CONNECTION_USERNAME="postgres" \
  -e APP_DATABASE_CONNECTION_PASSWORD="password" \
  ghcr.io/<owner>/luminair-migration:latest
```

#### Running the CMS Service
To spin up the service, run:
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

## 5. Multi-Cloud Deployment Configuration

The Docker images are compiled for Linux (`linux/amd64`) using `debian:bookworm-slim` for minimal footprint. They can be deployed to any container orchestration service:
- **AWS ECS (Fargate)** or **EKS**
- **Azure Container Apps**
- **Google Cloud Run**
- **DigitalOcean App Platform**

Configurations are loaded from `/app/config/default.yaml` and can be overridden via environment variables prefixed with `APP_` (e.g., `APP_SERVER_PORT`, `APP_DATABASE_CONNECTION_HOST`).
