# Expanse Ruby Bindings (`expanse`)

High-performance, pure-Rust Judy arrays and digital trie engine for Ruby via FFI (`Fiddle`).

## Installation

Add to your `Gemfile`:
```ruby
gem "expanse"
```

Or install directly:
```bash
gem install expanse
```

## Quickstart

```ruby
require "expanse"

# 1. Dynamic sparse 64-bit integer set (Judy1)
set = Expanse::Set.new
set.add(42)
set.add(100_000)
puts set.include?(42)     # true
puts set.rank(100_000)    # O(depth) rank
puts set.first            # 42
puts set.last             # 100000

# 2. Ordered 64-bit key-value map (JudyL)
map = Expanse::Map.new
map[42] = 1000
map[100] = 5000
puts map[42]              # 1000
map.each do |k, v|
  puts "Key #{k} -> #{v}"
end

# 3. String trie (JudySL)
strmap = Expanse::StrMap.new
strmap["/api/v1/users"] = 200
puts strmap["/api/v1/users"] # 200

# 4. Arbitrary binary bytes map (JudyHS)
bytesmap = Expanse::BytesMap.new
bytesmap["binary\x00key".b] = 9999
puts bytesmap["binary\x00key".b] # 9999

# 5. Large-value off-heap blob map
blobmap = Expanse::BlobMap.new
blobmap.set(100, "hello world", hot_meta: 1234)
val, meta = blobmap.get(100)
puts "#{val} (meta: #{meta})"
```

## Documentation

For complete API specifications and benchmarks, see [docs/BINDINGS_RUBY.md](../../docs/BINDINGS_RUBY.md).
