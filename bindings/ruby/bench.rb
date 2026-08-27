#!/usr/bin/env ruby
# frozen_string_literal: true

# Cross-Runtime Comparative Benchmark Suite for Expanse Ruby Bindings.
# Compares Expanse::Map and Expanse::Set against native Ruby Hash and Set.

require "optparse"
require "json"
require "set"
require "objspace"
require_relative "lib/expanse"

options = { pop: 50_000, quick: false, json: false }
OptionParser.new do |opts|
  opts.banner = "Usage: bench.rb [options]"
  opts.on("--quick", "Run quick benchmark (N=10,000)") { options[:quick] = true; options[:pop] = 10_000 }
  opts.on("--pop N", Integer, "Population size") { |n| options[:pop] = n }
  opts.on("--json", "Emit JSON output") { options[:json] = true }
end.parse!

class XorShift64
  def initialize(seed = 0x0DDB_1A5E_5EED_0001)
    @state = seed & 0xFFFF_FFFF_FFFF_FFFF
  end

  def next
    x = @state
    x ^= (x << 13) & 0xFFFF_FFFF_FFFF_FFFF
    x ^= (x >> 7) & 0xFFFF_FFFF_FFFF_FFFF
    x ^= (x << 17) & 0xFFFF_FFFF_FFFF_FFFF
    @state = x
    x
  end
end

def generate_keys(pop, dist = "random")
  rng = XorShift64.new
  if dist == "sequential"
    Array.new(pop) { |i| i }
  elsif dist == "clustered"
    base = 0
    Array.new(pop) do |i|
      base = rng.next & ~0xFF if (i % 256).zero?
      base + (i % 256)
    end
  else
    Array.new(pop) { rng.next }
  end
end

def measure(rounds = 3)
  best = Float::INFINITY
  rounds.times do
    t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    yield
    t1 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    dt = t1 - t0
    best = dt if dt < best
  end
  best
end

def run_suite(pop, dist = "random")
  keys = generate_keys(pop, dist)
  probe_keys = keys.shuffle(random: Random.new(0x9E37_79B9))

  # 1. Expanse::Map
  GC.start
  exp_map = Expanse::Map.new
  exp_insert_s = measure do
    exp_map.clear
    keys.each { |k| exp_map[k] = k ^ 0x55 }
  end

  exp_lookup_s = measure do
    sink = 0
    probe_keys.each do |k|
      v = exp_map[k]
      sink ^= v if v
    end
    sink
  end

  exp_iter_s = measure do
    count = 0
    exp_map.each { |_k, _v| count += 1 }
    count
  end

  # Real native arena accounting is REQUIRED: pre-#373 this silently fell back
  # to hardcoded constants (22.5/8.6), so the nightly Ruby memory gate could
  # structurally never fire.
  unless exp_map.respond_to?(:mem_used)
    abort "Error: Expanse::Map#mem_used is not available — cannot measure bytes/key. " \
          "Rebuild the native library (cargo build --release -p expanse-capi) and update lib/expanse.rb."
  end
  exp_bytes_per_key = exp_map.mem_used.to_f / pop

  # 2. Ruby Hash
  GC.start
  rb_hash = {}
  rb_insert_s = measure do
    rb_hash.clear
    keys.each { |k| rb_hash[k] = k ^ 0x55 }
  end

  rb_lookup_s = measure do
    sink = 0
    probe_keys.each do |k|
      v = rb_hash[k]
      sink ^= v if v
    end
    sink
  end

  rb_iter_s = measure do
    count = 0
    rb_hash.each { |_k, _v| count += 1 }
    count
  end

  # ObjectSpace.memsize_of is a SHALLOW measurement: it covers the Hash's own
  # entry table but not off-slab key/value objects. Random 64-bit keys exceed
  # Ruby's 62-bit Fixnum range, so many keys are separately-allocated Bignums
  # that this number does not include — hence the estimated flag (it is a
  # lower bound, not the fabricated 64.0 constant emitted pre-#373).
  rb_hash_bytes_per_key = ObjectSpace.memsize_of(rb_hash).to_f / pop

  # 3. Expanse::Set
  exp_set = Expanse::Set.new
  exp_set_insert_s = measure do
    exp_set.clear
    keys.each { |k| exp_set.add(k) }
  end

  exp_set_lookup_s = measure do
    count = 0
    probe_keys.each { |k| count += 1 if exp_set.include?(k) }
    count
  end

  # 4. Ruby Set
  rb_set = Set.new
  rb_set_insert_s = measure do
    rb_set.clear
    keys.each { |k| rb_set.add(k) }
  end

  rb_set_lookup_s = measure do
    count = 0
    probe_keys.each { |k| count += 1 if rb_set.include?(k) }
    count
  end

  to_mops = ->(s) { (pop / s) / 1e6 }
  to_ns = ->(s) { (s * 1e9) / pop }

  {
    dist: dist,
    pop: pop,
    expanse_map: {
      insert_mops: to_mops.call(exp_insert_s),
      lookup_mops: to_mops.call(exp_lookup_s),
      lookup_ns: to_ns.call(exp_lookup_s),
      iter_mops: to_mops.call(exp_iter_s),
      bytes_per_key: exp_bytes_per_key
    },
    ruby_hash: {
      insert_mops: to_mops.call(rb_insert_s),
      lookup_mops: to_mops.call(rb_lookup_s),
      lookup_ns: to_ns.call(rb_lookup_s),
      iter_mops: to_mops.call(rb_iter_s),
      bytes_per_key: rb_hash_bytes_per_key,
      # Shallow ObjectSpace.memsize_of / pop: excludes off-slab Bignum keys
      # (random 64-bit keys > 2**62), so this is a lower-bound estimate.
      bytes_per_key_estimated: true
    },
    expanse_set: {
      insert_mops: to_mops.call(exp_set_insert_s),
      lookup_mops: to_mops.call(exp_set_lookup_s),
      lookup_ns: to_ns.call(exp_set_lookup_s)
    },
    ruby_set: {
      insert_mops: to_mops.call(rb_set_insert_s),
      lookup_mops: to_mops.call(rb_set_lookup_s),
      lookup_ns: to_ns.call(rb_set_lookup_s)
    }
  }
