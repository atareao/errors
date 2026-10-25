# Tasks: fix-image-build

> Esto es **empaquetado**, no código: la «prueba» de esta tabla es un **comando**, no un `#[test]`.
> No hay prueba de Rust que pueda fijar que la imagen se construye —un test no lanza un build de
> Docker—, así que la fase RED es **el propio build fallando**, ya reproducido, y la fase GREEN es ese
> mismo comando en verde.

## 0. Prerrequisito

- [x] 0.1 Reproducir el fallo, en local y sin caché previa: `podman build --target builder .` termina
      con `exit status 101` y los **doce** errores `couldn't read 'src/../content/<código>.html'` (el
      último, `content/css/error.css`). Es el fallo medido al etiquetar la `0.4.0`.
- [x] 0.2 Dejar constancia de que **nada se publicó** con el fallo: GHCR sigue en
      `['0.3.0', 'latest']` —no aparece `0.4.0`—, no hay Release `v0.4.0` y crates.io sigue en `0.3.0`.

## 1. RED (el comando que falla primero)

- [x] 1.1 El build del stage `builder` con el `Dockerfile` de hoy **falla**, con esa salida: es la RED
      de este change, y su forma de fallar es el `exit status 101`, no una aserción.
- [x] 1.2 **Antes** de tocar el `Dockerfile`, añadir a `.github/workflows/ci.yml` el job `imagen` que
      construye la imagen (`docker build` con el docker del runner, sin actions de terceros). En el PR
      del arreglo ese job tiene que salir **en rojo**: es la puerta nueva fallando por el mismo motivo
      que el release.

## 2. GREEN (la implementación mínima)

- [x] 2.1 `Dockerfile`, stage `builder`: añadir `COPY content ./content` antes de `COPY src ./src`, con
      su comentario que diga por qué —las macros leen al compilar—. El stage `runtime` **no** se toca.
- [x] 2.2 El build completo en verde: `podman build --target builder .` y luego
      `podman build -t errserve:local .`, ambos sin error y sin caché previa.
- [x] 2.3 Smoke test local con podman: arrancar el contenedor y comprobar, contra él,
      `GET /404` → `404` (con cuerpo HTML, no el respaldo de texto), `GET /css/error.css` →
      `200 text/css`, `GET /healthz` → `200`, y que el proceso corre como `10001:10001`
      (`podman exec … id -u`/`id -g`, o `podman inspect` de la imagen).

## 3. Verificación

- [x] 3.1 El job `imagen` del `ci.yml` en **verde** en el PR, con el `COPY` aplicado.
- [x] 3.2 Marcar el check nuevo (`imagen`) como **requerido** en `main` y en `development`, junto a
      `fmt, clippy y tests` y `cargo-deny`.
- [x] 3.3 La verificación de verdad, la publicación: mover la etiqueta `v0.4.0` al commit del arreglo
      y comprobar que el workflow de release publica **la imagen en GHCR con `0.4.0` y `latest`**, que
      el **Release** `v0.4.0` se crea, y que el `HEALTHCHECK` de la imagen queda `healthy`.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: corregir el quirk de la imagen —la copia en el `builder` es **obligatoria**,
      no una redundancia elegida— y el quirk de `include_str!`, que hoy describen un `Dockerfile` que
      no compilaba.
- [x] 4.2 `CHANGELOG.md`: corregir la entrada «Cambiado» de la `0.4.0` —la que dice «la imagen no
      cambia»—, que la `0.4.0` todavía **no está publicada** (no hay imagen ni Release).

## 5. Cierre

- [x] 5.1 `openspec validate fix-image-build --strict` limpio.
- [x] 5.2 `openspec archive fix-image-build`, para fusionar la delta en `openspec/specs/http-server/`.
- [x] 5.3 Mover la etiqueta `v0.4.0` al commit del arreglo (D4), que es lo que cierra la publicación.
