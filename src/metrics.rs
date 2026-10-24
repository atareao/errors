//! Contador de páginas de error servidas, por código, y su serialización en
//! texto Prometheus.
//!
//! El contador responde a la única pregunta que hoy no se puede contestar sin
//! contar líneas de registro: «¿cuántos `503` he servido esta hora?». Es
//! deliberadamente mínimo: un contador por código y nada más (ni histogramas ni
//! latencias: un servicio que sirve ficheros no las mira nadie).
//!
//! El contador es del **router**, no del proceso: vive en su estado, se comparte
//! entre sus clones y **no** entre routers distintos. Un `static` global sería
//! más corto de escribir y sería el contador del servicio en producción (donde
//! hay un único router), pero contaminaría los conteos de las pruebas, que
//! corren en paralelo y piden `/404` y `/503` en routers distintos: cada prueba
//! que mide cuentas necesita un contador propio para ser determinista.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::resolve::{MAX_ERROR_CODE, MIN_ERROR_CODE, is_usable_code};

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

/// Contador de páginas servidas por código.
///
/// Una tabla de `AtomicU64` indexada por `code - MIN_ERROR_CODE`: sin cerrojos
/// (cada incremento es un `fetch_add` relajado, basta para un contador de
/// sucesos) y con memoria **acotada por construcción**, porque el índice solo
/// existe para códigos del rango utilizable: `/2` o `/1000` no crean ninguna
/// entrada, así que un cliente no puede hacer crecer la tabla pidiendo códigos
/// arbitrarios.
#[derive(Clone)]
pub struct PageCounters {
    counters: Arc<[AtomicU64; CODES]>,
}

impl PageCounters {
    /// Crea el contador a cero.
    pub fn new() -> Self {
        Self {
            counters: Arc::new([const { AtomicU64::new(0) }; CODES]),
        }
    }

    /// Suma uno al contador del código.
    ///
    /// Un código fuera de `400..=599` no se cuenta y no crea entrada: es
    /// inalcanzable desde el camino de las páginas de error
    /// ([`crate::resolve::resolve_code`] solo entrega códigos utilizables), pero
    /// aquí se comprueba por si el índice se llegara a calcular mal.
    pub fn record(&self, code: u16) {
        let Some(index) = index_of(code) else {
            return;
        };

        self.counters[index].fetch_add(1, Ordering::Relaxed);
    }

    /// Valor actual del contador de un código.
    pub fn get(&self, code: u16) -> u64 {
        match index_of(code) {
            Some(index) => self.counters[index].load(Ordering::Relaxed),
            None => 0,
        }
    }

    /// Serializa el contador en el formato de texto de Prometheus.
    ///
    /// Solo se emiten las series que se han contado alguna vez: las 200 líneas
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
            let value = self.get(code);
            if value > 0 {
                out.push_str(&format!("{COUNTER_NAME}{{code=\"{code}\"}} {value}\n"));
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
    /// Resumen legible: los códigos contados y sus valores, no las 200 celdas.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut list = f.debug_map();
        for code in MIN_ERROR_CODE..=MAX_ERROR_CODE {
            let value = self.get(code);
            if value > 0 {
                list.entry(&code, &value);
            }
        }
        list.finish()
    }
}

/// Índice de un código en la tabla; `None` si no está en el rango utilizable.
fn index_of(code: u16) -> Option<usize> {
    if !is_usable_code(code) {
        return None;
    }

    Some(usize::from(code - MIN_ERROR_CODE))
}
