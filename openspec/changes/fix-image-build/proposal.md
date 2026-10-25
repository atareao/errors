# Proposal: fix-image-build

## Why

Etiquetar la `0.4.0` **no publicó la imagen**: el workflow `release.yml` falló construyéndola con
`exit code 101` y no dejó ni imagen en GHCR ni Release. El stage `builder` del `Dockerfile` compila
el crate —líneas 37-48: copia `Cargo.toml` y `Cargo.lock`, escribe un `src/main.rs` de relleno con
`cargo build --locked` para cachear el árbol de dependencias, y luego `COPY src ./src` y vuelve a
compilar—, pero **nunca copia `content/`**, que solo llega al stage `runtime` (línea 76), que corre
**después**. Como el change `embedded-error-pages` —ya archivado y en `main`— hizo que las macros
`include_str!`/`include_bytes!` lean esos doce ficheros **al compilar**, `cargo build` muere antes
de producir el binario:

```
error: couldn't read `src/../content/429.html`: No such file or directory (os error 2)
   --> src/embedded.rs:53:11     (…y así hasta 12 errores; el 12.º es `content/css/error.css`)
error: could not compile `errserve` (bin "errserve") due to 12 previous errors
```

**El código no está mal.** El binario compila con el árbol del repositorio delante y las 143
pruebas pasan; lo que dejó de ser coherente es **el empaquetado con el embedding**. Y no lo cazó
ninguna puerta porque **la que lo habría cazado no existía**: `ci.yml` corre `cargo test` con el
árbol del repositorio delante, donde `content/` sí está, y **nunca construye la imagen**; la imagen
solo se construye en `release.yml`, que se ejecuta **al etiquetar**. Un fallo de empaquetado solo
podía descubrirse por primera vez en el peor momento posible, con el tag ya empujado y el Release a
medias.

El `design.md` del change anterior se equivocó al dar por hecho que «no se toca la imagen»
equivalía a «el comportamiento queda idéntico»: no tocar el `Dockerfile` cuando las páginas se
embeben en **tiempo de compilación** **no** era inocuo, era dejar a `cargo build` sin los ficheros
que necesita.

## What Changes

1. **El stage `builder` del `Dockerfile` dispone de `content/` antes de compilar** —un
   `COPY content ./content`—, porque las once páginas y la hoja `css/error.css` se embeben al
   compilar y `cargo` no puede construir el crate sin ellas (las macros leen los ficheros, no su
   ausencia). La copia del stage `runtime` (línea 76) **se conserva**: `CONTENT_DIR` sigue siendo la
   única ruta de estáticos y la capa del disco sigue ganando, así que lo que se sirve en producción
   son esos ficheros.
2. **Una puerta nueva en el CI**: un job que **construye la imagen** en cada propuesta de cambio y en
   cada push, para que un fallo de empaquetado no vuelva a descubrirse por primera vez al publicar
   una etiqueta. La configuración es **sencilla**: `docker build` con el docker del propio runner,
   sin actions de terceros nuevas ni caché compartida.
3. **La prosa que quedó mal se corrige.** El quirk de `AGENTS.md` dice que la imagen copia
   `content/` «a propósito», como si fuera una redundancia *elegida*, cuando en el `builder` esa
   copia es **obligatoria**; y la entrada «Cambiado» del `CHANGELOG.md` de la `0.4.0` lo insinúa
   igual («la imagen no cambia»). Las dos frases describen un `Dockerfile` que ya no compilaba.

## Capabilities

### New Capabilities

(ninguna: esto no añade una capacidad al servicio, sino que **devuelve el empaquetado a la
coherencia con el embedding** que ya existe)

### Modified Capabilities

- `http-server`: **un requisito nuevo** de empaquetado —que la imagen se construya desde el árbol del
  repositorio y que el stage de **compilación** tenga las páginas dentro—, con sus escenarios. El
  requisito describe un hecho del empaquetado, no del servicio en ejecución: nada de lo que hoy sirve
  el binario cambia.

## Non-goals

- **No se rediseña el embedding.** Las macros `include_str!`/`include_bytes!` se quedan: son las que
  hacen que un `cargo install errserve` sirva páginas de verdad sin ficheros de datos, y este change
  no las toca.
- **No se quita la copia de `content/` del stage `runtime`.** Esa decisión sigue siendo la *Open
  Question* congelada del change anterior, y quitarla cambiaría qué bytes se sirven en producción
  (la capa del disco gana). Aquí solo se añade la copia que falta —la del `builder`—, no se retira la
  que ya había.
- **No se toca `src/**` ni las pruebas.** El fallo no está en el código: está en el empaquetado. Ni
  una línea de Rust cambia y ninguna aserción se mueve.
- **No se añaden dependencias ni actions de terceros.** El `COPY` es una línea del `Dockerfile` y la
  puerta nueva usa el `docker` que ya trae el runner; `cargo deny` no ve nada nuevo.
- **No se toca la lógica del contenedor** —usuario, healthcheck, `EXPOSE`, `CMD`, etiquetas OCI—: es
  exactamente la de la `0.3.0` y su comprobación sigue valiendo.

## Impact

- `Dockerfile` — una línea nueva en el stage `builder` (`COPY content ./content`) y su comentario. El
  stage `runtime` no cambia.
- `.github/workflows/ci.yml` — un job nuevo (`imagen`) que construye la imagen, y con él un check
  nuevo que habrá que marcar como requerido en `main` y en `development`.
- `AGENTS.md` — la lista/tabla de módulos si procede, el quirk de la imagen y el de `include_str!`,
  que hoy cuentan un `Dockerfile` que no compilaba.
- `CHANGELOG.md` — la entrada de la `0.4.0`, que todavía **no está publicada** (no hay imagen ni
  Release), así que se puede corregir sin reescribir historia publicada.
- `openspec/specs/http-server/spec.md` — vía la delta de este change, que añade el requisito de
  empaquetado.
- `src/**`, `content/**`, `tests/**`, `Cargo.toml` y `Cargo.lock` — **sin cambios**.
