package io.github.orieg.expanse.collections;

import io.github.orieg.expanse.ExpanseMap;
import java.util.AbstractCollection;
import java.util.AbstractMap;
import java.util.AbstractSet;
import java.util.Collection;
import java.util.Comparator;
import java.util.Iterator;
import java.util.Map;
import java.util.NavigableMap;
import java.util.NavigableSet;
import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.Set;
import java.util.SortedMap;
import java.util.SortedSet;

/**
 * Standard {@link NavigableMap} wrapper over an off-heap {@link ExpanseMap}.
 */
public class ExpanseJavaNavigableMap extends AbstractMap<Long, Long> implements NavigableMap<Long, Long> {

    protected final ExpanseMap map;
    protected final Long fromKey;
    protected final boolean fromInclusive;
    protected final Long toKey;
    protected final boolean toInclusive;
    protected final boolean descending;

    public ExpanseJavaNavigableMap(ExpanseMap map) {
        this(map, null, true, null, true, false);
    }

    protected ExpanseJavaNavigableMap(
            ExpanseMap map,
            Long fromKey,
            boolean fromInclusive,
            Long toKey,
            boolean toInclusive,
            boolean descending
    ) {
        this.map = Objects.requireNonNull(map);
        this.fromKey = fromKey;
        this.fromInclusive = fromInclusive;
        this.toKey = toKey;
        this.toInclusive = toInclusive;
        this.descending = descending;
    }

    private boolean inRange(long key) {
        if (fromKey != null) {
            if (fromInclusive ? key < fromKey : key <= fromKey) {
                return false;
            }
        }
        if (toKey != null) {
            if (toInclusive ? key > toKey : key >= toKey) {
                return false;
            }
        }
        return true;
    }

    @Override
    public int size() {
        if (fromKey == null && toKey == null) {
            return (int) Math.min(Integer.MAX_VALUE, map.size());
        }
        long count = 0;
        for (Entry<Long, Long> ignored : entrySet()) {
            count++;
            if (count == Integer.MAX_VALUE) break;
        }
        return (int) count;
    }

    @Override
    public boolean isEmpty() {
        if (fromKey == null && toKey == null) {
            return map.isEmpty();
        }
        return !entrySet().iterator().hasNext();
    }

    @Override
    public boolean containsKey(Object key) {
        if (!(key instanceof Long l)) return false;
        return inRange(l) && map.containsKey(l);
    }

    @Override
    public boolean containsValue(Object value) {
        if (!(value instanceof Long target)) return false;
        for (Entry<Long, Long> e : entrySet()) {
            if (Objects.equals(e.getValue(), target)) return true;
        }
        return false;
    }

    @Override
    public Long get(Object key) {
        if (!(key instanceof Long l)) return null;
        if (!inRange(l)) return null;
        OptionalLong val = map.get(l);
        return val.isPresent() ? val.getAsLong() : null;
    }

    @Override
    public Long put(Long key, Long value) {
        Objects.requireNonNull(key);
        Objects.requireNonNull(value);
        if (!inRange(key)) {
            throw new IllegalArgumentException("Key " + key + " out of map range");
        }
        OptionalLong old = map.putAndGetOld(key, value);
        return old.isPresent() ? old.getAsLong() : null;
    }

    @Override
    public Long remove(Object key) {
        if (!(key instanceof Long l)) return null;
        if (!inRange(l)) return null;
        OptionalLong old = map.removeAndGetOld(l);
        return old.isPresent() ? old.getAsLong() : null;
    }

    @Override
    public void clear() {
        if (fromKey == null && toKey == null) {
            map.clear();
        } else {
            Iterator<Entry<Long, Long>> it = entrySet().iterator();
            while (it.hasNext()) {
                it.next();
                it.remove();
            }
        }
    }

    @Override
    public Comparator<? super Long> comparator() {
        return descending ? Comparator.reverseOrder() : null;
    }

    @Override
    public Long firstKey() {
        if (isEmpty()) throw new NoSuchElementException();
        return firstEntry().getKey();
    }

    @Override
    public Long lastKey() {
        if (isEmpty()) throw new NoSuchElementException();
        return lastEntry().getKey();
    }

