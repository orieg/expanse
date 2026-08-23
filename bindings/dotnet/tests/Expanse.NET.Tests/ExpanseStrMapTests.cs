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
}
