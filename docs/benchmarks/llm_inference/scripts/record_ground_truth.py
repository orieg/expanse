#!/usr/bin/env python3
"""
Generates and commits deterministic offline synthetic token stream fixtures for speculative decoding benchmarks.

Covers three representative synthetic workload patterns:
1. Code Patterns (Simulates function definitions, recurring variable names, loops, returns)
2. Summary Patterns (Simulates article text with recurring entity key phrases)
3. JSON Schemas (Simulates nested object schemas, repeated keys, enums)

Outputs committed fixtures to `data/` so benchmark suites run 100% offline with zero external downloads.
"""

import sys
import json
import random
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent.parent / "data"

def generate_code_workload(seed: int = 42) -> dict:
    """Generates synthetic code token streams with high context and structural repetition."""
    rng = random.Random(seed)
    
    TOK_DEF = 1205
    TOK_FN = 4821
    TOK_LPAREN = 7
    TOK_RPAREN = 8
    TOK_COLON = 25
    TOK_INDENT = 258
    TOK_FOR = 512
    TOK_IN = 290
    TOK_RANGE = 2814
    TOK_IF = 415
    TOK_RETURN = 672
    TOK_VAR_I = 312
    TOK_VAR_RES = 1840
    TOK_VAR_DATA = 2410
    TOK_PLUS = 488
    TOK_EQUAL = 284
    TOK_NEWLINE = 198
    TOK_SELF = 2104
    TOK_DOT = 13
    TOK_APPEND = 4590

    # Build prompt context
    prompt = []
    for fn_id in range(10):
        prompt.extend([
            TOK_DEF, TOK_FN + fn_id, TOK_LPAREN, TOK_SELF, TOK_VAR_DATA, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_VAR_RES, TOK_EQUAL, 98, TOK_NEWLINE,
            TOK_INDENT, TOK_FOR, TOK_VAR_I, TOK_IN, TOK_RANGE, TOK_LPAREN, 100, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_IF, TOK_VAR_DATA, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_INDENT, TOK_VAR_RES, TOK_DOT, TOK_APPEND, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_NEWLINE,
            TOK_INDENT, TOK_RETURN, TOK_VAR_RES, TOK_NEWLINE, TOK_NEWLINE
        ])

    # Target sequence with repetition
    ground_truth = []
    for step in range(50):
        fn_id = rng.choice(range(10))
        ground_truth.extend([
            TOK_DEF, TOK_FN + fn_id, TOK_LPAREN, TOK_SELF, TOK_VAR_DATA, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_VAR_RES, TOK_EQUAL, 98, TOK_NEWLINE,
            TOK_INDENT, TOK_FOR, TOK_VAR_I, TOK_IN, TOK_RANGE, TOK_LPAREN, 100, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_IF, TOK_VAR_DATA, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_INDENT, TOK_VAR_RES, TOK_DOT, TOK_APPEND, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_NEWLINE,
            TOK_INDENT, TOK_RETURN, TOK_VAR_RES, TOK_NEWLINE
        ])

    return {
        "workload": "code_patterns",
        "description": "Synthetic code generation pattern fixture with high context repetition",
        "seed": seed,
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def generate_summary_workload(seed: int = 101) -> dict:
    """Generates synthetic summarization token streams with recurring key phrases."""
    rng = random.Random(seed)
    
    TOK_ARTICLE = [5000 + (i * 37) % 50000 for i in range(2000)]
    KEY_PHRASES = [
        [12040, 391, 4810, 2901],
        [8910, 110, 48201, 3910],
        [310, 9410, 1002, 591],
        [781, 4801, 29, 48100],
    ]
    prompt = list(TOK_ARTICLE)
    for i, phrase in enumerate(KEY_PHRASES):
        idx = (i + 1) * 300
        prompt[idx:idx+len(phrase)] = phrase

    ground_truth = []
    for _ in range(5):
        for phrase in KEY_PHRASES:
            ground_truth.extend([101, 592] + phrase + [284, 1920, 13, 198])

    return {
        "workload": "summary_patterns",
        "description": "Synthetic document summarization pattern fixture with recurring phrases",
        "seed": seed,
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def generate_json_workload(seed: int = 202) -> dict:
    """Generates synthetic structured JSON schema token streams."""
    rng = random.Random(seed)
    
    TOK_LBRACE = 58
    TOK_RBRACE = 60
    TOK_COLON = 25
    TOK_COMMA = 11
    TOK_QUOTE = 36
    TOK_ID_KEY = 1420
    TOK_NAME_KEY = 2810
    TOK_TYPE_KEY = 3910
    TOK_STATUS_KEY = 4910
    TOK_ACTIVE = 18400
    TOK_PENDING = 29400

    prompt = []
    for i in range(20):
        prompt.extend([
            TOK_LBRACE, TOK_QUOTE, TOK_ID_KEY, TOK_QUOTE, TOK_COLON, 1000 + i, TOK_COMMA,
            TOK_QUOTE, TOK_NAME_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, 5000 + i, TOK_QUOTE, TOK_COMMA,
            TOK_QUOTE, TOK_TYPE_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, 8000 + (i % 3), TOK_QUOTE, TOK_COMMA,
            TOK_QUOTE, TOK_STATUS_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, (TOK_ACTIVE if i % 2 == 0 else TOK_PENDING), TOK_QUOTE,
            TOK_RBRACE, TOK_COMMA
        ])

    ground_truth = []
    for i in range(40):
        ground_truth.extend([
            TOK_LBRACE, TOK_QUOTE, TOK_ID_KEY, TOK_QUOTE, TOK_COLON, 2000 + i, TOK_COMMA,
            TOK_QUOTE, TOK_NAME_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, 5000 + (i % 20), TOK_QUOTE, TOK_COMMA,
            TOK_QUOTE, TOK_TYPE_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, 8000 + (i % 3), TOK_QUOTE, TOK_COMMA,
            TOK_QUOTE, TOK_STATUS_KEY, TOK_QUOTE, TOK_COLON, TOK_QUOTE, (TOK_ACTIVE if i % 2 == 0 else TOK_PENDING), TOK_QUOTE,
            TOK_RBRACE, TOK_COMMA
        ])

    return {
        "workload": "json_schemas",
        "description": "Synthetic structured JSON schema fixture with recurring key patterns",
        "seed": seed,
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def main():
    DATA_DIR.mkdir(parents=True, exist_ok=True)

    code_data = generate_code_workload()
    with open(DATA_DIR / "code_patterns_tokens.json", "w", encoding="utf-8") as f:
        json.dump(code_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'code_patterns_tokens.json'} ({len(code_data['prompt_tokens'])} prompt tokens, {len(code_data['ground_truth_tokens'])} target tokens)")

    summary_data = generate_summary_workload()
    with open(DATA_DIR / "summary_patterns_tokens.json", "w", encoding="utf-8") as f:
        json.dump(summary_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'summary_patterns_tokens.json'} ({len(summary_data['prompt_tokens'])} prompt tokens, {len(summary_data['ground_truth_tokens'])} target tokens)")

    json_data = generate_json_workload()
    with open(DATA_DIR / "json_schemas_tokens.json", "w", encoding="utf-8") as f:
        json.dump(json_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'json_schemas_tokens.json'} ({len(json_data['prompt_tokens'])} prompt tokens, {len(json_data['ground_truth_tokens'])} target tokens)")

if __name__ == "__main__":
    main()
