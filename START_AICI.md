# AVM Subscription Hub

Contractul nou este pentru abonamente preplătite și facturare, cu două monede,
mai mulți comercianți, prețuri versionate, anulare proporțională, închidere
negociată și retrageri separate pentru clienți, comercianți și platformă.

Față de primul proiect, clientul poate retrage efectiv banii nefolosiți, iar
sloturile abonamentelor închise se reutilizează cu identificatori noi.

## Verificare

Pe Linux compatibil cu SDK-ul, cu Python 3.11+, Bash și Rust 1.93.0:

```bash
python3 scripts/setup_sdk.py
bash scripts/check.sh
```

Ai nevoie de targetul wasm32-unknown-unknown, rustfmt și clippy. Scripturile fixează
SDK 0.3.0 și Binaryen 133 și verifică SHA-256 pentru pachetele descărcate.
Dependențele Rust sunt incluse în arhivă și sunt fixate în Cargo.lock.

Sunt incluse 107 teste Rust (98 proprii + 9 SDK), 4.096 vectori calculați independent
cu întregi Python și trei scenarii în standul oficial AVM. Acestea verifică
ambele monede, ieșirea completă din contract, 64 de abonamente succesive,
16 simultane și un istoric de 128 callback-uri, plus replay.

Documentația pentru review este în engleză: README.md, docs/SPEC.md și docs/REVIEW.md.
Dovezile sunt în evidence/. Artefactele compilate sunt în artifacts/.

Este un exemplu funcțional pentru testele SDK, nu un audit pentru fonduri reale.
Limita de 128 încercări de retragere este per deployment; nu există migrare sau
recuperare automată pentru un callback pierdut. Detaliile sunt explicate în README.
Nu a fost publicat încă pe GitHub; workflow-ul CI este pregătit.
