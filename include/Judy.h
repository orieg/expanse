/*
 * Judy.h — libexpanse legacy-compat header.
 *
 * Source-compatible with the classic libjudy Judy.h API, written CLEAN-ROOM
 * from the documented contract (the published Judy(3), Judy1(3), JudyL(3),
 * JudySL(3) man pages) — no LGPL libjudy source consulted. The binding
 * contract, including doc-gap resolutions (Word_t width, JU_ERRNO_*
 * numbering, OOM behavior), lives in docs/COMPAT.md.
 *
 * All four families — Judy1, JudyL, JudySL, and JudyHS — are exported
 * by libexpanse (backed by ExpanseSet, ExpanseMap, ExpanseStrMap, and
 * ExpanseBytesMap respectively; docs/COMPAT.md status).
 */
#ifndef EXPANSE_COMPAT_JUDY_H
#define EXPANSE_COMPAT_JUDY_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- Core types (COMPAT.md D1: Word_t is pointer-width) ---- */
typedef size_t Word_t, *PWord_t;
typedef void *Pvoid_t;
typedef void **PPvoid_t;
typedef const void *Pcvoid_t;

/* ---- Error handling (COMPAT.md D2: numbering is libexpanse's own) ---- */
typedef enum {
    JU_ERRNO_NONE = 0,
    JU_ERRNO_FULL = 1,
    JU_ERRNO_NFMAX = JU_ERRNO_FULL,
    JU_ERRNO_NOMEM = 2,
    JU_ERRNO_NULLPPARRAY = 3,
    JU_ERRNO_NONNULLPARRAY = 4,
    JU_ERRNO_NULLPINDEX = 5,
    JU_ERRNO_NULLPVALUE = 6,
    JU_ERRNO_NOTJUDY1 = 7,
    JU_ERRNO_NOTJUDYL = 8,
    JU_ERRNO_NOTJUDYSL = 9,
    JU_ERRNO_UNSORTED = 12,
    JU_ERRNO_OVERRUN = 13,
    JU_ERRNO_CORRUPT = 14
} JU_Errno_t;

typedef struct J_UDY_ERROR_STRUCT {
    int je_Errno;          /* one of JU_Errno_t */
    int je_ErrID;          /* internal location id */
    Word_t je_reserved[4]; /* reserved for future use */
} JError_t, *PJError_t;

#define JU_ERRNO(PJError) ((PJError)->je_Errno)
#define JU_ERRID(PJError) ((PJError)->je_ErrID)

#define PJE0 ((PJError_t) NULL)
#define JERR (-1)
#define PJERR ((Pvoid_t) (size_t) (-1))
#define PPJERR ((PPvoid_t) (size_t) (-1))

