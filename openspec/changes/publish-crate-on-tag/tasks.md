# Tasks: publish-crate-on-tag

> Esto es **entrega**, no código: la «prueba» de esta tabla es un **comando**, no un `#[test]`. Pero,
> a diferencia de `fix-image-build`, aquí **sí hay lógica que se puede ejecutar en local**: la
> comprobación de la versión contra la etiqueta es un comando, y se somete a casos que **deben** fallar
> y casos que **deben** pasar antes de cablearla. La publicación de verdad se verifica contra
> crates.io, que es el único sitio donde se puede medir.

## 0. Prerrequisito

- [x] 0.1 Medir el punto de partida y dejarlo escrito: crates.io en `0.3.0`
      (`max_version = 0.3.0`, `newest_version = 0.3.0`, `versiones = ['0.3.0']`) y el `.crate` de
      `0.4.0` inexistente (la descarga responde `302` a un fichero que no está). Sin esta medida, la
      verificación del §3 no se puede comparar contra nada. Medido.
- [x] 0.2 Secreto `CARGO_REGISTRY_TOKEN` en el repositorio, con un token de crates.io con permiso de
      publicación. El token **no** se escribe en ningún fichero: vive en los secretos de Actions.
      Medido: ya existía (`gh secret list`, alta el `2026-10-25T06:46:46Z`), así que no hubo que
      crearlo en esta sesión.

## 1. RED (el comando que falla primero)

- [ ] 1.1 **Antes** de cablear nada, ejecutar la comprobación de versión contra etiqueta con valores
      **desajustados** —`Cargo.toml` en `0.4.0` y etiqueta `v0.4.1`— en local: tiene que **fallar**
      nombrando el desajuste. Es la RED del requisito de la versión: el comando se ve en rojo antes de
      existir en el workflow.
- [ ] 1.2 Ejecutar la misma comprobación con valores **iguales** —`Cargo.toml` y etiqueta en `0.4.0`—
      en local: tiene que **pasar**. Es la GREEN mínima de la comprobación, y la pareja de 1.1
      demuestra que el comando discrimina en los dos sentidos.
- [ ] 1.3 `cargo publish --dry-run --locked` en local, para dejar constancia de que el paquete es
      publicable —o del error que salga—. Empaqueta, compila y valida los metadatos sin subir nada; es
      la comprobación que el CI pasará a hacer sola.

## 2. GREEN (la implementación mínima)

- [ ] 2.1 `release.yml`: un job `crate`, con `needs: imagen`, que resuelve la versión, **comprueba que
      coincide con la de `Cargo.toml`** (falla nombrando el desajuste si no), **consulta crates.io con
      `User-Agent`** y, si la versión ya está publicada, avisa y **continúa** (idempotencia), y si no,
      publica con `cargo publish --locked` usando `CARGO_REGISTRY_TOKEN` desde `secrets`.
- [ ] 2.2 `ci.yml`: un paso nuevo en el job `puertas` —con el toolchain y la caché que ya tiene— que
      corre `cargo publish --dry-run --locked`. No necesita un check requerido nuevo: va dentro de
      `puertas`, que ya lo es.
- [ ] 2.3 El `dry-run` en **verde** en el CI del PR, con el árbol actual: el paquete que lleva
      `content/` dentro es publicable.

## 3. Verificación

- [ ] 3.1 La publicación de verdad, con el `workflow_dispatch` que ya existe sobre la etiqueta
      `v0.4.0`: el job `crate` corre, y **el crate aparece en crates.io**. Antes, la imagen se
      republica **sin daño** —misma `0.4.0` y `latest`— y el Release **se reutiliza** (no se duplica).
- [ ] 3.2 La idempotencia, medida: re-ejecutar el mismo `workflow_dispatch` sobre `v0.4.0` termina
      **en verde**, con el paso de publicación detectando que la versión ya está y continuando sin
      error.

## 4. La verificación que cierra el motivo del change

- [ ] 4.1 **Desde un directorio vacío**, con el binario **instalado con `cargo install`**, medir lo que
      motivó el change: `GET /404` con `Accept: text/html` responde `404` con `text/html` —la página
      de verdad, no el respaldo de texto— y `GET /css/error.css` responde `200 text/css`. Es
      literalmente el defecto que abrió `embedded-error-pages`, y la primera vez que se puede medir de
      punta a punta: hasta hoy, quien instalaba con cargo seguía recibiendo `text/plain` y un `404`.

## 5. Documentación y cierre

- [ ] 5.1 `AGENTS.md`: la sección de entrega —el tag publica imagen **y** crate, y el secreto
      `CARGO_REGISTRY_TOKEN` que hace falta— y, si procede, un quirk sobre la idempotencia de
      republicar una versión ya publicada.
- [ ] 5.2 `CHANGELOG.md`: un bullet de Mantenimiento en la `0.4.0` —que no está publicada en
      crates.io y por tanto se puede corregir sin reescribir historia publicada—.
- [ ] 5.3 `openspec validate publish-crate-on-tag --strict` limpio.
- [ ] 5.4 `openspec archive publish-crate-on-tag`, para fusionar la delta en
      `openspec/specs/http-server/`.
