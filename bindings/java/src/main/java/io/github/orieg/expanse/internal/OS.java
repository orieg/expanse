package io.github.orieg.expanse.internal;

import java.util.Locale;

/**
 * Operating System and Architecture detection for Expanse native library resolution.
 */
public enum OS {
    LINUX_X86_64("linux-x86_64", "libexpanse.so"),
    LINUX_AARCH64("linux-aarch64", "libexpanse.so"),
    DARWIN_AARCH64("darwin-aarch64", "libexpanse.dylib"),
    DARWIN_X86_64("darwin-x86_64", "libexpanse.dylib"),
    WINDOWS_X86_64("windows-x86_64", "expanse.dll");

    private final String classifier;
    private final String libraryFileName;

    OS(String classifier, String libraryFileName) {
        this.classifier = classifier;
        this.libraryFileName = libraryFileName;
    }

    public String getClassifier() {
        return classifier;
    }

    public String getLibraryFileName() {
        return libraryFileName;
    }

    public static OS current() {
        String osName = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String osArch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);

        boolean isAarch64 = osArch.equals("aarch64") || osArch.equals("arm64");
        boolean isX86_64 = osArch.equals("x86_64") || osArch.equals("amd64") || osArch.equals("x64");

        if (osName.contains("linux")) {
            if (isAarch64) {
                return LINUX_AARCH64;
            } else if (isX86_64) {
                return LINUX_X86_64;
            }
        } else if (osName.contains("mac") || osName.contains("darwin") || osName.contains("os x")) {
            if (isAarch64) {
                return DARWIN_AARCH64;
            } else if (isX86_64) {
                return DARWIN_X86_64;
            }
        } else if (osName.contains("windows")) {
            if (isX86_64) {
                return WINDOWS_X86_64;
            }
        }

        throw new UnsupportedOperationException(
                "Unsupported operating system or architecture: " + osName + " (" + osArch + "). " +
                "Expanse requires a 64-bit platform (Linux x86_64/aarch64, macOS aarch64/x86_64, Windows x86_64)."
        );
    }
}
