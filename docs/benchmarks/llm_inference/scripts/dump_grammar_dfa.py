#!/usr/bin/env python3
"""
Reproducible Grammar DFA State Generator for Grammar-Constrained Decoding Benchmarks.

Generates `data/grammar_dfa_states.json` representing DFA states for structured JSON schemas:
- Vocab sizes: 128,000 / 151,936 / 256,000 tokens.
- Sparsity tiers:
  1. Dense states (10% allowed tokens, e.g. general text / string body).
  2. Medium states (1% allowed tokens, e.g. identifiers / alphanumeric tokens).
  3. Sparse states (0.01% - 0.1% allowed tokens, e.g. JSON punctuation, boolean, null, keyword enums).
"""

import sys
import json
import argparse
import numpy as np
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent.parent / "data"

def generate_dfa_states(num_states: int = 1000, vocab_size: int = 128000, seed: int = 424242):
    rng = np.random.default_rng(seed)
    
    states = []
    
    # Pre-define token sets for common grammatical primitives
    punct_tokens = [500, 501, 502, 503, 504, 505, 506] # { } [ ] : , "
    bool_null_tokens = [1000, 1001, 1002] # true, false, null
    digit_tokens = list(range(100, 120)) # 0..9, ., -, +, e, E
    
    for state_id in range(num_states):
        tier_choice = rng.random()
        
        if tier_choice < 0.40:
            # Sparse state (0.01% - 0.1% allowed tokens): punctuation / keywords / enums
            k = rng.integers(5, 50)
            base_set = punct_tokens if (state_id % 2 == 0) else bool_null_tokens
            extra = rng.integers(0, vocab_size, size=k).tolist()
            allowed = sorted(list(set(base_set + extra)))
            tier = "sparse_0.01pct"
        elif tier_choice < 0.75:
            # Medium state (~1% allowed tokens): identifiers, alphanumeric continuations
            k = int(vocab_size * 0.01)
            allowed = sorted(rng.integers(0, vocab_size, size=k).tolist())
            tier = "medium_1pct"
        else:
            # Dense state (~10% allowed tokens): general string / free text
            k = int(vocab_size * 0.10)
            allowed = sorted(rng.integers(0, vocab_size, size=k).tolist())
            tier = "dense_10pct"
            
        states.append({
            "state_id": state_id,
            "tier": tier,
            "allowed_count": len(allowed),
            "allowed_tokens": allowed,
        })
        
    return {
        "num_states": num_states,
        "vocab_size": vocab_size,
        "seed": seed,
        "states": states,
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--states", type=int, default=1000, help="Number of DFA states to generate")
    parser.add_argument("--vocab", type=int, default=128000, help="Vocabulary size")
    args = parser.parse_args()
    
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    dfa_data = generate_dfa_states(num_states=args.states, vocab_size=args.vocab)
    
    out_path = DATA_DIR / "grammar_dfa_states.json"
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(dfa_data, f, indent=2)
    print(f"Generated {len(dfa_data['states'])} DFA states in {out_path}")

if __name__ == "__main__":
    main()
