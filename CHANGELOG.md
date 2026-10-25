# Changelog

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el
versionado, [SemVer](https://semver.org/lang/es/).

## [0.4.0](https://github.com/atareao/errserve/compare/v0.3.0..v0.4.0) - 2026-10-25

### Añadido

- **embedded-error-pages**: El binario lleva **dentro** las once páginas de error y su hoja
  `css/error.css`, así que un `cargo install errserve` —un ejecutable y ningún fichero de datos—
  sirve las páginas de verdad en vez del respaldo de texto: `GET /404.html` pasa de `text/plain` a
  `text/html`, y `GET /css/error.css` de `404` a `200 text/css`.

### Cambiado

- **`CONTENT_DIR` pasa a ser un *override*** de esos recursos embebidos: misma variable, mismo
  defecto `content`, y sigue siendo la única ruta de estáticos. Un fichero que se pueda leer sigue
  ganando —también para la hoja—, y la cascada del directorio se agota entera antes de entrar en la
  copia embebida. **La imagen no cambia para quien la despliegue**: el `Dockerfile` copia `content/`
  —en el stage de compilación, porque el binario la embebe, y también en el de ejecución— y la capa del
  disco gana, así que sirve los mismos bytes que en `0.3.0` y no hay que hacer nada.

### Documentación

- Corrige el estado medido que quedó viejo tras el renombrado
- **embedded-error-pages**: El contrato embebido en el README y en AGENTS

### Pruebas

- **embedded-error-pages**: RED, la página embebida sin ficheros de datos
- **embedded-error-pages**: RED, la paridad de las dos copias

### Mantenimiento

- **openspec**: Propone embeber las páginas en el binario
- **ci**: El CI construye la imagen en cada propuesta de cambio —la puerta del empaquetado, que
  faltaba—: un `Dockerfile` que deje de compilar se ve en el PR, y no al publicar la etiqueta.

## [0.3.0](https://github.com/atareao/errserve/compare/v0.2.0..v0.3.0) - 2026-10-24

### Cambiado

- Renombra el proyecto y el crate a `errserve`. El nombre `errors` estaba ocupado en crates.io
  (`seanmonstar`, desde 2019), así que el crate no se podía publicar con él.
- **La imagen cambia de paquete**: el paquete de GHCR no se renombra ni se redirige, así que
  `ghcr.io/atareao/errors` queda congelado en `0.2.0` —sigue descargable— y a partir de esta
  versión la imagen es `ghcr.io/atareao/errserve`. Hay que cambiar la referencia al desplegar.

## [0.2.0](https://github.com/atareao/errserve/compare/v0.1.0..v0.2.0) - 2026-10-24

### Añadido

- Compresión, correlación, métricas, LOG_FORMAT y entrega
- **release**: Publica la imagen en GHCR y crea el Release
- **metrics**: Distingue el origen del código en el contador
- **config**: El valor de Retry-After se configura con RETRY_AFTER

### Corregido

- **error-page**: Ningún estado de error del servidor de ficheros se escapa

### Documentación

- **traefik**: Ejemplo ejecutable y compose de producción
- **release**: La visibilidad del paquete, según lo medido
- La regla nueva del estado y el contador de dos etiquetas
- RETRY_AFTER y el registro de peticiones

### Mantenimiento

- Audita las dependencias y automatiza el CHANGELOG
- **compose**: Endurece el contenedor
- **openspec**: Archiva el change security-hardening
- **openspec**: Archiva el change retry-after-configurable

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
