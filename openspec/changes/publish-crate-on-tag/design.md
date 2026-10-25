# Design: publish-crate-on-tag

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- **El release publica la imagen y crea el Release, y para ahí.** `release.yml` es un solo job,
  `imagen` (`name: construir, probar y publicar`), y sus pasos van de construir a probar con docker
  real, entrar en GHCR, publicar y crear el Release. **Ninguno menciona `cargo publish`** —medido—,
  que es la causa directa de que crates.io siga en `0.3.0`.
- **Publicar en crates.io es irreversible.** Una versión publicada se puede *yank* —ocultar— pero no
  reemplazar ni borrar. Por eso la operación que no se puede deshacer va **después** de las que sí.
- **`cargo publish` publica lo que diga `Cargo.toml`, no lo que diga el tag.** El número de versión
  del paquete sale del manifiesto; la etiqueta es una convención del repositorio. Sin comprobación,
  los dos pueden discrepar sin que nadie se entere.
- **El workflow está pensado para re-ejecutarse.** El `workflow_dispatch` con input `tag` republica
  una etiqueta ya existente, y el paso del Release ya reutiliza el que ya está (`gh release view …`).
  Un `cargo publish` a secas rompería esa propiedad con un rojo falso.
- **La API de crates.io responde `403` a una petición sin `User-Agent`** —medido hoy al consultarla—,
  así que cualquier comprobación por HTTP tiene que mandar uno o dará un resultado engañoso.
- **`ci.yml` no verifica que el paquete sea publicable.** Tiene tres jobs —`puertas`
  (`fmt, clippy y tests`), `dependencias` (`cargo-deny`) e `imagen`—, los tres requeridos en `main` y
  en `development`, y ninguno empaqueta el crate.
- **El `exclude` de `Cargo.toml`** es `["/openspec", "/.github", "/AGENTS.md", "/cliff.toml",
  "/deny.toml", "/.justfile"]`, así que `/.github` no entra en el paquete: cablear el workflow no
  cambia el artefacto. `content/` **sí** entra, y es lo que hace que el binario instalado sirva las
  páginas.

## Goals / Non-Goals

**Goals**

- Que un tag `v*` publique **la imagen y el crate**, con la versión comprobada contra la etiqueta.
- Que lo irreversible ocurra **solo después** de que lo demás haya pasado, y **nunca** antes.
- Que republicar una etiqueta no rompa por una versión ya publicada.
- Que un paquete que no se puede publicar se descubra **en el PR**, no al publicar.

**Non-Goals**

- No se cambia la imagen, ni su smoke test, ni el `Dockerfile`.
- No se toca el código, ni las pruebas, ni el manifiesto, ni el `exclude`.
- No se publica nada en este change, ni se versiona el token en ningún sitio.

## Decisions

### D1 — Por qué un job aparte, y con `needs: imagen`

`cargo publish` es **irreversible**; construir y probar la imagen, no. Si el crate se publicara en
**paralelo** a la imagen y la imagen fallara, quedaría una versión publicada de la que no hay
artefacto: el peor estado posible, porque el crate **no se puede retirar** —solo *yank*, que oculta
pero deja la versión ocupada—. Con `needs: imagen`, el crate se publica **cuando ya hay una imagen
probada y publicada**, de modo que lo que no se puede deshacer va último y solo sobre terreno firme.

Va en **job aparte** —y no como un paso más del job `imagen`— por dos razones. La primera es de
responsabilidad: un job, una cosa que publicar; meter el crate en el job que publica la imagen
mezclaría dos artefactos con ciclos de vida distintos en un mismo sitio. La segunda es la herramienta:
el job `imagen` es de **docker**, no tiene **toolchain de Rust**, y añadir el crate ahí obligaría a
instalar Rust en un job que no lo necesita —solo para invocar `cargo publish`—. Un job `crate` con su
propio `rustup toolchain install stable` reintroduce la herramienta donde hace falta, y el `needs:
imagen` es lo que garantiza el orden.

### D2 — Por qué se comprueba la versión contra la etiqueta

`cargo publish` publica **lo que diga `Cargo.toml`**, no lo que diga el tag. Sin la comprobación,
etiquetar `v0.4.1` con `Cargo.toml` en `0.4.0` daría dos desenlaces, los dos malos:

- **Publicar la `0.4.0` otra vez.** Si la `0.4.0` ya está publicada, `cargo publish` falla —porque una
  versión no se puede reemplazar—, pero con un error que **no explica la causa real**: habla de
  duplicado, no de que la etiqueta y el manifiesto no cuadran. Quien lea ese rojo buscará el problema
  donde no está.
- **Publicar una versión que no es la que se anuncia.** Si la `0.4.0` no estuviera publicada, subiría
  ese número mientras la etiqueta dice `v0.4.1`: un artefacto cuyo nombre y cuyo contenido discrepan,
  y ya sin vuelta atrás.

