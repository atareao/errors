# Changelog

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el
versionado, [SemVer](https://semver.org/lang/es/).

## [0.8.0](https://github.com/atareao/errserve/compare/v0.7.0..v0.8.0) - 2026-10-26

**Ruptura**: un tema deja de ser una **hoja de estilos** y pasa a ser una **página**, con
el nombre del tema como **ruta del fichero** (`content/<nombre>.html`). Con la hoja se van su
marcador `<link rel="stylesheet" href="/css/error.css" />`, su sustitución al servir y el
asset público `/css/error.css`; las reglas de cada tema viven ahora en el `<style>` de su
propia página, y la cascada del cuerpo HTML queda en un solo paso.

### Añadido

- **El séptimo tema, `matrix`**: un documento propio —lluvia digital en un `<canvas>`, glitch
  del código, scanlines, un terminal con su prompt, una pastilla roja y otra azul, un minijuego
  de teclado y un aviso—, autocontenido y con la animación **apagable de verdad**: su script
  consulta `prefers-reduced-motion` con `matchMedia` y **escucha sus cambios**, así que para y
  reanuda la lluvia y el glitch **sin recargar** —deja de pedir fotogramas de verdad—. Es la
  prueba de que un tema no cabe en una hoja.
- **Las siete páginas van embebidas en el binario** con `include_str!`: un `cargo install
  errserve` —un ejecutable y ningún fichero de datos— sirve los **siete** diseños, y borrar una
  página es un **error de compilación**, no un respaldo silencioso.
- **`/default.html` y `/default.htm` son el alias del código por defecto**:
  se resuelven después de la normalización del path y responden el código por defecto
  (`DEFAULT_ERROR_PAGE`), **nunca `200`**.

### Cambiado

- **Un tema es una página y el nombre es la ruta**: la página de cada tema vive en
  la **raíz** de `CONTENT_DIR`, como `CONTENT_DIR/<nombre>.html`. La ruta se **deriva** del
  nombre, así que no hay tabla que pueda divergir de él.
- **El cuerpo HTML se resuelve en un solo paso**: la página del tema (la del operador si se
  puede leer, y si no su copia embebida). Se retira la capa por código
  —`CONTENT_DIR/<código>.html` y `CONTENT_DIR/<código por defecto>.html`—: con las siete
  páginas que entrega el repositorio, esa capa quedaría siempre sombreada.
  `DEFAULT_ERROR_PAGE` **conserva** su significado —el código que se sirve cuando ninguna
  fuente aporta uno utilizable— pero **ya no nombra ningún fichero**; el diseño por código se
  expresa con la clase `c{{code}}` del `body`, que no puede discrepar del estado.
- **`content/404.html` → `content/default.html`**: es la página del tema `default` y el punto
  de personalización del operador.
- **`THEME` y `X-Theme` enumeran ahora siete nombres** (`default`, `minimal`, `terminal`,
  `paper`, `amber`, `compact` y `matrix`), con la misma semántica: `THEME` es una lista
  cerrada que aborta el arranque si el valor no es uno de los siete, y un `X-Theme` conocido
  gana al configurado.
- **El HTML servido es la página del tema**, con su `<style>` **dentro**: el cuerpo servido
  deja de enlazar ninguna hoja.

### Eliminado

- **El asset `/css/error.css` deja de existir**: `GET /css/error.css` responde `404` como
  cualquier path inexistente. Con él se van `content/css/error.css`, `content/themes/*.css`
  y el respaldo de assets embebidos del servidor de ficheros, que se quedó sin tabla.
- El marcador `<link rel="stylesheet" href="/css/error.css" />` y su sustitución al servir
  (`autocontenido()`, `marcador()`, `MARCADOR_ETIQUETA`, `MARCADOR_HREF`), junto con la
  cascada por código (`read_html()`).

### Corregido

- **Un `HEAD` sobre una página de error ya no lleva cuerpo.** El middleware componía el
  cuerpo igual que para un `GET`; ahora responde el mismo estado y las mismas cabeceras
  —`Content-Type`, `X-Robots-Tag`, `Retry-After`— pero **sin cuerpo**, como `ServeDir` y como
  las rutas de axum. Lo destapó la prueba que repuso la cobertura de `HEAD`, que se había
  quedado sin una.

### Pruebas

- Las **tres guardas** se retargetizan de hojas a **páginas** y viven en el `#[cfg(test)]` de
  `src/theme.rs`: la de **red** escanea la página entera —incluido su `<script>`—; la de
  **movimiento** el CSS y el guion, con la neutralización **por familia** y su límite
  escrito; y la de **cobertura de marcado** solo las seis sobrias, con `matrix` fuera.
- **222 pruebas** en verde (eran 207).

### Documentación

- El `README`, `AGENTS.md`, el `Dockerfile` y el contrato con Traefik describen el
  modelo «el tema es una página»: siete nombres, la estructura plana `content/<nombre>.html`,
  el alias `/default.html` y el leak de las seis páginas, que se bloquea en el proxy.

### Actualización desde la 0.7.0

Si tienes un `content/404.html`, un `content/css/error.css` o un
`content/themes/*.css`, ya no se consultan: tu punto de personalización pasa a ser la
**página de tu tema** (`content/<tu tema>.html`), y lo que **pierdes** es la granularidad
por código —con `THEME=matrix` verás la página de `matrix`, no la tuya—. Precio ya
documentado: las páginas llevan su `<style>` (y `matrix` su `<script>`) **en línea**, así
que una `Content-Security-Policy` estricta puesta en el proxy las deja sin estilo.

## [0.7.0](https://github.com/atareao/errserve/compare/v0.6.0..v0.7.0) - 2026-10-25

### Cambiado

- **theme**: La guarda de movimiento exige neutralizar, no nombrar
- **theme**: Repasa las seis hojas y firma la página

### Documentación

- **openspec**: Propone theme-visual-review
- **theme**: La guarda endurecida, el marcado y el separador decorativo
- **openspec**: Archiva theme-visual-review
- **openspec**: Cierra la revisión a ojo de las seis páginas

### Pruebas

- **theme**: Las pruebas del contrato de movimiento, que hoy fallan

## [0.6.0](https://github.com/atareao/errserve/compare/v0.5.0..v0.6.0) - 2026-10-25

### Añadido

- **theme**: La guarda de las hojas mide el contrato que declara
- **error-page**: Una plantilla rellenada al servir, y cualquier código con su propio cuerpo

### Documentación

- **theme**: Los dos contratos, y la cabecera de los cinco temas dice la verdad
- **openspec**: Archiva theme-sheet-contracts
- **error-page**: La página única, y la prosa de las seis hojas dice la verdad
- **openspec**: Archiva error-page-template

## [0.5.0](https://github.com/atareao/errserve/compare/v0.4.0..v0.5.0) - 2026-10-25

### Añadido

- **release**: Publica el crate en crates.io al etiquetar
- **theme**: El HTML servido es autocontenido y el tema es configurable

### Corregido

- **release**: Cualifica el ref de los checkouts a refs/tags
- **traefik**: El respaldo no publica el endpoint de servicio /metrics

### Documentación

- **openspec**: Propone publicar el crate en el release
- **openspec**: Documenta el crate en el release y su guarda de versión
- **openspec**: Cierra las casillas medidas del guion
- **release**: El tag publica también el crate
- **openspec**: Archiva publish-crate-on-tag

### Mantenimiento

- **openspec**: Archiva el change del build de la imagen

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
- **release**: Un tag publica también el **crate en crates.io**, así que `cargo install errserve`
  deja de instalar la `0.3.0`. El workflow de release gana un job `crate` que publica con
  `cargo publish --locked` desde el **árbol de la etiqueta**, y un job `version` —el primero— que
  comprueba que la versión del `Cargo.toml` es la de la etiqueta **antes** de publicar nada.
  Publicar una versión ya publicada no falla (idempotencia, para republicar una etiqueta), y el CI
  gana la puerta del paquete —`cargo publish --dry-run` en el job `puertas`—, que empaqueta y
  compila el crate extraído en cada PR.
- **release**: Ningún valor del evento entra en un `run:` del workflow de release: la etiqueta y la
  versión entran por `env:` y su forma se valida antes de usarlas. La primera auditoría del cambio
  dio NO APTO porque `${{ }}` se sustituye **antes** de que bash parsee el script, así que el filtro
  llegaba tarde y un `v0.4.0-$(comando)` correría junto al paso que publica.

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
