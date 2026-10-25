# Proposal: publish-crate-on-tag

## Why

La `0.4.0` está publicada **a medias**: la imagen en GHCR —con `0.4.0` y `latest`— y el Release de
GitHub **sí** salieron, pero **el crate no**. Medido hoy, crates.io sigue en `0.3.0`
(`max_version = 0.3.0`, `versiones = ['0.3.0']`), y el `.crate` de `0.4.0` responde 263 bytes porque
no existe. No es que el release esté roto: es que **le falta la mitad**, y la mitad que falta es la
del crate. La causa tampoco es un error de ejecución: `release.yml` **nunca menciona
`cargo publish`** —verificado—, así que el crate no se ha publicado jamás por esta vía. El release
publica la imagen y crea el Release, y ahí se acaba.

La consecuencia es la que cierra en falso el motivo del change `embedded-error-pages`: **`cargo
install errserve` sigue instalando un binario que sirve `text/plain` sin estilo a `GET /404.html` y
un `404` a `GET /css/error.css`.** Todo ese change —las once páginas embebidas y su hoja dentro del
ejecutable, la paridad de las dos copias— existe para que quien instala con cargo sirva las páginas
de verdad, y a quien las instala con cargo **todavía no le ha llegado**: a quien despliega la imagen,
sí. El empaquetado que se hizo para ese fin está en el árbol, en `main`, pero no ha viajado al
registro.

Y hay un agravante de proceso, que es la razón de que esto sea un change y no un `git push` a mano:
**publicar en crates.io es un paso a mano, y es el paso irreversible.** Una versión publicada solo se
puede *yank* —ocultar—, nunca reemplazar ni borrar. El error más caro del proyecto es, además, el
único que hoy no tiene puerta: `ci.yml` tiene tres jobs —`fmt, clippy y tests`, `cargo-deny` e
`imagen`— y **ninguno comprueba que el paquete sea publicable**; y el único sitio donde el paquete se
tocaba era el momento de publicarlo, que es justo cuando ya no hay vuelta atrás.

## What Changes

1. **`release.yml` gana un job `crate`, con `needs: imagen`.** El crate SHALL publicarse **solo si**
   la imagen se ha construido, se ha probado y se ha publicado. Es deliberado: lo **irreversible**
   va al final, y solo cuando todo lo demás ha pasado. `cargo publish` no se puede deshacer; construir
   y probar la imagen, sí, así que el orden no es cosmético.
2. **La versión de `Cargo.toml` y la de la etiqueta SHALL coincidir.** Si no cuadran, la publicación
   **falla** en vez de publicar una versión que no es la que se está anunciando. La razón es que
   `cargo publish` publica **lo que diga `Cargo.toml`**, no lo que diga el tag: etiquetar `v0.4.1` con
   `Cargo.toml` en `0.4.0` publicaría la `0.4.0` —o fallaría por ya publicada— **en silencio**, que es
   el peor de los desenlaces posibles para un artefacto irreversible.
3. **Republicar NO SHALL fallar por una versión ya publicada.** El workflow está diseñado para
   re-ejecutarse —el `workflow_dispatch` con input `tag` sobre una etiqueta ya publicada; el paso del
   Release ya reutiliza el existente—, así que el paso **detecta** que la versión ya está en crates.io
   y **continúa** en verde, en vez de romper con un rojo falso.
4. **Una puerta nueva en el CI.** El job `puertas` de `ci.yml` verifica en cada propuesta de cambio
   que el paquete **es publicable** (`cargo publish --dry-run --locked`), que empaqueta, compila y
   valida los metadatos **sin subir nada**. Es la misma lección de `fix-image-build`: **el empaquetado
   no puede descubrirse por primera vez al publicar**. Y no es teórico: `content/` viajando dentro del
   `.crate` es lo único que hace que el binario instalado sirva las páginas de verdad, y hoy eso solo
   lo comprueba una medición a mano en una sesión.
5. **El secreto `CARGO_REGISTRY_TOKEN` va como secreto del repositorio.** El token **NO SHALL**
   escribirse en ningún fichero: se pasa al paso como variable de entorno y se consume en el momento
   de publicar.

## Capabilities

### New Capabilities

(ninguna: el crate ya existía y ya era publicable; lo que falta es que **el release lo publique** y
que el paquete tenga puerta)

### Modified Capabilities

- `http-server`: **un requisito nuevo** de entrega —que empujar un tag publique la imagen **y** el
  crate, con la versión comprobada contra la etiqueta y sin fallar al republicar—, con sus escenarios.
  El requisito describe un hecho de la entrega, no del servicio en ejecución: nada de lo que hoy sirve
  el binario cambia.

## Non-goals

- **No se toca la imagen.** Ni el `Dockerfile`, ni el smoke test con docker real, ni la lógica del
  stage `runtime`: el job `imagen` se queda exactamente como está. Este change **añade** un job, no
  reorganiza el que ya publica.
- **No se toca el código ni las pruebas.** El crate ya es publicable y ya sirve las páginas embebidas;
  lo que falta es el pegamento de la entrega. Ni una línea de Rust cambia y ninguna aserción se mueve.
- **No se publica nada en este change.** La `0.4.0` se publica al final de su guion, con el
  `workflow_dispatch` que ya existe; aquí solo se documenta y se cablea el workflow.
- **No se versiona ni se guarda el token en ninguna parte.** Vive en los secretos del repositorio y se
  consume al vuelo; no hay fichero, ni `Cargo.toml`, ni documentación que lo contenga.
- **No se toca el `exclude` de `Cargo.toml`.** El paquete ya lleva lo que tiene que llevar —las once
  páginas y la hoja, que son las que el binario embebe—, y `/.github` no es del crate; cablear el
  workflow **no cambia** el artefacto que se publica.

## Impact

- `.github/workflows/release.yml` — un job nuevo (`crate`), con `needs: imagen`, la comprobación de
  versión contra la etiqueta, la idempotencia y el `cargo publish --locked`.
- `.github/workflows/ci.yml` — un paso nuevo en el job `puertas` (`cargo publish --dry-run --locked`),
  que no necesita ningún check requerido nuevo porque va dentro de uno que ya lo es.
- `AGENTS.md` — la sección de entrega (el tag publica imagen **y** crate, y el secreto que hace falta)
  y, si procede, un quirk sobre la idempotencia de republicar.
- `CHANGELOG.md` — un bullet de Mantenimiento en la `0.4.0`, que todavía no está publicada en
  crates.io y por tanto se puede corregir sin reescribir historia publicada en el registro.
- `openspec/specs/http-server/spec.md` — vía la delta de este change, que añade el requisito de
  entrega.
- `src/**`, `content/**`, `tests/**`, `Cargo.toml`, `Cargo.lock` y `Dockerfile` — **sin cambios**.
