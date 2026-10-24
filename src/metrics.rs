//! Contador de páginas de error servidas, por código **y por el origen del que
//! salió ese código**, y su serialización en texto Prometheus.
//!
//! El contador responde a la única pregunta que hoy no se puede contestar sin
//! contar líneas de registro: «¿cuántos `503` he servido esta hora?». Es
//! deliberadamente mínimo: un contador por par `(código, origen)` y nada más
//! (ni histogramas ni latencias: un servicio que sirve ficheros no las mira
//! nadie).
//!
//! El contador es del **router**, no del proceso: vive en su estado, se comparte
//! entre sus clones y **no** entre routers distintos. Un `static` global sería
//! más corto de escribir y sería el contador del servicio en producción (donde
//! hay un único router), pero contaminaría los conteos de las pruebas, que
//! corren en paralelo y piden `/404` y `/503` en routers distintos: cada prueba
//! que mide cuentas necesita un contador propio para ser determinista.
//!
//! ---------------------------------------------------------------------------
//! La serie lleva el **origen del código** como segunda etiqueta, junto a
//! `code` y en ese orden:
//!
//! ```text
//! errors_pages_total{code="404",source="path"} 3
//! errors_pages_total{code="503",source="header"} 1
//! errors_pages_total{code="404",source="default"} 1
//! ```
//!
//! Los tres valores de `source` son `path` (el código lo aportó el path),
//! `header` (lo aportó la cabecera `X-Code`) y `default` (no lo aportó ninguno:
//! es el código por defecto). La etiqueta está ahí porque la cabecera `X-Code`
//! la elige quien llama: sin ella, cincuenta peticiones con `X-Code: 503`
//! fabrican el mismo `errors_pages_total{code="503"} 50` que una avalancha
//! real, y la única señal de observabilidad del servicio se puede inventar.
//!
//! Sigue emitiéndose solo la serie que se ha contado alguna vez (valor mayor
//! que cero), ahora por par `(code, source)`, y la ayuda y el tipo no cambian.
//!
//! Quién decide el origen es la cascada del código, así que lo entrega
//! [`crate::resolve`] ([`resolve_code_with_source`](crate::resolve::resolve_code_with_source)
//! y [`CodeSource`]): esta capa solo lo etiqueta.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::resolve::{CodeSource, MAX_ERROR_CODE, MIN_ERROR_CODE, is_usable_code};

/// Nombre del contador, tal cual aparece en `/metrics`.
pub const COUNTER_NAME: &str = "errors_pages_total";

/// Línea de ayuda (`# HELP`) del contador, en el formato exacto de Prometheus.
const HELP_LINE: &str = "# HELP errors_pages_total Páginas de error servidas, por código.";

/// Línea de tipo (`# TYPE`) del contador.
const TYPE_LINE: &str = "# TYPE errors_pages_total counter";

/// `Content-Type` del texto Prometheus (`text/plain; version=0.0.4`).
pub const CONTENT_TYPE: &str = "text/plain; version=0.0.4";

/// Número de códigos contables: el rango utilizable completo.
const CODES: usize = (MAX_ERROR_CODE - MIN_ERROR_CODE + 1) as usize;

/// Número de orígenes contables: las tres variantes de [`CodeSource`].
const SOURCES: usize = 3;

/// Celdas de la tabla: un par `(código, origen)` por combinación.
const CELLS: usize = CODES * SOURCES;

/// Orígenes en el orden en que se recorren (el `source` va después de `code`).
const SOURCE_ORDER: [CodeSource; SOURCES] =
    [CodeSource::Path, CodeSource::Header, CodeSource::Default];

/// Contador de páginas servidas por par `(código, origen)`.
///
/// Una tabla de `AtomicU64` indexada por el código y el origen: sin cerrojos
/// (cada incremento es un `fetch_add` relajado, basta para un contador de
/// sucesos) y con memoria **acotada por construcción** —200 códigos por tres
/// orígenes, siempre las mismas celdas—, porque el índice solo existe para
/// códigos del rango utilizable: `/2` o `/1000` no crean ninguna entrada, así
/// que un cliente no puede hacer crecer la tabla pidiendo códigos arbitrarios.
#[derive(Clone)]
pub struct PageCounters {
    counters: Arc<[AtomicU64; CELLS]>,
}

impl PageCounters {
    /// Crea el contador a cero.
    pub fn new() -> Self {
        Self {
            counters: Arc::new([const { AtomicU64::new(0) }; CELLS]),
        }
    }

    /// Suma uno al contador del par `(código, origen)`.
    ///
    /// Un código fuera de `400..=599` no se cuenta y no crea entrada: es
    /// inalcanzable desde el camino de las páginas de error
    /// ([`crate::resolve::resolve_code_with_source`] solo entrega códigos
    /// utilizables), pero aquí se comprueba por si el índice se llegara a
    /// calcular mal.
    pub fn record(&self, code: u16, source: CodeSource) {
        let Some(index) = index_of(code, source) else {
            return;
        };

        self.counters[index].fetch_add(1, Ordering::Relaxed);
    }

    /// Valor actual del contador del par `(código, origen)`.
    pub fn get(&self, code: u16, source: CodeSource) -> u64 {
        match index_of(code, source) {
            Some(index) => self.counters[index].load(Ordering::Relaxed),
            None => 0,
        }
    }

