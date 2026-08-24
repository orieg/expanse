package io.github.orieg.expanse.collections;

import io.github.orieg.expanse.ExpanseSet;
import java.util.AbstractSet;
import java.util.Comparator;
import java.util.Iterator;
import java.util.NavigableSet;
import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.OptionalLong;
import java.util.PrimitiveIterator;
import java.util.SortedSet;

/**
 * Standard {@link NavigableSet} wrapper over an off-heap {@link ExpanseSet}.
 */
public class ExpanseJavaNavigableSet extends AbstractSet<Long> implements NavigableSet<Long> {

    protected final ExpanseSet set;
    protected final Long fromElement;
    protected final boolean fromInclusive;
    protected final Long toElement;
    protected final boolean toInclusive;
    protected final boolean descending;

    public ExpanseJavaNavigableSet(ExpanseSet set) {
        this(set, null, true, null, true, false);
    }

    protected ExpanseJavaNavigableSet(
            ExpanseSet set,
            Long fromElement,
            boolean fromInclusive,
            Long toElement,
            boolean toInclusive,
            boolean descending
    ) {
        this.set = Objects.requireNonNull(set);
        this.fromElement = fromElement;
        this.fromInclusive = fromInclusive;
        this.toElement = toElement;
        this.toInclusive = toInclusive;
        this.descending = descending;
    }

    private boolean inRange(long key) {
        // Keys are ordered as unsigned 64-bit integers by the native trie, so all
        // boundary comparisons must use Long.compareUnsigned. Signed < / > would
        // misplace every key >= 2^63 (e.g. -1L is the LARGEST key, not the smallest).
        if (fromElement != null) {
            int c = Long.compareUnsigned(key, fromElement);
            if (fromInclusive ? c < 0 : c <= 0) {
                return false;
            }
        }
        if (toElement != null) {
            int c = Long.compareUnsigned(key, toElement);
            if (toInclusive ? c > 0 : c >= 0) {
                return false;
            }
        }
        return true;
    }

    @Override
    public int size() {
        if (fromElement == null && toElement == null) {
            return (int) Math.min(Integer.MAX_VALUE, set.size());
        }
        long count = 0;
        for (long ignored : this) {
            count++;
            if (count == Integer.MAX_VALUE) break;
        }
        return (int) count;
    }

    @Override
    public boolean isEmpty() {
        if (fromElement == null && toElement == null) {
            return set.isEmpty();
        }
        return !iterator().hasNext();
    }

    @Override
    public boolean contains(Object o) {
        if (!(o instanceof Long l)) {
            return false;
        }
        return inRange(l) && set.contains(l);
    }

    @Override
    public boolean add(Long e) {
        Objects.requireNonNull(e);
        if (!inRange(e)) {
            throw new IllegalArgumentException("Key " + e + " out of range");
        }
        return set.add(e);
    }

    @Override
    public boolean remove(Object o) {
        if (!(o instanceof Long l)) {
            return false;
        }
        return inRange(l) && set.remove(l);
    }

    @Override
    public void clear() {
        if (fromElement == null && toElement == null) {
            set.clear();
        } else {
            Iterator<Long> it = iterator();
            while (it.hasNext()) {
                it.next();
                it.remove();
            }
        }
    }

    @Override
    public Comparator<? super Long> comparator() {
        // Unsigned order: must NOT return null (null implies natural signed order,
        // which is wrong for keys >= 2^63). Return the unsigned comparator, reversed
        // for descending views.
        Comparator<Long> cmp = Long::compareUnsigned;
        return descending ? cmp.reversed() : cmp;
    }

    @Override
    public Long first() {
        if (isEmpty()) throw new NoSuchElementException();
        return iterator().next();
    }

    @Override
    public Long last() {
        if (isEmpty()) throw new NoSuchElementException();
        return descendingIterator().next();
    }

