package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.MemorySegment;
import java.nio.charset.StandardCharsets;

/**
 * Top-level entry point and version information for Expanse Java bindings.
 */
public final class Expanse {

    private static final String VERSION;

    static {
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_version.invokeExact();
            VERSION = ptr.reinterpret(1024).getString(0, StandardCharsets.UTF_8);
        } catch (Throwable t) {
            throw new ExceptionInInitializerError(t);
        }
    }

    private Expanse() {}

    /**
     * Returns the native libexpanse library version (e.g. "0.3.0").
     *
     * @return semantic version string
     */
    public static String version() {
        return VERSION;
    }
}
