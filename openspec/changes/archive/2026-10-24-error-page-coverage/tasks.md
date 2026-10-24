# Tasks: error-page-coverage

## 0. Prerrequisito

- [ ] 0.1 Sin dependencias nuevas: este change solo toca `content/`, pruebas y documentación.

## 1. RED (pruebas que fallan primero)

- [x] 1.1 `tests/` o el módulo que corresponda: prueba que recorre la lista cubierta —`400`, `401`, `403`, `404`, `405`, `408`, `429`, `500`, `502`, `503`, `504`— y exige, para cada código, que exista `CONTENT_DIR/<código>.html` y que su cuerpo contenga ese mismo código.
- [x] 1.2 Prueba que extrae los `href` locales de cada página de `content/` y exige que cada ruta resuelva a un fichero existente dentro de `CONTENT_DIR`.
- [x] 1.3 Prueba que exige que toda página referencie `css/error.css` —nunca la hoja de otro código—, que ese fichero exista y que cada `body` lleve la clase `c<código>`.
- [x] 1.4 Ejecutar `cargo test`: las pruebas nuevas fallan (faltan cinco páginas, sus hojas y los `href` siguen apuntando al 404).

## 2. GREEN (mínimo para que pasen)

- [x] 2.1 `content/`: las páginas `400.html`, `401.html`, `405.html`, `408.html` y `504.html`, con el mismo formato que las existentes y nombrando su propio código.
- [x] 2.2 `content/css/error.css` (la hoja única, renombrada desde `404.css`), los `href` de las once páginas apuntando a ella y la clase `c<código>` en cada `body`.
- [x] 2.3 `cargo test` en verde.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Sin `unwrap`/`expect`/`panic!` en producción (las pruebas nuevas no deben introducir ninguno fuera de `#[cfg(test)]`).

## 4. Verificación manual (operador)

- [x] 4.1 `curl -i localhost:8080/504` → `504` y el cuerpo nombra `504`.
- [x] 4.2 `curl -i localhost:8080/401` → `401` y el cuerpo nombra `401`.
- [x] 4.3 `curl -i localhost:8080/css/error.css` → `200` con `Content-Type` `text/css` (la hoja es **única**).
- [x] 4.4 `curl -i localhost:8080/504` con `Accept: text/html` → HTML, no texto plano, y el `body` con `class="c504"`.

## 5. Documentación y cierre

- [x] 5.1 `README.md`: la lista de códigos cubiertos y el **reparto de responsabilidad del código con Traefik** —quien decide es Traefik cuando el middleware `errors` intercepta; nosotros, en el acceso directo (catch-all o default backend)—.
- [x] 5.2 `AGENTS.md`: lo mismo, en la sección de garantías.
- [x] 5.3 `openspec validate error-page-coverage --strict` en verde.
- [x] 5.4 Archivar (`openspec archive error-page-coverage --yes`).
