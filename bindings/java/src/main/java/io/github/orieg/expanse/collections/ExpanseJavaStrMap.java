package io.github.orieg.expanse.collections;

import io.github.orieg.expanse.ExpanseStrMap;
import java.util.AbstractMap;
import java.util.AbstractSet;
import java.util.Iterator;
import java.util.Map;
import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;

/**
 * Standard {@link Map} wrapper over an off-heap {@link ExpanseStrMap}.
 */
public class ExpanseJavaStrMap extends AbstractMap<String, Long> {

    private final ExpanseStrMap map;

    public ExpanseJavaStrMap(ExpanseStrMap map) {
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
        return key instanceof String s && map.containsKey(s);
    }

    @Override
    public Long get(Object key) {
        if (!(key instanceof String s)) return null;
        OptionalLong val = map.get(s);
        return val.isPresent() ? val.getAsLong() : null;
    }

    @Override
    public Long put(String key, Long value) {
        Objects.requireNonNull(key);
        Objects.requireNonNull(value);
        OptionalLong old = map.putAndGetOld(key, value);
        return old.isPresent() ? old.getAsLong() : null;
    }

    @Override
    public Long remove(Object key) {
        if (!(key instanceof String s)) return null;
        OptionalLong old = map.removeAndGetOld(s);
        return old.isPresent() ? old.getAsLong() : null;
    }

    @Override
    public void clear() {
        map.clear();
    }

    @Override
    public Set<Entry<String, Long>> entrySet() {
        return new EntrySetView();
    }

    private class EntrySetView extends AbstractSet<Entry<String, Long>> {
        @Override
        public int size() {
            return ExpanseJavaStrMap.this.size();
        }

        @Override
        public Iterator<Entry<String, Long>> iterator() {
            return new Iterator<>() {
                private ExpanseStrMap.Entry nextEntry;
                private ExpanseStrMap.Entry lastReturned;
                private boolean initialized = false;

                private void advance() {
                    if (!initialized) {
                        nextEntry = map.firstEntry().orElse(null);
                        initialized = true;
                    }
                }

                @Override
                public boolean hasNext() {
                    advance();
                    return nextEntry != null;
                }

                @Override
                public Entry<String, Long> next() {
                    advance();
                    if (nextEntry == null) throw new NoSuchElementException();
                    lastReturned = nextEntry;
                    nextEntry = map.nextAfter(lastReturned.key()).orElse(null);
                    return new SimpleImmutableEntry<>(lastReturned.key(), lastReturned.value());
                }

                @Override
                public void remove() {
                    if (lastReturned == null) throw new IllegalStateException();
                    map.remove(lastReturned.key());
                    lastReturned = null;
                }
            };
        }
    }
}