    private Entry<Long, Long> mapEntry(ExpanseMap.Entry e) {
        if (e == null || !inRange(e.key())) return null;
        return new SimpleImmutableEntry<>(e.key(), e.value());
    }

    @Override
    public Entry<Long, Long> lowerEntry(Long key) {
        Objects.requireNonNull(key);
        if (!descending) {
            return map.lowerEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            return map.higherEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Long lowerKey(Long key) {
        Entry<Long, Long> e = lowerEntry(key);
        return e == null ? null : e.getKey();
    }

    @Override
    public Entry<Long, Long> floorEntry(Long key) {
        Objects.requireNonNull(key);
        if (!descending) {
            return map.floorEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            return map.ceilingEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Long floorKey(Long key) {
        Entry<Long, Long> e = floorEntry(key);
        return e == null ? null : e.getKey();
    }

    @Override
    public Entry<Long, Long> ceilingEntry(Long key) {
        Objects.requireNonNull(key);
        if (!descending) {
            return map.ceilingEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            return map.floorEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Long ceilingKey(Long key) {
        Entry<Long, Long> e = ceilingEntry(key);
        return e == null ? null : e.getKey();
    }

    @Override
    public Entry<Long, Long> higherEntry(Long key) {
        Objects.requireNonNull(key);
        if (!descending) {
            return map.higherEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            return map.lowerEntry(key).filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Long higherKey(Long key) {
        Entry<Long, Long> e = higherEntry(key);
        return e == null ? null : e.getKey();
    }

    @Override
    public Entry<Long, Long> firstEntry() {
        if (!descending) {
            Optional<ExpanseMap.Entry> opt;
            if (fromKey == null) {
                opt = map.firstEntry();
            } else {
                opt = fromInclusive ? map.ceilingEntry(fromKey) : map.higherEntry(fromKey);
            }
            return opt.filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            Optional<ExpanseMap.Entry> opt;
            if (toKey == null) {
                opt = map.lastEntry();
            } else {
                opt = toInclusive ? map.floorEntry(toKey) : map.lowerEntry(toKey);
            }
            return opt.filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Entry<Long, Long> lastEntry() {
        if (!descending) {
            Optional<ExpanseMap.Entry> opt;
            if (toKey == null) {
                opt = map.lastEntry();
            } else {
                opt = toInclusive ? map.floorEntry(toKey) : map.lowerEntry(toKey);
            }
            return opt.filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        } else {
            Optional<ExpanseMap.Entry> opt;
            if (fromKey == null) {
                opt = map.firstEntry();
            } else {
                opt = fromInclusive ? map.ceilingEntry(fromKey) : map.higherEntry(fromKey);
            }
            return opt.filter(e -> inRange(e.key())).map(this::mapEntry).orElse(null);
        }
    }

    @Override
    public Entry<Long, Long> pollFirstEntry() {
        Entry<Long, Long> e = firstEntry();
        if (e != null) {
            map.remove(e.getKey());
        }
        return e;
    }

    @Override
    public Entry<Long, Long> pollLastEntry() {
        Entry<Long, Long> e = lastEntry();
        if (e != null) {
            map.remove(e.getKey());
        }
        return e;
    }

    @Override
    public NavigableMap<Long, Long> descendingMap() {
        return new ExpanseJavaNavigableMap(map, fromKey, fromInclusive, toKey, toInclusive, !descending);
    }

    @Override
    public NavigableSet<Long> navigableKeySet() {
        return new KeySetView();
    }

    @Override
    public Set<Long> keySet() {
        return navigableKeySet();
    }

    @Override
    public NavigableSet<Long> descendingKeySet() {
        return descendingMap().navigableKeySet();
    }

    @Override
    public Collection<Long> values() {
        return new ValuesView();
    }

    @Override
    public Set<Entry<Long, Long>> entrySet() {
        return new EntrySetView();
    }

    @Override
    public NavigableMap<Long, Long> subMap(Long fromKey, boolean fromInclusive, Long toKey, boolean toInclusive) {
        return new ExpanseJavaNavigableMap(map, fromKey, fromInclusive, toKey, toInclusive, descending);
    }

    @Override
    public NavigableMap<Long, Long> headMap(Long toKey, boolean inclusive) {
        return new ExpanseJavaNavigableMap(map, fromKey, fromInclusive, toKey, inclusive, descending);
    }

    @Override
    public NavigableMap<Long, Long> tailMap(Long fromKey, boolean inclusive) {
        return new ExpanseJavaNavigableMap(map, fromKey, inclusive, toKey, toInclusive, descending);
    }

    @Override
    public SortedMap<Long, Long> subMap(Long fromKey, Long toKey) {
        return subMap(fromKey, true, toKey, false);
    }

    @Override
    public SortedMap<Long, Long> headMap(Long toKey) {
        return headMap(toKey, false);
    }

    @Override
    public SortedMap<Long, Long> tailMap(Long fromKey) {
        return tailMap(fromKey, true);
    }

    private class EntrySetView extends AbstractSet<Entry<Long, Long>> {
        @Override
        public int size() {
            return ExpanseJavaNavigableMap.this.size();
        }

        @Override
        public boolean isEmpty() {
            return ExpanseJavaNavigableMap.this.isEmpty();
        }

        @Override
        public boolean contains(Object o) {
            if (!(o instanceof Entry<?, ?> e)) return false;
            if (!(e.getKey() instanceof Long k && e.getValue() instanceof Long v)) return false;
            Long actual = get(k);
            return actual != null && actual.equals(v);
        }

        @Override
        public boolean remove(Object o) {
            if (!(o instanceof Entry<?, ?> e)) return false;
            if (!(e.getKey() instanceof Long k && e.getValue() instanceof Long v)) return false;
            Long actual = get(k);
            if (actual != null && actual.equals(v)) {
                ExpanseJavaNavigableMap.this.remove(k);
                return true;
            }
            return false;
        }

        @Override
        public void clear() {
            ExpanseJavaNavigableMap.this.clear();
        }

        @Override
        public Iterator<Entry<Long, Long>> iterator() {
            if (!descending) {
                return new ForwardEntryIterator();
            } else {
                return new ReverseEntryIterator();
            }
        }
    }

    private class ForwardEntryIterator implements Iterator<Entry<Long, Long>> {
        private ExpanseMap.Entry nextEntry;
        private ExpanseMap.Entry lastReturned;
        private boolean initialized = false;

        private void advance() {
            if (!initialized) {
                Optional<ExpanseMap.Entry> opt;
                if (fromKey == null) {
                    opt = map.firstEntry();
                } else {
                    opt = fromInclusive ? map.ceilingEntry(fromKey) : map.higherEntry(fromKey);
                }
                nextEntry = opt.filter(e -> inRange(e.key())).orElse(null);
                initialized = true;
            }
        }

        @Override
        public boolean hasNext() {
            advance();
            return nextEntry != null;
        }

        @Override
        public Entry<Long, Long> next() {
            advance();
            if (nextEntry == null) throw new NoSuchElementException();
            lastReturned = nextEntry;
            nextEntry = map.higherEntry(lastReturned.key()).filter(e -> inRange(e.key())).orElse(null);
            return new SimpleImmutableEntry<>(lastReturned.key(), lastReturned.value());
        }

        @Override
        public void remove() {
            if (lastReturned == null) throw new IllegalStateException();
            map.remove(lastReturned.key());
            lastReturned = null;
        }
    }

    private class ReverseEntryIterator implements Iterator<Entry<Long, Long>> {
        private ExpanseMap.Entry nextEntry;
        private ExpanseMap.Entry lastReturned;
        private boolean initialized = false;

        private void advance() {
            if (!initialized) {
                Optional<ExpanseMap.Entry> opt;
                if (toKey == null) {
                    opt = map.lastEntry();
                } else {
                    opt = toInclusive ? map.floorEntry(toKey) : map.lowerEntry(toKey);
                }
                nextEntry = opt.filter(e -> inRange(e.key())).orElse(null);
                initialized = true;
            }
        }

        @Override
        public boolean hasNext() {
            advance();
            return nextEntry != null;
        }

        @Override
        public Entry<Long, Long> next() {
            advance();
            if (nextEntry == null) throw new NoSuchElementException();
            lastReturned = nextEntry;
            nextEntry = map.lowerEntry(lastReturned.key()).filter(e -> inRange(e.key())).orElse(null);
            return new SimpleImmutableEntry<>(lastReturned.key(), lastReturned.value());
        }

        @Override
        public void remove() {
            if (lastReturned == null) throw new IllegalStateException();
            map.remove(lastReturned.key());
            lastReturned = null;
        }
    }

    private class KeySetView extends AbstractSet<Long> implements NavigableSet<Long> {
        @Override
        public int size() {
            return ExpanseJavaNavigableMap.this.size();
        }

        @Override
        public boolean isEmpty() {
            return ExpanseJavaNavigableMap.this.isEmpty();
        }

        @Override
        public boolean contains(Object o) {
            return containsKey(o);
        }

        @Override
        public boolean remove(Object o) {
            return ExpanseJavaNavigableMap.this.remove(o) != null;
        }

        @Override
        public void clear() {
            ExpanseJavaNavigableMap.this.clear();
        }

        @Override
        public Iterator<Long> iterator() {
            Iterator<Entry<Long, Long>> it = entrySet().iterator();
            return new Iterator<>() {
                @Override
                public boolean hasNext() {
                    return it.hasNext();
                }

                @Override
                public Long next() {
                    return it.next().getKey();
                }

                @Override
                public void remove() {
                    it.remove();
                }
            };
        }

        @Override
        public Comparator<? super Long> comparator() {
            return ExpanseJavaNavigableMap.this.comparator();
        }

        @Override
        public Long first() {
            return firstKey();
        }

        @Override
        public Long last() {
            return lastKey();
        }

        @Override
        public Long lower(Long e) {
            return lowerKey(e);
        }

        @Override
        public Long floor(Long e) {
            return floorKey(e);
        }

        @Override
        public Long ceiling(Long e) {
            return ceilingKey(e);
        }

        @Override
        public Long higher(Long e) {
            return higherKey(e);
        }

        @Override
        public Long pollFirst() {
            Entry<Long, Long> e = pollFirstEntry();
            return e == null ? null : e.getKey();
        }

        @Override
        public Long pollLast() {
            Entry<Long, Long> e = pollLastEntry();
            return e == null ? null : e.getKey();
        }

        @Override
        public NavigableSet<Long> descendingSet() {
            return descendingMap().navigableKeySet();
        }

        @Override
        public Iterator<Long> descendingIterator() {
            return descendingMap().navigableKeySet().iterator();
        }

        @Override
        public NavigableSet<Long> subSet(Long fromElement, boolean fromInclusive, Long toElement, boolean toInclusive) {
            return subMap(fromElement, fromInclusive, toElement, toInclusive).navigableKeySet();
        }

        @Override
        public NavigableSet<Long> headSet(Long toElement, boolean inclusive) {
            return headMap(toElement, inclusive).navigableKeySet();
        }

        @Override
        public NavigableSet<Long> tailSet(Long fromElement, boolean inclusive) {
            return tailMap(fromElement, inclusive).navigableKeySet();
        }

        @Override
        public SortedSet<Long> subSet(Long fromElement, Long toElement) {
            return subSet(fromElement, true, toElement, false);
        }

        @Override
        public SortedSet<Long> headSet(Long toElement) {
            return headSet(toElement, false);
        }

        @Override
        public SortedSet<Long> tailSet(Long fromElement) {
            return tailSet(fromElement, true);
        }
    }

    private class ValuesView extends AbstractCollection<Long> {
        @Override
        public int size() {
            return ExpanseJavaNavigableMap.this.size();
        }

        @Override
        public boolean isEmpty() {
            return ExpanseJavaNavigableMap.this.isEmpty();
        }

        @Override
        public boolean contains(Object o) {
            return containsValue(o);
        }

        @Override
        public Iterator<Long> iterator() {
            Iterator<Entry<Long, Long>> it = entrySet().iterator();
            return new Iterator<>() {
                @Override
                public boolean hasNext() {
                    return it.hasNext();
                }

                @Override
                public Long next() {
                    return it.next().getValue();
                }

                @Override
                public void remove() {
                    it.remove();
                }
            };
        }

        @Override
        public void clear() {
            ExpanseJavaNavigableMap.this.clear();
        }
    }
}
