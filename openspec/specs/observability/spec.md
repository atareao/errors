# observability Specification

## Purpose
Lo que hace operable al servicio: el formato del registro, elegible entre texto y JSON con `LOG_FORMAT`, y el contador de páginas servidas por código que se expone en `GET /metrics` como endpoint de servicio, en texto Prometheus. Es el mínimo que responde «cuántos 503 he servido esta hora» sin contar líneas de log.

## Requirements

### Requirement: El formato del registro es configurable

`LOG_FORMAT` SHALL decidir el formato de los registros: `text` —el actual— por defecto, y `json` para que un colector los parsee sin expresiones regulares. Un valor distinto de esos dos SHALL ser un error de arranque que nombre `LOG_FORMAT`.

#### Scenario: json produce líneas parseables
- **GIVEN** `LOG_FORMAT=json`
- **WHEN** el servicio arranca y sirve una petición
- **THEN** cada línea del registro SHALL ser un objeto JSON válido

#### Scenario: el defecto es texto
- **GIVEN** un entorno sin `LOG_FORMAT`
- **WHEN** el servicio arranca
- **THEN** los registros SHALL ir en texto

#### Scenario: un formato desconocido no arranca
- **GIVEN** `LOG_FORMAT=basura`
- **WHEN** se carga la configuración
- **THEN** SHALL fallar con un error que nombre `LOG_FORMAT`

### Requirement: Las páginas servidas se cuentan por código

El servicio SHALL contar las páginas de error servidas, por código **y por el origen del que salió ese código** —el path, la cabecera `X-Code` o el defecto—, y exponerlas en `GET /metrics` en texto Prometheus. `GET /metrics` SHALL responder `200` como endpoint de servicio y NO SHALL pasar por la resolución de páginas de error.

El código contado SHALL ser el de la página **servida**: la serie que se incrementa y el estado con el que sale la respuesta NO SHALL diferir.

El contador NO SHALL crecer sin límite: solo cuenta códigos del rango utilizable.

El origen SHALL distinguirse porque la cabecera `X-Code` la elige quien llama: sin esa etiqueta, un tercero puede inflar a voluntad el contador de un código y fabricar la apariencia de una avalancha justo durante una incidencia.

#### Scenario: el contador distingue de dónde salió el código
- **GIVEN** un servicio recién arrancado
- **WHEN** se piden varias páginas por su path y varias más indicando el código con la cabecera
- **THEN** `/metrics` SHALL declarar las primeras con origen `path`
- **AND** las segundas con origen `header`

#### Scenario: el path gana y la etiqueta lo dice
- **GIVEN** un servicio recién arrancado
- **WHEN** se pide `/404` con la cabecera `X-Code: 503`
- **THEN** el estado SHALL ser `404`
- **AND** `/metrics` SHALL declararlo con origen `path` y NO con origen `header`

#### Scenario: el contador refleja lo servido
- **GIVEN** un servicio recién arrancado
- **WHEN** se piden N páginas de error con código `404` y M con código `503`
- **THEN** `/metrics` SHALL declarar N para el `404` y M para el `503`

#### Scenario: metrics es un endpoint de servicio
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /metrics`
- **THEN** SHALL responder `200`
- **AND** el cuerpo NO SHALL ser una página de error

#### Scenario: las métricas declaran su tipo
- **GIVEN** una respuesta de `/metrics`
- **WHEN** se inspecciona
- **THEN** SHALL incluir la ayuda y el tipo del contador conforme al formato Prometheus
