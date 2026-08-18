/*
 * expanse.h — placeholder for the modern libexpanse API header.
 *
 * The expanse_* API (expanse_set_t, expanse_map_t, expanse_strmap_t,
 * expanse_bytesmap_t) exposes the engine's modern capabilities under a
 * clean namespace. It is additive: the legacy Judy compat surface lives in
 * Judy.h and never changes semantics (docs/COMPAT.md).
 *
 * The header lands together with the first exported functions (Phases 4
 * and 6); until then including it is a compile-time error by design.
 */
#ifndef EXPANSE_H
#define EXPANSE_H

#error "libexpanse does not export the modern API yet - see docs/COMPAT.md for the roadmap"

#endif /* EXPANSE_H */
