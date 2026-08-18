/*
 * Judy.h — placeholder for the libexpanse legacy-compat header.
 *
 * libexpanse ships two headers: expanse.h (the modern expanse_* API) and
 * this one, source-compatible with the classic libjudy Judy.h — the Judy1 /
 * JudyL / JudySL function prototypes, the J1S/JLI/JSLG-style convenience
 * macros, and the Pvoid_t / PPvoid_t / JError_t / PJERR / JERR conventions,
 * as specified in docs/COMPAT.md.
 *
 * This is a clean-room project: the header is written from the documented
 * API contract (published man pages and API documentation), not copied from
 * the LGPL libjudy sources. The full header lands together with the first
 * exported functions (Phases 4 and 6); until then including it is a
 * compile-time error by design.
 */
#ifndef EXPANSE_COMPAT_JUDY_H
#define EXPANSE_COMPAT_JUDY_H

#error "libexpanse does not export the Judy compat API yet - see docs/COMPAT.md for the roadmap"

#endif /* EXPANSE_COMPAT_JUDY_H */
