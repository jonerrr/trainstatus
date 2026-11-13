# Frontend

## Structure

- Trips, routes, and stops, each have a `Button` and `Modal` component.

## Config

| Environment Variable                     | Usage                                                                                                                                             | Required | Default                 |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ----------------------- |
| `__VITE_ADDITIONAL_SERVER_ALLOWED_HOSTS` | Comma-separated extra hosts allowed by Vite.                                                                                                      | No       | None                    |
| `API_ORIGIN`                             | Internal backend origin for server-side `/api` requests, e.g. `http://trainstatus-backend:3055`. Read at runtime; browser requests stay relative. | No       | Incoming request origin |

Behind a trusted reverse proxy, set `HOST_HEADER=x-forwarded-host` and
`PROTOCOL_HEADER=x-forwarded-proto` so adapter-node sees the public request URL.
