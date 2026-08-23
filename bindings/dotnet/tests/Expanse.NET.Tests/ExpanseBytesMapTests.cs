using System;
using System.Collections.Generic;
using System.Text;
using Xunit;

namespace Expanse.Tests;

public class ExpanseBytesMapTests
{
    [Fact]
    public void BinaryKeysWithEmbeddedNul()
    {
        using var map = new ExpanseBytesMap();

        byte[] key1 = [0x01, 0x00, 0x02, 0x00, 0x03];
        byte[] key2 = [0x01, 0x00, 0x02, 0x00, 0x04];
        byte[] emptyKey = [];

        map.Set(key1, 100);
        map.Set(key2, 200);
        map.Set(emptyKey, 300);

        Assert.Equal(3, map.Count);

        Assert.True(map.TryGet(key1, out ulong val1));
        Assert.Equal(100UL, val1);

        Assert.True(map.TryGet(key2, out ulong val2));
        Assert.Equal(200UL, val2);

        Assert.True(map.TryGet(emptyKey, out ulong valEmpty));
        Assert.Equal(300UL, valEmpty);

        Assert.True(map.ContainsKey(key1));
        Assert.True(map.ContainsKey(key2));
        Assert.True(map.ContainsKey(emptyKey));

        // Modify
        map.Set(key1, 999);
        Assert.True(map.TryGet(key1, out ulong valModified));
        Assert.Equal(999UL, valModified);

        // Remove
        Assert.True(map.Remove(key1));
        Assert.False(map.ContainsKey(key1));
        Assert.Equal(2, map.Count);

        map.Clear();
        Assert.Equal(0, map.Count);
        Assert.True(map.IsEmpty);
    }

    [Fact]
    public void ArbitraryBinaryPayloadKeys()
    {
        using var map = new ExpanseBytesMap();

        for (int i = 0; i < 100; i++)
        {
            byte[] key = new byte[32];
            Random.Shared.NextBytes(key);
            key[0] = (byte)i; // Distinct keys
            map.Set(key, (ulong)(i * 10));
            Assert.True(map.TryGet(key, out ulong val));
            Assert.Equal((ulong)(i * 10), val);
        }

        Assert.Equal(100, map.Count);
    }
}