La comprobación —comparar la versión de `Cargo.toml` con la de la etiqueta— convierte los dos casos en
**un error claro y temprano**, que nombra el desajuste. Es la clase de error que en un artefacto
irreversible hay que cazar **antes**, no después.

### D3 — Por qué el paso es idempotente

El workflow ya está pensado para re-ejecutarse sobre una etiqueta publicada: el `workflow_dispatch`
con input `tag` existe para eso, y el paso del Release ya reutiliza el existente si lo encuentra. Un
`cargo publish` a secas rompería esa propiedad: la segunda ejecución fallaría con «crate version
already uploaded», que es un **rojo falso** —la versión está publicada, que es justo lo que se quería—.
Así que el paso **consulta el registro** y, si la versión ya está, avisa y **continúa** en verde.

Ojo con el detalle medido: la API de crates.io responde **`403` sin `User-Agent`**, así que la consulta
tiene que mandar uno. Sin eso la comprobación daría un falso «no está publicado» —el `403` parece «no
encontrado» si no se mira— y el `cargo publish` posterior fallaría por duplicada, que es exactamente lo
que esta decisión existe para evitar.

### D4 — Por qué la puerta del paquete va en el CI

Es la misma lección de `fix-image-build`, ahora sobre el crate: hasta hoy **el paquete publicado solo
se verificaba en el momento de publicarlo**, y en parte **a mano** —una medición de `cargo package
--list` en una sesión—. `cargo publish --dry-run --locked` empaqueta, compila el paquete y valida sus
metadatos **sin subir nada**, y cabe en el job `puertas`, que **ya tiene toolchain y caché**: no
necesita un job nuevo ni una instalación de Rust, solo un paso más. Con eso, un paquete que no se puede
publicar —un `content/` que no viaja, un `Cargo.toml` inválido, una dependencia que falta— se ve **en
el PR**, no la primera vez que se publica una etiqueta. La puerta **no** necesita un check requerido
nuevo: va dentro de `puertas`, que ya lo es.

## Risks / Trade-offs

- [El release tarda más] → el job `crate` instala el toolchain, empaqueta y compila el paquete antes de
  publicar: unos minutos más por release. Se acepta: es el precio de que lo irreversible ocurra sobre
  terreno firme, y el release es raro y deliberado.
- [El `dry-run` del CI añade tiempo a cada PR] → `cargo publish --dry-run` compila el paquete entero
  —una compilación *release* del crate, aparte de las de `puertas`—. Se acepta a cambio de que el
  empaquetado deje de ser el único camino sin puerta; el job `puertas` ya tiene caché de Cargo, que es
  donde se paga el grueso.
- [El paso de publicación depende de la red] → la consulta a crates.io para la idempotencia (D3) puede
  fallar por un corte de red, y daría un rojo que **no es del paquete**. Se acepta: se prefiere un rojo
  ruidoso y revisable a publicar a ciegas. Es el mismo trato que el paso del aviso de GHCR, que también
  habla por HTTP y no falla el release.

## Migration Plan

1. Crear el secreto `CARGO_REGISTRY_TOKEN` en el repositorio (a mano, en los ajustes de Actions).
2. Añadir el paso del `dry-run` al job `puertas` de `ci.yml`, y verlo en verde en el PR.
3. Añadir el job `crate` a `release.yml` con `needs: imagen`, la comprobación, la idempotencia y el
   `cargo publish --locked`.
4. Ejecutar el guion de tareas: la comprobación de versión en RED con los dos casos, y el `dry-run` en
   verde.
5. Publicar la `0.4.0` con el `workflow_dispatch` que ya existe, sobre la etiqueta ya publicada:
   comprobar que la imagen se republica sin daño, el Release se reutiliza y **el crate aparece en
   crates.io**.
6. Documentar (`AGENTS.md`, `CHANGELOG.md`) y archivar.

## Open Questions

- **¿El `workflow_dispatch` debería ser el camino normal y no la excepción?** Hoy el camino normal es
  el push de la etiqueta y el dispatch existe para republicar. Si republicar se volviera frecuente,
  tendría sentido invertirlo —el dispatch como forma canónica, con el push como atajo—, pero no es el
  caso hoy y el change no lo necesita para su motivo.
- **¿Merece un mensaje propio el «secreto ausente»?** Un `cargo publish` sin `CARGO_REGISTRY_TOKEN`
  falla con un error de cargo que **no nombra el secreto**: dice que falta la autenticación, no que
  falte *esta* variable. No cambia el diseño —el paso falla igual, en el mismo sitio— y por eso no se
  resuelve aquí; si el rojo resulta confuso, un paso previo que compruebe la variable y dé un mensaje
  claro es un cambio de una línea.
