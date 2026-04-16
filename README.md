# neutrino-notes

Collaborative note taking microservice for the Neutrino platform.

## Responsibilities

- Rich text document creation 
- Real-time collaborative editing via WebSockets
- Content stored as versioned files in neutrino-drive

## MIME type

`application/x-neutrino-notes`

## API

Swagger UI: `http://localhost:8080/swagger-ui/`

## Running locally

```bash
cp .env.example .env
cargo run
```

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `JWT_SECRET` | — | **Required.** Must match auth service |
| `AUTH_URL` | — | **Required.** Base URL of neutrino-auth |
| `NOTES_DATABASE_URL` | `./docs.db` | SQLite database path |
| `NOTES_PORT` | `8080` | HTTP listen port |
| `LOG_LEVEL` | `info` | Tracing level |
