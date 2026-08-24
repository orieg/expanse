using System;
using System.Collections.Generic;
using System.Linq;
using Xunit;

namespace Expanse.Tests;

public class ExpanseStrMapTests
{
    [Fact]
    public void BasicStringCrudAndSpan()
    {
        using var map = new ExpanseStrMap();
        Assert.Equal(0, map.Count);

        map["alpha"] = 1;
        map["beta".AsSpan()] = 2;
        map["gamma"] = 3;

        Assert.Equal(3, map.Count);
        Assert.True(map.ContainsKey("alpha"));
        Assert.True(map.ContainsKey("beta".AsSpan()));
        Assert.False(map.ContainsKey("delta"));

        Assert.Equal(1UL, map["alpha"]);
        Assert.Equal(2UL, map["beta".AsSpan()]);
        Assert.Equal(3UL, map["gamma"]);

        // Remove
        Assert.True(map.Remove("beta"));
        Assert.False(map.ContainsKey("beta"));
        Assert.Equal(2, map.Count);

        map.Clear();
        Assert.Equal(0, map.Count);
        Assert.True(map.IsEmpty);
    }

    [Fact]
    public void LexicographicalNavigation()
    {
        using var map = new ExpanseStrMap();
        map["apple"] = 10;
        map["banana"] = 20;
        map["cherry"] = 30;
        map["date"] = 40;

        Assert.Equal(new KeyValuePair<string, ulong>("apple", 10), map.First());
        Assert.Equal(new KeyValuePair<string, ulong>("date", 40), map.Last());

        Assert.Equal(new KeyValuePair<string, ulong>("banana", 20), map.Next("apple"));
        Assert.Equal(new KeyValuePair<string, ulong>("banana", 20), map.NextAtOrAfter("b"));
        Assert.Equal(new KeyValuePair<string, ulong>("cherry", 30), map.Prev("date"));
        Assert.Equal(new KeyValuePair<string, ulong>("cherry", 30), map.PrevAtOrBefore("czz"));

        var items = map.ToList();
        Assert.Equal(4, items.Count);
        Assert.Equal("apple", items[0].Key);
        Assert.Equal("banana", items[1].Key);
        Assert.Equal("cherry", items[2].Key);
        Assert.Equal("date", items[3].Key);
    }

    [Fact]
    public void NavigationReturnsKeysLongerThanTheScratchBuffer()
    {
        // The pre-fix nav used a fixed 4096-byte buffer and treated the native
        // 'false' (buffer-too-small) as 'no more keys', so a key >= 4096 bytes made
        // the whole map look empty from First()/Last()/Next()/Prev(). The _ex-based
        // retry loop must grow the buffer and return the full key.
        using var map = new ExpanseStrMap();
        string shortKey = "aaa";
        string longKey = new string('b', 10_000);    // ~10 KiB, far past the 4 KiB default
        string longerKey = new string('c', 20_000);
        map[shortKey] = 1;
        map[longKey] = 2;
        map[longerKey] = 3;

        // First/Last must see the long keys, not report empty ('a' < 'b' < 'c').
        Assert.Equal(new KeyValuePair<string, ulong>(shortKey, 1), map.First());
        var last = map.Last();
        Assert.NotNull(last);
        Assert.Equal(longerKey, last!.Value.Key);
        Assert.Equal(20_000, last.Value.Key.Length);
        Assert.Equal(3ul, last.Value.Value);

        // Forward navigation must step INTO and OUT OF the 10 KiB key.
        var afterShort = map.Next(shortKey);
        Assert.NotNull(afterShort);
        Assert.Equal(longKey, afterShort!.Value.Key);
        Assert.Equal(10_000, afterShort.Value.Key.Length);
        Assert.Equal(longerKey, map.Next(longKey)!.Value.Key);

        // Reverse navigation likewise.
        Assert.Equal(longKey, map.Prev(longerKey)!.Value.Key);

        // Full enumeration must visit all three keys in order.
        var keys = map.Select(kv => kv.Key).ToList();
        Assert.Equal(new[] { shortKey, longKey, longerKey }, keys);
    }
}
