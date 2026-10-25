###############################################################################
# errserve — servidor de páginas de error para Traefik
#
# Multi-stage:
#   * builder  -> `rust:alpine`, cuyo target nativo del toolchain **ya es**
#                 `x86_64-unknown-linux-musl`. Por eso aquí NO hay nada de la
#                 maquinaria de cross-compilación que arrastraba `simpleserver`:
#                 ni `rustup target add`, ni `--target`, ni `musl-tools`, ni
#                 `OPENSSL_LIB_DIR`, ni `cmake`/`pkg-config`. El proyecto no
#                 tiene ninguna dependencia con C (ni OpenSSL), así que el
#                 enlazado estático con musl sale "gratis" al elegir esta imagen.
#   * runtime  -> `alpine`, la libc contra la que se enlazó. El binario queda
#                 estático, de modo que builder y runtime no comparten ABI ni
#                 hay que igualar la versión de glibc.
###############################################################################

# Fijadas a una versión concreta para builds reproducibles. Basta con cambiarlas
# aquí (o con `--build-arg`) para subir de toolchain o de base.
ARG RUST_IMAGE=rust:1.99.1-alpine3.23
ARG RUNTIME_IMAGE=alpine:3.23.7

###############################################################################
## Stage: builder
###############################################################################
FROM ${RUST_IMAGE} AS builder

WORKDIR /app

# --- Capa de dependencias ----------------------------------------------------
# Los manifiestos cambian mucho menos que `src/`, así que se copian primero:
# mientras `Cargo.toml` y `Cargo.lock` no cambien, esta capa se reutiliza y el
# build final solo recompila el crate local en lugar de todo el árbol de deps.
# `Cargo.lock` se copia (y va versionado) porque esto es un binario: `--locked`
# garantiza que la imagen usa exactamente las versiones del lock, o falla.
# El `main.rs` de relleno existe solo porque `cargo` necesita un crate raíz para
# resolver el grafo; se borra antes de copiar el fuente real.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

# --- Crate real --------------------------------------------------------------
# `content/` es una **entrada de la compilación**, no un adorno del runtime: las
# macros `include_str!`/`include_bytes!` de `src/embedded.rs` leen esos ficheros
# **al compilar** y resuelven la ruta relativa al propio fuente
# (`src/../content/…`). Sin ellos aquí, `cargo build` del crate real muere con
# doce «couldn't read …: No such file or directory» (las once páginas y
# `content/css/error.css`) antes de que el stage de runtime exista. Va antes de
# `src/` porque cambia menos: mientras las páginas no cambien, esta capa y la de
# dependencias se reutilizan. El `COPY content` del stage de runtime sigue
# siendo necesario y **no se toca**: `CONTENT_DIR` es la única ruta de estáticos,
# y como la capa del disco gana a la copia embebida, la imagen sigue sirviendo
# los ficheros del operador.
COPY content ./content
COPY src ./src
# `COPY` puede preservar el mtime del origen; tocar la raíz del crate asegura
# que `cargo` invalida la huella del crate de relleno y recompila el real.
RUN touch src/main.rs \
    && cargo build --release --locked

###############################################################################
## Stage: runtime
###############################################################################
FROM ${RUNTIME_IMAGE} AS runtime

# Etiquetas OCI en el **último** stage: las de un stage intermedio no llegan a
# la imagen final (comprobado: puestas en `builder`, la imagen salía sin ellas).
LABEL org.opencontainers.image.title="errserve" \
      org.opencontainers.image.description="Servidor de páginas de error para Traefik que devuelve el código HTTP real" \
      org.opencontainers.image.authors="Lorenzo Carbonell <a.k.a. atareao>"

# Usuario y grupo de sistema sin privilegios, con uid/gid **fijos** (`-S` y
# `-g 10001 -u 10001`) para que el propietario de ficheros sea determinista en
# cualquier host. Sin shell de login y sin HOME: el contenedor solo ejecuta un
# binario.
RUN addgroup -S -g 10001 app \
    && adduser -S -u 10001 -G app -H -s /sbin/nologin app

WORKDIR /app

# El binario lee los estáticos de `CONTENT_DIR`, cuyo valor por defecto es la
# ruta **relativa** `content`; con `WORKDIR /app` eso resuelve a `/app/content`.
# Aquí se copia `content/` a `/app/content`: una única ruta, coherente entre
# código (`src/config.rs`), Dockerfile y `.dockerignore`. (El bug de
# `simpleserver` era declarar `static` en el código y copiar `content/`.)
COPY --from=builder --chown=10001:10001 /app/target/release/errserve ./errserve
COPY --chown=10001:10001 content ./content

# Valores por defecto documentados; coinciden con `src/config.rs`.
# `CONTENT_DIR` es relativo a `WORKDIR`, de ahí `/app/content`.
ENV PORT=8080 \
    LOG_LEVEL=info \
    CONTENT_DIR=content \
    DEFAULT_ERROR_PAGE=404

# Healthcheck con el `wget` de busybox, ya presente en la base: `curl` no lo
# está y no se instala un paquete solo para esto. `/healthz` responde siempre
# 200 y no pasa por la resolución de páginas de error. `${PORT}` se expande
# desde el `ENV`, así que el healthcheck sigue a un `PORT` sobreescrito.
HEALTHCHECK --interval=30s --timeout=3s --start-period=2s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/healthz"

# El puerto que asume la guía de Traefik.
EXPOSE 8080

USER 10001:10001

CMD ["/app/errserve"]
