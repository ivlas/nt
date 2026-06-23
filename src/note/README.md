# note/

Note-domain utilities: ids, dates, and CommonMark body parsing.

| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports the public API. |
| `id.rs` | Note id validation, id-to-iso conversion, and collision-safe id allocation. |
| `date.rs` | Timestamps, calendar date validation, and date arithmetic. |
| `body.rs` | Title extraction and URL source extraction from CommonMark bodies. |
