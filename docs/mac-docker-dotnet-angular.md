# Mac Development Environment for .NET, Angular, and SQL Server Using Docker

This guide describes how to set up a productive local development workflow on a MacBook Pro for a solution that uses:

- Backend services in C# targeting .NET (Core/.NET 6+) 
- An Angular front-end
- SQL Server for data persistence
- Docker (and Docker Compose) to orchestrate local dependencies and enable seamless collaboration across team members

The steps assume macOS 12+ (Monterey or newer). Note notes are provided for both Intel and Apple Silicon (M1/M2/M3) machines.

---

## 1. Hardware & OS Prerequisites

1. **macOS Version**: Ensure macOS 12+ to receive the latest Docker Desktop and virtualization updates.
2. **Virtualization Support**: Enable virtualization in macOS:
   - For Apple Silicon: virtualization is always on. Install Rosetta 2 using `softwareupdate --install-rosetta` if prompted to support x64 images.
   - For Intel: ensure "Intel VT-x" is enabled in firmware (usually enabled by default on Mac hardware).
3. **System Resources**: Allocate at least 16 GB RAM and 50 GB free disk space for Docker images, containers, and project assets.

---

## 2. Core Tooling Installation

1. **Homebrew (package manager)**
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```
   After installation, add Homebrew to your shell profile if not auto-configured.

2. **Command Line Tools**
   ```bash
   xcode-select --install
   ```

3. **Docker Desktop for Mac**
   - Download from [https://www.docker.com/products/docker-desktop/](https://www.docker.com/products/docker-desktop/).
   - Install and sign in with a Docker Hub account (optional but helpful for pulling images).
   - Configure resource limits via *Settings → Resources*: allocate 6–8 CPUs and 8–12 GB RAM for smooth SQL Server execution.

4. **.NET SDK**
   - Install via Homebrew for multi-version support:
     ```bash
     brew install --cask dotnet-sdk
     ```
   - Optionally manage multiple SDKs with `asdf` or `dotnet-install` scripts.

5. **Node.js and Angular CLI**
   - Install Node.js LTS (v18+ recommended):
     ```bash
     brew install node@18
     echo 'export PATH="/opt/homebrew/opt/node@18/bin:$PATH"' >> ~/.zshrc
     ```
   - Install Angular CLI globally:
     ```bash
     npm install -g @angular/cli
     ```

6. **Database Tools**
   - Azure Data Studio (`brew install --cask azure-data-studio`) or DBeaver for GUI management.
   - Install `sqlcmd` for CLI interactions:
     ```bash
     brew install --cask ms-azurestorageexplorer # installs MSSQL tooling bundle
     ```
     or use `mssql-tools` via the official Microsoft tap.

7. **Additional Utilities**
   - `make`, `jq`, `gnu-sed`, `watchman` for build scripts (`brew install make jq gnu-sed watchman`).
   - `direnv` or `dotenv-linter` for environment variable management.

---

## 3. Project Structure & Docker Compose

A typical repository layout for combined .NET API, Angular UI, and SQL Server might look like:

```
/your-project
├── api/                # .NET Web API project(s)
├── web/                # Angular workspace
├── db/                 # SQL migration scripts, seed data
├── docker/             # Dockerfile templates, compose overrides
├── docker-compose.yml
└── .env
```

### Sample `docker-compose.yml`

```yaml
version: "3.9"
services:
  api:
    build:
      context: ./api
      dockerfile: Dockerfile
    env_file:
      - .env
    environment:
      ASPNETCORE_ENVIRONMENT: Development
      ASPNETCORE_URLS: http://+:5000
    ports:
      - "5000:5000"
    depends_on:
      - db
    volumes:
      - ./api:/app
      - ~/.nuget/packages:/root/.nuget/packages:ro

  web:
    build:
      context: ./web
      dockerfile: Dockerfile
    env_file:
      - .env
    command: npm run start
    ports:
      - "4200:4200"
    volumes:
      - ./web:/usr/src/app
      - /usr/src/app/node_modules
    depends_on:
      - api

  db:
    image: mcr.microsoft.com/azure-sql-edge:latest # For Apple Silicon
    container_name: local-sql
    ports:
      - "1433:1433"
    environment:
      ACCEPT_EULA: "Y"
      MSSQL_SA_PASSWORD: "YourStrong!Passw0rd"
    volumes:
      - sqlvolume:/var/opt/mssql

volumes:
  sqlvolume:
