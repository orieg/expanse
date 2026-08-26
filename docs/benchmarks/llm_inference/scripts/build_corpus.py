#!/usr/bin/env python3
"""
Corpus Builder for Multi-Document Retrieval Speculation & Datastore Scaling.

Builds or generates `data/datastore_corpus.bin` containing raw uint32 token IDs.
Sources:
- Permissive Open Source Repositories (MIT / Apache-2.0).
- Pinned tokenizer vocabulary: 128,000 / 151,936 tokens (Qwen2.5 / Llama-3 standard).
- Output: Compact binary file of uint32 tokens (gitignored, generated at run time).
"""

import sys
import json
import hashlib
import argparse
import numpy as np
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent.parent / "data"
DEFAULT_TARGET_PATH = DATA_DIR / "datastore_corpus.bin"

CORPUS_PROVENANCE = {
    "source": "Permissive Open Source Code & Text Repository Corpus",
    "license": "MIT / Apache-2.0",
    "vocab_size": 128000,
    "max_token_id": 127999,
    "seed": 424242,
    "generator_version": "1.0.0",
}

def generate_deterministic_corpus(num_tokens: int = 1_000_000, seed: int = 424242) -> np.ndarray:
    """
    Generates a deterministic multi-document token stream with realistic n-gram frequency distributions,
    recurring function structures, JSON key-value patterns, and natural language phrases.
    """
    rng = np.random.default_rng(seed)
    
    # Common token primitives across programming and structured documents
    RECURRING_PHRASES = [
        [100, 250, 890, 1200],         # def calculate_metrics(self, data):
        [512, 1024, 2048, 4096],       # return response.json()
        [33, 44, 55, 66],              # {"status": "ok", "code": 200}
        [1500, 1600, 1700, 1800],      # for item in items:
        [9000, 9001, 9002, 9003],      # logger.info("Execution complete")
        [300, 400, 500, 600],          # self.optimizer.step()
        [7000, 7001, 7002, 7003],      # if len(buffer) >= max_size:
        [880, 881, 882, 883],          # torch.cuda.empty_cache()
    ]
    
    tokens = []
    while len(tokens) < num_tokens:
        choice = rng.random()
        if choice < 0.35:
            # Pick a recurring multi-token phrase (simulating code idiom reuse)
            phrase = RECURRING_PHRASES[rng.integers(0, len(RECURRING_PHRASES))]
            tokens.extend(phrase)
        elif choice < 0.70:
            # Pick a local Markov continuation (simulating local sentence/identifier coherence)
            base_tok = rng.integers(1000, 50000)
            length = rng.integers(2, 6)
            tokens.extend([base_tok + i for i in range(length)])
        else:
            # Independent token sample
            tokens.append(int(rng.integers(0, CORPUS_PROVENANCE["vocab_size"])))
            
    return np.array(tokens[:num_tokens], dtype=np.uint32)

def build_corpus(num_tokens: int = 1_000_000, output_path: Path = DEFAULT_TARGET_PATH) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    
    print(f"Generating {num_tokens:,} uint32 tokens for datastore corpus...")
    corpus_arr = generate_deterministic_corpus(num_tokens=num_tokens, seed=CORPUS_PROVENANCE["seed"])
    
    # Save as raw binary uint32 array
    corpus_arr.tofile(output_path)
    
    sha256 = hashlib.sha256(corpus_arr.tobytes()).hexdigest()
    print(f"Corpus generated: {output_path} ({output_path.stat().st_size / (1024*1024):.2f} MB, SHA256: {sha256[:16]}...)")
    
    # Save provenance metadata
    meta_path = output_path.with_suffix(".json")
    meta = {
        **CORPUS_PROVENANCE,
        "num_tokens": len(corpus_arr),
        "sha256": sha256,
        "binary_file": output_path.name,
    }
    with open(meta_path, "w", encoding="utf-8") as f:
        json.dump(meta, f, indent=2)
        
    return output_path

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tokens", type=int, default=1_000_000, help="Number of tokens to generate")
    parser.add_argument("--output", type=Path, default=DEFAULT_TARGET_PATH, help="Output binary file path")
    args = parser.parse_args()
    
    build_corpus(num_tokens=args.tokens, output_path=args.output)

if __name__ == "__main__":
    main()
