using System;
using System.Collections.Generic;
using System.Linq;
using Xunit;

namespace Expanse.Tests;

public class ExpanseOrderedBytesMapTests
{
    // Unsigned lexicographic order: a shorter key sorts before a longer key it prefixes.
    private static readonly byte[][] Sorted =
    [
        [],
        [0x00],
        [0x00, 0x00],
        [0x01],
        [0x01, 0x00],
        [0xFF],
    ];

    private static void InsertSorted(ExpanseOrderedBytesMap map)
    {
        // Insert in reverse so the native order is not the insertion order.
        for (int i = Sorted.Length - 1; i >= 0; i--)
        {
            Assert.True(map.Insert(Sorted[i], (ulong)i, out _));
        }
    }

    [Fact]
    public void RoundTripGetContainsRemove()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] key = [0x61, 0x62, 0x63];

        map.Set(key, 42);

        Assert.True(map.TryGet(key, out ulong value));
        Assert.Equal(42UL, value);
        Assert.True(map.ContainsKey(key));
        Assert.Equal(1UL, map.LongCount);
        Assert.Equal(1, map.Count);

        Assert.True(map.Remove(key));
        Assert.False(map.ContainsKey(key));
        Assert.False(map.Remove(key));
        Assert.True(map.IsEmpty);
    }

    [Fact]
    public void InsertReportsNewAndReplaced()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] key = [0x01, 0x02];

        Assert.True(map.Insert(key, 1, out ulong old));
        Assert.Equal(0UL, old);

        Assert.False(map.Insert(key, 2, out old));
        Assert.Equal(1UL, old);

        Assert.True(map.Remove(key, out ulong removed));
        Assert.Equal(2UL, removed);
    }

    [Fact]
    public void IndexerGetSetAndMissingKey()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] key = [0x09];

        map[key] = 3;
        Assert.Equal(3UL, map[key]);
        Assert.Throws<KeyNotFoundException>(() => map[new byte[] { 0x0A }]);
    }

    [Fact]
    public void OrderingAndNavigation()
    {
        using var map = new ExpanseOrderedBytesMap();
        InsertSorted(map);

        Assert.Equal(Sorted.Length, map.Count);

        var first = map.FirstEntry();
        Assert.NotNull(first);
        Assert.Equal(Sorted[0], first!.Key);
        Assert.Equal(0UL, first.Value);

        var last = map.LastEntry();
        Assert.NotNull(last);
        Assert.Equal(Sorted[^1], last!.Key);
        Assert.Equal((ulong)(Sorted.Length - 1), last.Value);

        // Ceiling: smallest key >= search key.
        Assert.Equal(new byte[] { 0x01 }, map.CeilingEntry(new byte[] { 0x00, 0x01 })!.Key);
        Assert.Equal(new byte[] { 0x01 }, map.CeilingEntry(new byte[] { 0x01 })!.Key);

        // Higher: smallest key > search key.
        Assert.Equal(new byte[] { 0x01, 0x00 }, map.HigherEntry(new byte[] { 0x01 })!.Key);

        // Floor: largest key <= search key.
        Assert.Equal(new byte[] { 0x01, 0x00 }, map.FloorEntry(new byte[] { 0x02 })!.Key);
        Assert.Equal(new byte[] { 0x01 }, map.FloorEntry(new byte[] { 0x01 })!.Key);
        Assert.Equal(new byte[] { 0x00 }, map.FloorEntry(new byte[] { 0x00 })!.Key);

        // Lower: largest key < search key.
        Assert.Equal(new byte[] { 0x00, 0x00 }, map.LowerEntry(new byte[] { 0x01 })!.Key);

        // Out-of-range navigation is absent, not an empty entry.
        Assert.Null(map.LowerEntry(Array.Empty<byte>()));
        Assert.Null(map.HigherEntry(new byte[] { 0xFF }));
        Assert.Null(map.CeilingEntry(new byte[] { 0xFF, 0x00 }));
    }

    [Fact]
    public void UnsignedByteOrder()
    {
        using var map = new ExpanseOrderedBytesMap();
        map.Set(new byte[] { 0x80 }, 1);
        map.Set(new byte[] { 0x7F }, 2);

        Assert.Equal(new byte[] { 0x7F }, map.FirstEntry()!.Key);
        Assert.Equal(new byte[] { 0x80 }, map.LastEntry()!.Key);
    }

    [Fact]
    public void EmptyKeyIsValidAndSortsFirst()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] emptyKey = [];

        map.Set(emptyKey, 7);

        Assert.True(map.TryGet(emptyKey, out ulong value));
        Assert.Equal(7UL, value);
        Assert.True(map.ContainsKey(emptyKey));

        var first = map.FirstEntry();
        Assert.NotNull(first);
        Assert.Empty(first!.Key);
        Assert.Equal(7UL, first.Value);

        // The empty key is the lower neighbour of every non-empty key.
        Assert.Empty(map.LowerEntry(new byte[] { 0x00 })!.Key);

        Assert.True(map.Remove(emptyKey));
        Assert.False(map.ContainsKey(emptyKey));
    }

    [Fact]
    public void ZeroAndFFBytesAreOrdinaryKeyBytes()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] zero = [0x00, 0x00, 0x00];
        byte[] ff = [0xFF, 0xFF, 0xFF];

        map.Set(ff, 2);
        map.Set(zero, 1);

        Assert.True(map.TryGet(zero, out ulong z));
        Assert.Equal(1UL, z);
        Assert.True(map.TryGet(ff, out ulong f));
        Assert.Equal(2UL, f);
        Assert.Equal(zero, map.FirstEntry()!.Key);
        Assert.Equal(ff, map.LastEntry()!.Key);
    }

    [Fact]
    public void KeyLongerThanEightAndInitialNavigationBuffer()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] shortKey = [0x01];
        byte[] longKey = Enumerable.Repeat((byte)0xFE, 1000).ToArray();
        byte[] longPrefix = longKey[..999];

        map.Set(shortKey, 1);
        map.Set(longKey, 99);

        Assert.True(map.TryGet(longKey, out ulong value));
        Assert.Equal(99UL, value);

        // The 1000-byte key is larger than the 256-byte first navigation buffer.
        var higher = map.HigherEntry(shortKey);
        Assert.NotNull(higher);
        Assert.Equal(longKey, higher!.Key);
        Assert.Equal(99UL, higher.Value);

        Assert.Equal(longKey, map.LastEntry()!.Key);
        Assert.Equal(longKey, map.CeilingEntry(longPrefix)!.Key);
        Assert.Equal(shortKey, map.FloorEntry(longPrefix)!.Key);

        // A 9-byte key round-trips past the 8-byte boundary.
        byte[] nineBytes = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        map.Set(nineBytes, 9);
        Assert.Equal(nineBytes, map.FloorEntry(nineBytes)!.Key);
    }

    [Fact]
    public void AbsentKeysAndEmptyMap()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] key = [0x10];

        Assert.False(map.TryGet(key, out ulong value));
        Assert.Equal(0UL, value);
        Assert.False(map.ContainsKey(key));
        Assert.False(map.TrySlot(key, out _));

        Assert.Null(map.FirstEntry());
        Assert.Null(map.LastEntry());
        Assert.Null(map.CeilingEntry(key));
        Assert.Null(map.FloorEntry(key));
    }

    [Fact]
    public void IterationObservesMutationBetweenSteps()
    {
        using var map = new ExpanseOrderedBytesMap();
        InsertSorted(map);

        var visited = new List<byte[]>();
        map.ForEach((k, _) => visited.Add(k));
        Assert.Equal(Sorted, visited);

        // Remove the key that follows the first entry; the next step must skip it.
        var first = map.FirstEntry()!;
        Assert.Equal(new byte[] { 0x00 }, map.HigherEntry(first.Key)!.Key);
        Assert.True(map.Remove(new byte[] { 0x00 }));
        Assert.Equal(new byte[] { 0x00, 0x00 }, map.HigherEntry(first.Key)!.Key);

        var afterMutation = new List<byte[]>();
        map.ForEach((k, _) => afterMutation.Add(k));
        Assert.Equal(Sorted.Where(k => !k.SequenceEqual(new byte[] { 0x00 })), afterMutation);
    }

    [Fact]
    public void SlotsReadAndWriteTheStoredValue()
    {
        using var map = new ExpanseOrderedBytesMap();
        byte[] key = [0x10, 0x00];

        Span<ulong> created = map.InsertSlot(key);
        Assert.Equal(0UL, created[0]);
        created[0] = 5;
        Assert.True(map.TryGet(key, out ulong value));
        Assert.Equal(5UL, value);

        // Inserting an existing key keeps its value.
        map.Set(key, 9);
        Assert.Equal(9UL, map.InsertSlot(key)[0]);

        Assert.True(map.TrySlot(key, out Span<ulong> slot));
        slot[0] = 11;
        Assert.True(map.TryGet(key, out value));
        Assert.Equal(11UL, value);

        // The empty key has a slot too.
        Span<ulong> emptySlot = map.InsertSlot(Array.Empty<byte>());
        emptySlot[0] = 13;
        Assert.True(map.TryGet(Array.Empty<byte>(), out value));
        Assert.Equal(13UL, value);
    }

    [Fact]
    public void MemoryAccountingAndClear()
    {
        using var map = new ExpanseOrderedBytesMap();
        InsertSorted(map);

        Assert.True(map.MemoryHeld >= map.MemoryUsed);

        map.Clear();
        Assert.Equal(0UL, map.LongCount);
        Assert.True(map.IsEmpty);
        Assert.Null(map.FirstEntry());
    }

    [Fact]
    public void UseAfterDisposeThrows()
    {
        var map = new ExpanseOrderedBytesMap();
        map.Set(new byte[] { 0x01 }, 1);
        map.Dispose();
        map.Dispose();

        Assert.Throws<ObjectDisposedException>(() => map.Set(new byte[] { 0x01 }, 2));
        Assert.Throws<ObjectDisposedException>(() => map.FirstEntry());
    }
}
