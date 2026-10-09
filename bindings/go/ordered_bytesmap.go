package expanse

// ordBytesNavBufLen is the size of the first key buffer a navigation call
// uses. A key longer than it is fetched with one retry at the exact length the
// native call reports, so no key is ever truncated.
const ordBytesNavBufLen = 256

// ordBytesNavRetryFailed reports a native call that still asked for more room
// after it was given the exact length it reported. The native contract makes
// that impossible while the map is not mutated concurrently, so it is a bug in
// the library or in this binding, and it fails loudly rather than returning a
// key that was not read.
const ordBytesNavRetryFailed = "expanse: ordered bytes navigation reported BUFFER_TOO_SMALL after a retry at the exact key length"