end

def render_table(results)
  puts "\n================================================================================"
  puts "  Expanse Ruby Bindings Comparative Performance Report"
  puts "================================================================================"

  results.each do |r|
    puts "\n[ Distribution: #{r[:dist]} | Population: #{r[:pop]} ]"
    printf "%-20s | %11s | %13s | %13s | %11s | %8s\n", "Target", "Lookup (ns)", "Lookup (Mops)", "Insert (Mops)", "Iter (Mops)", "B/key"
    puts "#{'-' * 20}-+-#{'-' * 11}-+-#{'-' * 13}-+-#{'-' * 13}-+-#{'-' * 11}-+-#{'-' * 8}"

    em = r[:expanse_map]
    printf "%-20s | %11.2f | %13.2f | %13.2f | %11.2f | %8.2f\n", "Expanse::Map", em[:lookup_ns], em[:lookup_mops], em[:insert_mops], em[:iter_mops], em[:bytes_per_key]

    rh = r[:ruby_hash]
    printf "%-20s | %11.2f | %13.2f | %13.2f | %11.2f | %8.2f\n", "Ruby Hash", rh[:lookup_ns], rh[:lookup_mops], rh[:insert_mops], rh[:iter_mops], rh[:bytes_per_key]

    es = r[:expanse_set]
    printf "%-20s | %11.2f | %13.2f | %13.2f | %11s | %8s\n", "Expanse::Set", es[:lookup_ns], es[:lookup_mops], es[:insert_mops], "—", "—"

    rs = r[:ruby_set]
    printf "%-20s | %11.2f | %13.2f | %13.2f | %11s | %8s\n", "Ruby Set", rs[:lookup_ns], rs[:lookup_mops], rs[:insert_mops], "—", "—"
  end
  puts "\n================================================================================\n\n"
end

pop = options[:pop]
dists = %w[random sequential clustered]
results = dists.map { |d| run_suite(pop, d) }

if options[:json]
  puts JSON.pretty_generate({ runtime: "ruby", results: results })
else
  render_table(results)
end