    @Override
    public Long lower(Long e) {
        Objects.requireNonNull(e);
        if (!descending) {
            OptionalLong opt = set.lower(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        } else {
            OptionalLong opt = set.higher(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        }
    }

    @Override
    public Long floor(Long e) {
        Objects.requireNonNull(e);
        if (!descending) {
            OptionalLong opt = set.floor(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        } else {
            OptionalLong opt = set.ceiling(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        }
    }

    @Override
    public Long ceiling(Long e) {
        Objects.requireNonNull(e);
        if (!descending) {
            OptionalLong opt = set.ceiling(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        } else {
            OptionalLong opt = set.floor(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        }
    }

    @Override
    public Long higher(Long e) {
        Objects.requireNonNull(e);
        if (!descending) {
            OptionalLong opt = set.higher(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        } else {
            OptionalLong opt = set.lower(e);
            return (opt.isPresent() && inRange(opt.getAsLong())) ? opt.getAsLong() : null;
        }
    }

    @Override
    public Long pollFirst() {
        Iterator<Long> it = iterator();
        if (it.hasNext()) {
            Long val = it.next();
            it.remove();
            return val;
        }
        return null;
    }

    @Override
    public Long pollLast() {
        Iterator<Long> it = descendingIterator();
        if (it.hasNext()) {
            Long val = it.next();
            it.remove();
            return val;
        }
        return null;
    }

    @Override
    public Iterator<Long> iterator() {
        if (!descending) {
            return new ForwardIterator();
        } else {
            return new ReverseIterator();
        }
    }

    @Override
    public Iterator<Long> descendingIterator() {
        if (!descending) {
            return new ReverseIterator();
        } else {
            return new ForwardIterator();
        }
    }

    @Override
    public NavigableSet<Long> descendingSet() {
        return new ExpanseJavaNavigableSet(set, fromElement, fromInclusive, toElement, toInclusive, !descending);
    }

    @Override
    public NavigableSet<Long> subSet(Long fromElement, boolean fromInclusive, Long toElement, boolean toInclusive) {
        return new ExpanseJavaNavigableSet(set, fromElement, fromInclusive, toElement, toInclusive, descending);
    }

    @Override
    public NavigableSet<Long> headSet(Long toElement, boolean inclusive) {
        return new ExpanseJavaNavigableSet(set, fromElement, fromInclusive, toElement, inclusive, descending);
    }

    @Override
    public NavigableSet<Long> tailSet(Long fromElement, boolean inclusive) {
        return new ExpanseJavaNavigableSet(set, fromElement, inclusive, toElement, toInclusive, descending);
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

    private class ForwardIterator implements Iterator<Long> {
        private Long nextVal;
        private Long lastReturned;
        private boolean initialized = false;

        private void advance() {
            if (!initialized) {
                OptionalLong opt;
                if (fromElement == null) {
                    opt = set.first();
                } else {
                    opt = fromInclusive ? set.ceiling(fromElement) : set.higher(fromElement);
                }
                if (opt.isPresent() && inRange(opt.getAsLong())) {
                    nextVal = opt.getAsLong();
                } else {
                    nextVal = null;
                }
                initialized = true;
            }
        }

        @Override
        public boolean hasNext() {
            advance();
            return nextVal != null;
        }

        @Override
        public Long next() {
            advance();
            if (nextVal == null) throw new NoSuchElementException();
            lastReturned = nextVal;
            OptionalLong opt = set.higher(lastReturned);
            if (opt.isPresent() && inRange(opt.getAsLong())) {
                nextVal = opt.getAsLong();
            } else {
                nextVal = null;
            }
            return lastReturned;
        }

        @Override
        public void remove() {
            if (lastReturned == null) throw new IllegalStateException();
            set.remove(lastReturned);
            lastReturned = null;
        }
    }

    private class ReverseIterator implements Iterator<Long> {
        private Long nextVal;
        private Long lastReturned;
        private boolean initialized = false;

        private void advance() {
            if (!initialized) {
                OptionalLong opt;
                if (toElement == null) {
                    opt = set.last();
                } else {
                    opt = toInclusive ? set.floor(toElement) : set.lower(toElement);
                }
                if (opt.isPresent() && inRange(opt.getAsLong())) {
                    nextVal = opt.getAsLong();
                } else {
                    nextVal = null;
                }
                initialized = true;
            }
        }

        @Override
        public boolean hasNext() {
            advance();
            return nextVal != null;
        }

        @Override
        public Long next() {
            advance();
            if (nextVal == null) throw new NoSuchElementException();
            lastReturned = nextVal;
            OptionalLong opt = set.lower(lastReturned);
            if (opt.isPresent() && inRange(opt.getAsLong())) {
                nextVal = opt.getAsLong();
            } else {
                nextVal = null;
            }
            return lastReturned;
        }

        @Override
        public void remove() {
            if (lastReturned == null) throw new IllegalStateException();
            set.remove(lastReturned);
            lastReturned = null;
        }
    }
}
