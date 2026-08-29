Gem::Specification.new do |spec|
  spec.name          = "expanse"
  spec.version       = "0.5.0"
  spec.authors       = ["Nicolas Brousse"]
  spec.email         = ["nicolas@brousse.info"]
  spec.summary       = "Expanse: clean-room, pure-Rust Judy arrays"
  spec.description   = "Ruby bindings for the expanse library."
  spec.homepage      = "https://github.com/orieg/expanse"
  spec.license       = "MIT OR Apache-2.0"

  spec.files         = Dir["lib/**/*.rb", "README.md", "expanse.gemspec", "Rakefile"]
  spec.require_paths = ["lib"]
  spec.required_ruby_version = ">= 3.0.0"
end
