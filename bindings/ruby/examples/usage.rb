# The landing page's Ruby usage example. scripts/build_pages.py renders this
# file (without the `# Output:` block) on the Ruby tab, and
# test/test_expanse.rb runs it and checks its output against that block, so
# the page cannot drift from the binding.
require "expanse"

# Integer map: 64-bit keys to 64-bit values
map = Expanse::Map.new
map[42] = 100
puts "Key 42: #{map[42]}"

# Integer set with O(depth) rank
set = Expanse::Set.new
set.add(1001)
set.add(1002)
puts "Contains 1001: #{set.include?(1001)}"
puts "Rank: #{set.rank(1002)}"

# Output:
# Key 42: 100
# Contains 1001: true
# Rank: 1
