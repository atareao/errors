# Changelog

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el
versionado, [SemVer](https://semver.org/lang/es/).

## [0.1.0] - 2026-10-24

Primera versión.

### Añadido

- Servidor de páginas de error para Traefik que devuelve el **código de error real**,
  a diferencia de `error-pages`, que responde siempre `200`.
- Resolución del código en cascada: path (`/404`, `/404.json`) → cabecera `X-Code` →
  `DEFAULT_ERROR_PAGE`, restringida al rango `400..=599`.
- Negociación de formato en cascada: extensión → `Content-Type` → `X-Format` → `Accept`
  → texto plano. Cuerpos HTML (desde `content/`), JSON, XML y texto.
- Once códigos con página propia (`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`,
  `502`, `503`, `504`), con una hoja compartida `css/error.css` y la clase `c<código>`
  en el `body`.
- Cabeceras de acompañamiento: `X-Robots-Tag` en toda página y `Retry-After: 120` en
  `429` y `503`.
- Compresión gzip cuando el cliente la pide, con `Vary: Accept-Encoding`.
- Copia de las cabeceras de correlación (`X-Request-Id`, `X-Trace-Id`,
  `X-Correlation-Id`, `X-Amzn-Trace-Id`) de la petición a la respuesta.
- `GET /metrics` en texto Prometheus, con el contador de páginas servidas por código.
- `LOG_FORMAT` para elegir entre registro en texto (defecto) y en JSON.
- Empaquetado con `Dockerfile` multi-stage, `.dockerignore` y `.justfile`.

### Seguridad

- El path se normaliza antes de clasificarse, de modo que una página de error no pueda
  servirse como si fuera un asset con un `200`.
