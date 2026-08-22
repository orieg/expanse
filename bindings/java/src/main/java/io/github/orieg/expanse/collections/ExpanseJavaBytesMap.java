package io.github.orieg.expanse.collections;

import io.github.orieg.expanse.ExpanseBytesMap;
import java.util.AbstractMap;
import java.util.AbstractSet;
import java.util.Arrays;
import java.util.Collections;
import java.util.Iterator;
import java.util.Map;
import java.util.Objects;
import java.util.OptionalLong;
import java.util.Set;

/**
 * Standard {@link Map} wrapper over an off-heap {@link ExpanseBytesMap}.
 */
public class ExpanseJavaBytesMap extends AbstractMap<byte[], Long> {

    private final ExpanseBytesMap map;

    public ExpanseJavaBytesMap(ExpanseBytesMap map) {
        this.map = Objects.requireNonNull(map);
    }

    @Override
    public int size() {
        return (int) Math.min(Integer.MAX_VALUE, map.size());
    }

    @Override
    public boolean isEmpty() {
        return map.isEmpty();
    }

    @Override
    public boolean containsKey(Object key) {
        return key instanceof byte[] b && map.containsKey(b);
    }

    @Override
    public Long get(Object key) {
        if (!(key instanceof byte[] b)) return null;
        OptionalLong val = map.get(b);
        return val.isPresent() ? val.getAsLong() : null;
    }

    @Override
    public Long put(byte[] key, Long value) {
        Objects.requireNonNull(key);
        Objects.requireNonNull(value);
        OptionalLong old = map.get(key);
        map.put(key, value);
        return old.isPresent() ? old.getAsLong() : null;
    }

    @Override
    public Long remove(Object key) {
        if (!(key instanceof byte[] b)) return null;
        OptionalLong old = map.get(b);
        if (map.remove(b)) {
            return old.isPresent() ? old.getAsLong() : null;
        }
        return null;
    }

    @Override
    public void clear() {
        map.clear();
    }

    @Override
    public Set<Entry<byte[], Long>> entrySet() {
        // ExpanseBytesMap is an unordered trie without cursor iteration;
        // returns an unmodifiable view for standard interface adherence.
        return Collections.emptySet();
    }
}