/* ---- Judy1: dynamic bit set of Word_t indexes ---- */
extern int    Judy1Set(PPvoid_t PPArray, Word_t Index, PJError_t PJError);
extern int    Judy1Unset(PPvoid_t PPArray, Word_t Index, PJError_t PJError);
extern int    Judy1Test(Pcvoid_t PArray, Word_t Index, PJError_t PJError);
extern Word_t Judy1Count(Pcvoid_t PArray, Word_t Index1, Word_t Index2, PJError_t PJError);
extern int    Judy1ByCount(Pcvoid_t PArray, Word_t Nth, Word_t *PIndex, PJError_t PJError);
extern int    Judy1First(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1Next(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1Last(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1Prev(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1FirstEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1NextEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1LastEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int    Judy1PrevEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern Word_t Judy1FreeArray(PPvoid_t PPArray, PJError_t PJError);
extern Word_t Judy1MemUsed(Pcvoid_t PArray);

/* ---- JudyL: dynamic map from Word_t index to Word_t value ---- */
extern PPvoid_t JudyLIns(PPvoid_t PPArray, Word_t Index, PJError_t PJError);
extern int      JudyLDel(PPvoid_t PPArray, Word_t Index, PJError_t PJError);
extern PPvoid_t JudyLGet(Pcvoid_t PArray, Word_t Index, PJError_t PJError);
extern Word_t   JudyLCount(Pcvoid_t PArray, Word_t Index1, Word_t Index2, PJError_t PJError);
extern PPvoid_t JudyLByCount(Pcvoid_t PArray, Word_t Nth, Word_t *PIndex, PJError_t PJError);
extern PPvoid_t JudyLFirst(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern PPvoid_t JudyLNext(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern PPvoid_t JudyLLast(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern PPvoid_t JudyLPrev(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int      JudyLFirstEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int      JudyLNextEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int      JudyLLastEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern int      JudyLPrevEmpty(Pcvoid_t PArray, Word_t *PIndex, PJError_t PJError);
extern Word_t   JudyLFreeArray(PPvoid_t PPArray, PJError_t PJError);
extern Word_t   JudyLMemUsed(Pcvoid_t PArray);

/* ---- JudySL (ExpanseStrMap) / JudyHS (ExpanseBytesMap) ---- */
extern PPvoid_t JudySLIns(PPvoid_t PPArray, const unsigned char *Index, PJError_t PJError);
extern int      JudySLDel(PPvoid_t PPArray, const unsigned char *Index, PJError_t PJError);
extern PPvoid_t JudySLGet(Pcvoid_t PArray, const unsigned char *Index, PJError_t PJError);
extern PPvoid_t JudySLFirst(Pcvoid_t PArray, unsigned char *Index, PJError_t PJError);
extern PPvoid_t JudySLNext(Pcvoid_t PArray, unsigned char *Index, PJError_t PJError);
extern PPvoid_t JudySLLast(Pcvoid_t PArray, unsigned char *Index, PJError_t PJError);
extern PPvoid_t JudySLPrev(Pcvoid_t PArray, unsigned char *Index, PJError_t PJError);
extern Word_t   JudySLFreeArray(PPvoid_t PPArray, PJError_t PJError);
extern PPvoid_t JudyHSIns(PPvoid_t PPArray, void *Index, Word_t Length, PJError_t PJError);
extern int      JudyHSDel(PPvoid_t PPArray, void *Index, Word_t Length, PJError_t PJError);
extern PPvoid_t JudyHSGet(Pcvoid_t PArray, void *Index, Word_t Length);
extern Word_t   JudyHSFreeArray(PPvoid_t PPArray, PJError_t PJError);

/* ---- Convenience macros (documented shorthand layer) ---- */
/* The convenience macros are statement blocks, tolerating both classic
 * usage styles observed in consumers: `JLI(PV, A, I);` (man-page style)
 * and `JLI(PV, A, I)` bare at statement position. Consequence (doc-gap
 * D5): they are statements, not expressions, and an unbraced
 * `if (c) JLI(...); else ...` needs braces around the macro. */
#define J1S(Rc, PArray, Index)   { (Rc) = Judy1Set(&(PArray), Index, PJE0); }
#define J1U(Rc, PArray, Index)   { (Rc) = Judy1Unset(&(PArray), Index, PJE0); }
#define J1T(Rc, PArray, Index)   { (Rc) = Judy1Test((Pcvoid_t)(PArray), Index, PJE0); }
#define J1C(Rc, PArray, I1, I2)  { (Rc) = Judy1Count((Pcvoid_t)(PArray), I1, I2, PJE0); }
#define J1BC(Rc, PArray, Nth, Index) { (Rc) = Judy1ByCount((Pcvoid_t)(PArray), Nth, &(Index), PJE0); }
#define J1F(Rc, PArray, Index)   { (Rc) = Judy1First((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1N(Rc, PArray, Index)   { (Rc) = Judy1Next((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1L(Rc, PArray, Index)   { (Rc) = Judy1Last((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1P(Rc, PArray, Index)   { (Rc) = Judy1Prev((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1FE(Rc, PArray, Index)  { (Rc) = Judy1FirstEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1NE(Rc, PArray, Index)  { (Rc) = Judy1NextEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1LE(Rc, PArray, Index)  { (Rc) = Judy1LastEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1PE(Rc, PArray, Index)  { (Rc) = Judy1PrevEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define J1FA(Rc, PArray)         { (Rc) = Judy1FreeArray(&(PArray), PJE0); }
#define J1MU(Rc, PArray)         { (Rc) = Judy1MemUsed((Pcvoid_t)(PArray)); }

#define JLI(PV, PArray, Index)   { (PV) = (PWord_t) JudyLIns(&(PArray), Index, PJE0); }
#define JLD(Rc, PArray, Index)   { (Rc) = JudyLDel(&(PArray), Index, PJE0); }
#define JLG(PV, PArray, Index)   { (PV) = (PWord_t) JudyLGet((Pcvoid_t)(PArray), Index, PJE0); }
#define JLC(Rc, PArray, I1, I2)  { (Rc) = JudyLCount((Pcvoid_t)(PArray), I1, I2, PJE0); }
#define JLBC(PV, PArray, Nth, Index) { (PV) = (PWord_t) JudyLByCount((Pcvoid_t)(PArray), Nth, &(Index), PJE0); }
#define JLF(PV, PArray, Index)   { (PV) = (PWord_t) JudyLFirst((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLN(PV, PArray, Index)   { (PV) = (PWord_t) JudyLNext((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLL(PV, PArray, Index)   { (PV) = (PWord_t) JudyLLast((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLP(PV, PArray, Index)   { (PV) = (PWord_t) JudyLPrev((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLFE(Rc, PArray, Index)  { (Rc) = JudyLFirstEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLNE(Rc, PArray, Index)  { (Rc) = JudyLNextEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLLE(Rc, PArray, Index)  { (Rc) = JudyLLastEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLPE(Rc, PArray, Index)  { (Rc) = JudyLPrevEmpty((Pcvoid_t)(PArray), &(Index), PJE0); }
#define JLFA(Rc, PArray)         { (Rc) = JudyLFreeArray(&(PArray), PJE0); }
#define JLMU(Rc, PArray)         { (Rc) = JudyLMemUsed((Pcvoid_t)(PArray)); }

#define JSLI(PV, PArray, Index)  { (PV) = (PWord_t) JudySLIns(&(PArray), Index, PJE0); }
#define JSLD(Rc, PArray, Index)  { (Rc) = JudySLDel(&(PArray), Index, PJE0); }
#define JSLG(PV, PArray, Index)  { (PV) = (PWord_t) JudySLGet((Pcvoid_t)(PArray), Index, PJE0); }
#define JSLF(PV, PArray, Index)  { (PV) = (PWord_t) JudySLFirst((Pcvoid_t)(PArray), Index, PJE0); }
#define JSLN(PV, PArray, Index)  { (PV) = (PWord_t) JudySLNext((Pcvoid_t)(PArray), Index, PJE0); }
#define JSLL(PV, PArray, Index)  { (PV) = (PWord_t) JudySLLast((Pcvoid_t)(PArray), Index, PJE0); }
#define JSLP(PV, PArray, Index)  { (PV) = (PWord_t) JudySLPrev((Pcvoid_t)(PArray), Index, PJE0); }
#define JSLFA(Rc, PArray)        { (Rc) = JudySLFreeArray(&(PArray), PJE0); }

#define JHSI(PV, PArray, Index, Len) { (PV) = (PWord_t) JudyHSIns(&(PArray), Index, Len, PJE0); }
#define JHSD(Rc, PArray, Index, Len) { (Rc) = JudyHSDel(&(PArray), Index, Len, PJE0); }
#define JHSG(PV, PArray, Index, Len) { (PV) = (PWord_t) JudyHSGet((Pcvoid_t)(PArray), Index, Len); }
#define JHSFA(Rc, PArray)            { (Rc) = JudyHSFreeArray(&(PArray), PJE0); }

#ifdef __cplusplus
}
#endif

#endif /* EXPANSE_COMPAT_JUDY_H */
