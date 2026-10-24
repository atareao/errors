# Empaquetado de `errors`. `just --list` para ver las recetas.
#
# El motor de contenedores está parametrizado: esta máquina no tiene `docker`
# (sí `podman`), así que no se hardcodea el binario.
#
# La imagen se publica en **GHCR** (`ghcr.io/atareao/errors`), no en Docker Hub:
# es gratis para repositorios públicos y es donde la deja la etiqueta `v*`
# (`.github/workflows/release.yml`). Estas recetas son la vía manual, para
# construir en local o publicar a mano.
engine   := "docker"
registry := "ghcr.io"
user     := "atareao"
name     := `basename ${PWD}`

# Referencia completa de la imagen. GHCR exige el nombre en minúsculas, y aquí
# `user`/`name` ya lo están.
image := registry + "/" + user + "/" + name

# `version` = último tag de git. Ojo: `git tag -l | tail -n1` sale con éxito y
# con salida **vacía** cuando el repo todavía no tiene tags (es el caso ahora),
# y `ghcr.io/atareao/errors:` no es un tag válido. El `grep .` intermedio es el
# que fuerza el fallo con entrada vacía para que se aplique el respaldo `latest`.
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
    echo "construyendo {{image}}:{{version}}"
    {{engine}} build {{build_format}} -t {{image}}:{{version}} \
                                   -t {{image}}:latest \
                                   .

# Entra en GHCR. Hace falta una vez por máquina: `push` escribe en el registro
# y GHCR sí exige autenticación para escribir (para leer un paquete público, no).
#
#   * Con un PAT clásico que tenga el scope `write:packages`: se pasa como
#     contraseña cuando la receta lo pida.
#   * Con el token de `gh`: el que trae por defecto NO incluye los scopes de
#     paquetes (verificado con `gh auth status`: 'gist', 'read:org', 'repo'), así
#     que hay que añadírselo antes:
#         gh auth refresh -h github.com -s write:packages
#         gh auth token | {{engine}} login {{registry}} -u {{user}} --password-stdin
#
# El usuario es el propietario del paquete y el token hace de contraseña. Para
# podman es la misma receta: `just engine=podman login`.
login:
    {{engine}} login {{registry}} -u {{user}}

# Publica la versión y `latest`. Requiere haber entrado antes (`just login`).
# Ojo con mover `latest` desde una pre-release: el workflow solo lo empuja para
# versiones estables, así que `v0.2.0-rc1` no debe pisar `latest` a mano.
push:
    {{engine}} push {{image}}:{{version}}
    {{engine}} push {{image}}:latest

# Contenedor de servicio en 8080, desacoplado de la terminal.
run:
    {{engine}} run --rm \
                   --init \
                   --detach \
                   --publish {{port}}:{{port}} \
                   --name {{name}} \
                   {{image}}:latest

# Servidor en el host, sin Docker, sobre la misma `content/`.
dev:
    # Mismas variables por defecto que la imagen. `CONTENT_DIR=content` se
    # resuelve desde el directorio del repo, que es donde vive `content/`.
    CONTENT_DIR=content PORT={{port}} cargo run