```

> **Apple Silicon Notes**: Use `azure-sql-edge` (ARM64 compatible) or `mssql/server:2022-latest` with `platform: linux/amd64` to run the x64 SQL Server image under emulation. Set `DOCKER_DEFAULT_PLATFORM=linux/amd64` in `.env` if targeting x64.

### Environment Variables

Store secrets in `.env` (ignored by Git) and reference via `env_file` in Docker Compose. Example:

```
ASPNETCORE_ENVIRONMENT=Development
ConnectionStrings__Default=Server=db;Database=AppDb;User Id=sa;Password=YourStrong!Passw0rd;
Angular__ApiBaseUrl=http://localhost:5000
```

---

## 4. Backend (.NET) Development Workflow

1. **Project Scaffolding**
   ```bash
   dotnet new webapi -o api
   dotnet new classlib -o api/YourProject.Domain
   ```
2. **Local Development**
   - Use `dotnet watch` locally when developing outside Docker for faster feedback.
   - In-container development: mount the project folder as a volume (see compose) and use `dotnet watch` inside the container.
3. **Entity Framework Core Tooling**
   - Install dotnet tool: `dotnet tool install --global dotnet-ef`
   - Manage migrations targeting the containerized SQL Server:
     ```bash
     DOTNET_ENVIRONMENT=Development dotnet ef migrations add Init --project api/YourProject.Infrastructure
     DOTNET_ENVIRONMENT=Development dotnet ef database update
     ```
   - Configure EF connection strings to point at `Server=db;` (Docker network alias).
4. **Testing**
   - Run unit tests via `dotnet test`.
   - For integration tests that rely on SQL Server, use a separate compose service or ephemeral container started via `docker compose run`.

---

## 5. Front-End (Angular) Workflow

1. **Install Dependencies**
   ```bash
   cd web
   npm install
   ```
2. **Local Dev (outside Docker)**
   - Run `ng serve --proxy-config proxy.conf.json` to proxy API calls to `http://localhost:5000`.
3. **In-Docker Dev**
   - Ensure volume mounts allow hot reloading (`./web:/usr/src/app`).
   - Use `CHOKIDAR_USEPOLLING=true` on Docker Desktop for reliable file watching.

4. **Testing & Linting**
   - `ng test` for unit tests.
   - `ng e2e` for end-to-end tests (configure Cypress/Playwright containers if needed).

---

## 6. SQL Server Container Management

1. **Start Database**
   ```bash
   docker compose up db -d
   ```
2. **Create Database / Run Migrations**
   - Connect with `sqlcmd`:
     ```bash
     sqlcmd -S localhost,1433 -U sa -P 'YourStrong!Passw0rd' -Q "SELECT @@VERSION"
     ```
   - Apply schema using EF migrations or run `.sql` files inside the container:
     ```bash
     docker exec -it local-sql /opt/mssql-tools/bin/sqlcmd -S localhost -U sa -P 'YourStrong!Passw0rd' -i /path/in/container/setup.sql
     ```
3. **Persisting Data**
   - The named volume `sqlvolume` preserves data between restarts. For seed/reset scenarios, use a dedicated script to drop and recreate the volume.

---

## 7. Developer Productivity Tips

- **Container Profiles**: Use multiple compose files (`docker-compose.dev.yml`, `docker-compose.test.yml`) to tailor services per scenario.
- **Hot Reload**: Combine `dotnet watch` and Angular's live reload with volume mounts for near-native feedback.
- **Debugging**: Configure VS Code launch settings:
  - For .NET: `launch.json` with `type: coreclr`, `request: attach`, `processId` from container or use `vsdbg`.
  - For Angular: `Chrome` or `Edge` debugging via `launch.json`.
- **Secrets Management**: Leverage `.env` files for development, but adopt Azure Key Vault/AWS Secrets in production.
- **CI/CD Alignment**: Mirror Docker Compose setup in CI pipelines to ensure parity with local dev.
- **Cross-platform**: Ensure scripts use `/bin/bash` and avoid GNU-only flags unless installed via Homebrew.

---

## 8. Bootstrapping Scripts

Create `Makefile` tasks to streamline commands:

```makefile
setup:
brew bundle --file=Brewfile
dotnet tool restore
npm install --prefix web

docker-up:
docker compose up --build

docker-down:
docker compose down --remove-orphans

test:
dotnet test
npm test --prefix web
```

Maintain a `Brewfile` capturing package dependencies for effortless onboarding.

---

## 9. Troubleshooting

| Issue | Resolution |
| --- | --- |
| SQL Server fails to start on Apple Silicon | Use `mcr.microsoft.com/azure-sql-edge` or set `platform: linux/amd64`. Ensure Rosetta installed and allocate >8 GB RAM. |
| File changes not detected in containers | Enable polling: set `CHOKIDAR_USEPOLLING=true` for Angular, `DOTNET_USE_POLLING_FILE_WATCHER=1` for .NET. |
| `dotnet restore` slow due to containerized nuget cache | Bind-mount host NuGet cache (`~/.nuget/packages`). |
| Port conflicts | Customize exposed ports in `docker-compose.yml` or stop conflicting services (`sudo lsof -i :5000`). |
| SSL cert trust issues | Export development cert with `dotnet dev-certs https --trust` and share into the container via volume. |

---

## 10. Next Steps

- Add GitHub Actions or Azure Pipelines to run tests using the same Docker Compose file.
- Create onboarding documentation referencing this guide.
- Regularly update Docker images and Homebrew formulas to stay current with security patches.

