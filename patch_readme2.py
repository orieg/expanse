with open('README.md', 'r') as f:
    content = f.read()

target = "| **Node.js / Bun / Deno API** | [`crates/expanse-node`](crates/expanse-node) (`@orieg/expanse`) | Native high-performance N-API bindings via `napi-rs`: `ExpanseSet`, `ExpanseMap`, `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap`, `SyncExpanseMap`, `SyncExpanseSet` |"
replacement = target + "\n| **Ruby API** | [`gems/expanse`](gems/expanse) (`gem install expanse`) | Native Ruby extension via magnus / C ABI: `Expanse::Set`, `Expanse::Map`, `Expanse::StrMap`, `Expanse::BytesMap`, `Expanse::BlobMap` |"

if target in content:
    content = content.replace(target, replacement)
else:
    print("Could not find the target string.")

with open('README.md', 'w') as f:
    f.write(content)
