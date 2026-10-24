# Empaquetado de `errors`. `just --list` para ver las recetas.
#
# El motor de contenedores está parametrizado: esta máquina no tiene `docker`
# (sí `podman`), así que no se hardcodea el binario.
engine  := "docker"
user    := "atareao"
name    := `basename ${PWD}`

# `version` = último tag de git. Ojo: `git tag -l | tail -n1` sale con éxito y
# con salida **vacía** cuando el repo todavía no tiene tags (es el caso ahora),
# y `atareao/errors:` no es un tag válido. El `grep .` intermedio es el que
# fuerza el fallo con entrada vacía para que se aplique el respaldo `latest`.
# Decisión: degradar a `latest` en vez de petar, para que `just build` funcione
# en un clon recién hecho sin tags.
version := `git tag -l | tail -n1 | grep . || echo latest`

# Puerto del host. El contenedor publica 8080 y el servicio escucha en 8080
# (`PORT`, por defecto 8080 en `src/config.rs`): mismo número a los dos lados,
# sin el desajuste 8080:3000 que tenía `simpleserver`.
port    := "8080"

# `podman`/`buildah` construyen en formato OCI por defecto y en OCI **no existe
# el HEALTHCHECK**: lo descartan con un aviso por stderr y la imagen sale sin
# healthcheck (verificado). Como el Dockerfile sí lo define, se fuerza el
# formato `docker` cuando el motor es podman. `docker build` no acepta ese flag,
# así que va vacío en ese caso.
build_format := if engine == "podman" { "--format docker" } else { "" }

# Lista las recetas.
default:
    @just --list

# Imagen, etiquetada con la versión y con `latest`.
build:
    echo "construyendo {{user}}/{{name}}:{{version}}"
    {{engine}} build {{build_format}} -t {{user}}/{{name}}:{{version}} \
                                   -t {{user}}/{{name}}:latest \
                                   .

# Publica la versión y `latest`.
push:
    {{engine}} push {{user}}/{{name}}:{{version}}
    {{engine}} push {{user}}/{{name}}:latest

# Contenedor de servicio en 8080, desacoplado de la terminal.
run:
    {{engine}} run --rm \
                   --init \
                   --detach \
                   --publish {{port}}:{{port}} \
                   --name {{name}} \
                   {{user}}/{{name}}:latest

# Servidor en el host, sin Docker, sobre la misma `content/`.
dev:
    # Mismas variables por defecto que la imagen. `CONTENT_DIR=content` se
    # resuelve desde el directorio del repo, que es donde vive `content/`.
    CONTENT_DIR=content PORT={{port}} cargo run