    /// Serializa el contador en el formato de texto de Prometheus.
    ///
    /// Solo se emiten las series que se han contado alguna vez: las 600 líneas
    /// con ceros serían ruido en cada raspado y no aportan nada (una serie puede
    /// aparecer más tarde sin que nadie se pierda un suceso). La cabecera con la
    /// ayuda y el tipo van siempre, también con el contador a cero, porque son
    /// la descripción de la métrica y no un valor.
    pub fn render(&self) -> String {
        let mut out = String::from(HELP_LINE);
        out.push('\n');
        out.push_str(TYPE_LINE);
        out.push('\n');

        for code in MIN_ERROR_CODE..=MAX_ERROR_CODE {
            for source in SOURCE_ORDER {
                let value = self.get(code, source);
                if value > 0 {
                    out.push_str(&format!(
                        "{COUNTER_NAME}{{code=\"{code}\",source=\"{}\"}} {value}\n",
                        source_label(source)
                    ));
                }
            }
        }

        out
    }
}

impl Default for PageCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PageCounters {
    /// Resumen legible: los pares contados y sus valores (`404/path`), no las
    /// 600 celdas.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_map();
        for code in MIN_ERROR_CODE..=MAX_ERROR_CODE {
            for source in SOURCE_ORDER {
                let value = self.get(code, source);
                if value > 0 {
                    list.entry(&format!("{code}/{}", source_label(source)), &value);
                }
            }
        }
        list.finish()
    }
}

/// Índice del par `(código, origen)` en la tabla; `None` si el código no está
/// en el rango utilizable.
fn index_of(code: u16, source: CodeSource) -> Option<usize> {
    if !is_usable_code(code) {
        return None;
    }

    Some(source_index(source) * CODES + usize::from(code - MIN_ERROR_CODE))
}

/// Posición del origen en la tabla; el `match` es exhaustivo a propósito, para
/// que añadir una variante a [`CodeSource`] no compile sin darle sitio aquí.
fn source_index(source: CodeSource) -> usize {
    match source {
        CodeSource::Path => 0,
        CodeSource::Header => 1,
        CodeSource::Default => 2,
    }
}

/// Etiqueta `source` del origen, tal cual aparece en `/metrics`.
fn source_label(source: CodeSource) -> &'static str {
    match source {
        CodeSource::Path => "path",
        CodeSource::Header => "header",
        CodeSource::Default => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::path::{Path, PathBuf};
    use tower::ServiceExt;

    /// Configuración de prueba: el directorio de fixtures como `CONTENT_DIR`,
    /// para no depender de los ficheros reales de `content/`.
    ///
    /// Está duplicada a propósito respecto de la de `src/main.rs` (misma
    /// decisión que en `src/render.rs`): los ayudantes de un `mod tests` no son
    /// visibles fuera de él, y ensanchar su visibilidad para compartirlos
    /// obligaría a tocar pruebas que ya están en verde.
    fn fixtures_config() -> Config {
        let content_dir: PathBuf =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content");

        Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            content_dir.to_string_lossy().into_owned(),
        )])
        .unwrap()
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec()
    }

    /// Valor de la serie de un par `(código, origen)`, leído del texto
    /// Prometheus.
    ///
    /// Se busca la **línea entera** (`errors_pages_total{code="404",source="path"} 3`)
    /// con sus dos etiquetas, en el orden que fija la API asumida en la
    /// cabecera del módulo: un `contains` sobre el cuerpo daría por buena una
    /// etiqueta a medias, y buscar solo por `code` confundiría los orígenes.
    fn serie(cuerpo: &str, code: u16, source: &str) -> Option<u64> {
        let prefijo = format!("{COUNTER_NAME}{{code=\"{code}\",source=\"{source}\"}} ");

        cuerpo
            .lines()
            .find_map(|linea| linea.strip_prefix(&prefijo))
            .and_then(|valor| valor.trim().parse().ok())
    }

    #[tokio::test]
    async fn el_contador_distingue_de_donde_salio_el_codigo() {
        // La cabecera `X-Code` la elige quien llama: sin la etiqueta de origen,
        // cincuenta peticiones con `X-Code: 503` fabrican
        // `errors_pages_total{code="503"} 50` y la única señal de observabilidad
        // del servicio se puede inventar. Router propio de la prueba, para que
        // las cuentas no dependan de las demás pruebas (que corren en paralelo
        // y también piden `/404`).
        let router = crate::build_router(&fixtures_config());

        // Tres páginas pedidas por su path…
        for _ in 0..3 {
            let response = router.clone().oneshot(get("/404")).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // …una con el código en la cabecera…
        let con_cabecera = Request::builder()
            .uri("/")
            .header("X-Code", "503")
            .body(Body::empty())
            .unwrap();
        let response = router.clone().oneshot(con_cabecera).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // …y una sin ninguna pista, que cae al código por defecto.
        let response = router.clone().oneshot(get("/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = router.oneshot(get("/metrics")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cuerpo = String::from_utf8(body_bytes(response).await).unwrap();

        assert_eq!(
            serie(&cuerpo, 404, "path"),
            Some(3),
            "las páginas pedidas por su path se cuentan con origen `path`:\n{cuerpo}"
        );
        assert_eq!(
            serie(&cuerpo, 503, "header"),
            Some(1),
            "una página con `X-Code: 503` se cuenta con origen `header`:\n{cuerpo}"
        );
        assert_eq!(
            serie(&cuerpo, 503, "path"),
            None,
            "un código que vino de la cabecera no puede contarse como si viniera del path:\n{cuerpo}"
        );
        assert_eq!(
            serie(&cuerpo, 404, "default"),
            Some(1),
            "una página sin pistas se cuenta con origen `default`:\n{cuerpo}"
        );
    }
}
