package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.NativeLoader;
import io.github.orieg.expanse.internal.OS;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.lang.foreign.SymbolLookup;

import static org.junit.jupiter.api.Assertions.*;

class NativeLoaderTest {

    @Test
    @DisplayName("OS detection and classifier")
    void osDetection() {
        OS os = OS.current();
        assertNotNull(os);
        assertNotNull(os.getClassifier());
        assertNotNull(os.getLibraryFileName());
    }

    @Test
    @DisplayName("SymbolLookup successfully loaded and version available")
    void libraryLoaded() {
        SymbolLookup lookup = NativeLoader.getSymbolLookup();
        assertNotNull(lookup);

        String version = Expanse.version();
        assertNotNull(version);
        assertFalse(version.isBlank());
        assertTrue(version.startsWith("0.") || version.startsWith("1."), "Version should start with semver major");
    }
}
