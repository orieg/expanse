package io.github.orieg.expanse.internal;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.SymbolLookup;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;

/**
 * Native library loader for libexpanse using Java 22+ Project Panama Foreign Function & Memory (FFM) API.
 * Automatically resolves and extracts bundled native libraries across Linux, macOS, and Windows.
 */
public final class NativeLoader {

    private static final String PROPERTY_LIB_PATH = "expanse.library.path";
    private static final String ENV_LIB_PATH = "EXPANSE_LIBRARY_PATH";

    private static volatile SymbolLookup cachedLookup;
    private static volatile Path loadedLibraryPath;

    private NativeLoader() {}

    /**
     * Obtains the {@link SymbolLookup} for libexpanse, loading the native library if not already loaded.
     *
     * @return the SymbolLookup instance bound to the global arena
     */
    public static SymbolLookup getSymbolLookup() {
        SymbolLookup lookup = cachedLookup;
        if (lookup == null) {
            synchronized (NativeLoader.class) {
                lookup = cachedLookup;
                if (lookup == null) {
                    lookup = loadLibrary();
                    cachedLookup = lookup;
                }
            }
        }
        return lookup;
    }

    /**
     * Gets the path to the loaded library file, if loaded from a file path.
     *
     * @return Path to library or null if loaded via system lookup
     */
    public static Path getLoadedLibraryPath() {
        return loadedLibraryPath;
    }

    private static SymbolLookup loadLibrary() {
        List<String> errors = new ArrayList<>();
        OS os = OS.current();
        String libName = os.getLibraryFileName();
        String classifier = os.getClassifier();

        // 1. Check system property override: -Dexpanse.library.path=/path/to/libexpanse.so
        String propPath = System.getProperty(PROPERTY_LIB_PATH);
        if (propPath != null && !propPath.isBlank()) {
            Path p = Path.of(propPath);
            if (Files.isDirectory(p)) {
                p = p.resolve(libName);
            }
            if (Files.exists(p)) {
                try {
                    SymbolLookup lookup = SymbolLookup.libraryLookup(p.toAbsolutePath(), Arena.global());
                    loadedLibraryPath = p.toAbsolutePath();
                    return lookup;
                } catch (Exception e) {
                    errors.add("Failed loading from system property " + PROPERTY_LIB_PATH + " (" + propPath + "): " + e.getMessage());
                }
            } else {
                errors.add("Path specified in " + PROPERTY_LIB_PATH + " does not exist: " + propPath);
            }
        }

        // 2. Check environment variable override: EXPANSE_LIBRARY_PATH
        String envPath = System.getenv(ENV_LIB_PATH);
        if (envPath != null && !envPath.isBlank()) {
            Path p = Path.of(envPath);
            if (Files.isDirectory(p)) {
                p = p.resolve(libName);
            }
            if (Files.exists(p)) {
                try {
                    SymbolLookup lookup = SymbolLookup.libraryLookup(p.toAbsolutePath(), Arena.global());
                    loadedLibraryPath = p.toAbsolutePath();
                    return lookup;
                } catch (Exception e) {
                    errors.add("Failed loading from env var " + ENV_LIB_PATH + " (" + envPath + "): " + e.getMessage());
                }
            } else {
                errors.add("Path specified in " + ENV_LIB_PATH + " does not exist: " + envPath);
            }
        }

        // 3. Check development / build target directories
        String[] devRelativePaths = {
            "target/release/" + libName,
            "target/debug/" + libName,
            "../../target/release/" + libName,
            "../../target/debug/" + libName,
            "../target/release/" + libName,
            "../target/debug/" + libName,
            "crates/expanse-capi/target/release/" + libName,
            "../../crates/expanse-capi/target/release/" + libName
        };

        for (String rel : devRelativePaths) {
            Path p = Path.of(rel);
            if (Files.exists(p)) {
                try {
                    SymbolLookup lookup = SymbolLookup.libraryLookup(p.toAbsolutePath(), Arena.global());
                    loadedLibraryPath = p.toAbsolutePath();
                    return lookup;
                } catch (Exception e) {
                    errors.add("Failed loading from dev path " + p.toAbsolutePath() + ": " + e.getMessage());
                }
            }
        }

        // 4. Check bundled classpath resource: /native/{classifier}/{libraryFileName}
        String resourcePath = "/native/" + classifier + "/" + libName;
        try (InputStream in = NativeLoader.class.getResourceAsStream(resourcePath)) {
            if (in != null) {
                Path tempDir = Files.createTempDirectory("expanse-native-");
                tempDir.toFile().deleteOnExit();
                Path tempLib = tempDir.resolve(libName);
                Files.copy(in, tempLib, StandardCopyOption.REPLACE_EXISTING);
                tempLib.toFile().deleteOnExit();

                SymbolLookup lookup = SymbolLookup.libraryLookup(tempLib.toAbsolutePath(), Arena.global());
                loadedLibraryPath = tempLib.toAbsolutePath();
                return lookup;
            }
        } catch (IOException | IllegalArgumentException e) {
            errors.add("Failed extracting bundled resource " + resourcePath + ": " + e.getMessage());
        }

        // 5. Try system library name lookup (java.library.path / LD_LIBRARY_PATH / DYLD_LIBRARY_PATH)
        try {
            return SymbolLookup.libraryLookup("expanse", Arena.global());
        } catch (IllegalArgumentException e) {
            errors.add("Failed system libraryLookup(\"expanse\"): " + e.getMessage());
        }

        try {
            return SymbolLookup.libraryLookup("libexpanse", Arena.global());
        } catch (IllegalArgumentException e) {
            errors.add("Failed system libraryLookup(\"libexpanse\"): " + e.getMessage());
        }

        // 6. Fail with detailed diagnosis
        StringBuilder sb = new StringBuilder();
        sb.append("Could not load native libexpanse library for platform ").append(classifier).append(".\n");
        sb.append("Attempted the following strategies:\n");
        for (String err : errors) {
            sb.append("  - ").append(err).append("\n");
        }
        sb.append("\nTo provide the native library explicitly, set -D").append(PROPERTY_LIB_PATH)
          .append("=/path/to/").append(libName).append(" or export ").append(ENV_LIB_PATH).append("=/path/to/").append(libName);

        throw new UnsatisfiedLinkError(sb.toString());
    }
}
