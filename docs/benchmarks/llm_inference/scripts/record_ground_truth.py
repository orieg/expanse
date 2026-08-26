#!/usr/bin/env python3
"""
Generates and commits deterministic offline token stream fixtures for speculative decoding benchmarks.

Covers three representative workloads:
1. Code Generation (HumanEval / MBPP style: high context repetition, repeated variable names, types)
2. Document Summarization (CNN/DailyMail style: recurring named entities, topic phrases)
3. Structured JSON Extraction (Strict schemas, repeated object keys, enum types)

Outputs committed fixtures to `data/` so benchmark suites can run 100% offline without GPU or HuggingFace downloads.
"""

import os
import sys
import json
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent.parent / "data"

def generate_code_workload(seed: int = 42) -> dict:
    """Generates synthetic realistic code token streams with high context repetition."""
    # Representative token IDs mapping to standard BPE tokens in Qwen/Llama (vocabs 0..128000)
    # Simulates function definitions, recurring variable names, loops, and return statements
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
    TOK_TRUE = 1942
    TOK_FALSE = 2884

    # Build prompt tokens (e.g. class definition + docstrings + helper utilities)
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

    # Ground truth generation (synthesizing a new function that heavily repeats prompt patterns)
    ground_truth = []
    for step in range(50):
        fn_id = step % 10
        ground_truth.extend([
            TOK_DEF, TOK_FN + fn_id, TOK_LPAREN, TOK_SELF, TOK_VAR_DATA, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_VAR_RES, TOK_EQUAL, 98, TOK_NEWLINE,
            TOK_INDENT, TOK_FOR, TOK_VAR_I, TOK_IN, TOK_RANGE, TOK_LPAREN, 100, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_IF, TOK_VAR_DATA, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_COLON, TOK_NEWLINE,
            TOK_INDENT, TOK_INDENT, TOK_INDENT, TOK_VAR_RES, TOK_DOT, TOK_APPEND, TOK_LPAREN, TOK_VAR_I, TOK_RPAREN, TOK_NEWLINE,
            TOK_INDENT, TOK_RETURN, TOK_VAR_RES, TOK_NEWLINE
        ])

    return {
        "workload": "code_humaneval",
        "description": "Code generation with high context and pattern repetition",
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def generate_summarization_workload(seed: int = 101) -> dict:
    """Generates synthetic document summarization token streams."""
    # Simulates article text containing repeated entities, locations, statistics
    TOK_ARTICLE = [5000 + (i * 37) % 50000 for i in range(2000)]
    # Create repeated key phrase chunks
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

    # Summary ground truth that extracts and composes sentences containing the key phrases
    ground_truth = []
    for phrase in KEY_PHRASES:
        ground_truth.extend([101, 592] + phrase + [284, 1920, 13, 198])
    ground_truth = ground_truth * 5

    return {
        "workload": "summarization_cnndm",
        "description": "Document summarization with key entity and phrase recurrence",
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def generate_json_workload(seed: int = 202) -> dict:
    """Generates structured JSON schema token streams."""
    TOK_LBRACE = 58
    TOK_RBRACE = 60
    TOK_LBRACKET = 59
    TOK_RBRACKET = 61
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
    # Build a schema definition in prompt
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
        "workload": "structured_json",
        "description": "Structured JSON entity extraction with recurring key schemas",
        "prompt_tokens": prompt,
        "ground_truth_tokens": ground_truth,
    }

def main():
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    
    code_data = generate_code_workload()
    with open(DATA_DIR / "code_humaneval_tokens.json", "w", encoding="utf-8") as f:
        json.dump(code_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'code_humaneval_tokens.json'} ({len(code_data['prompt_tokens'])} prompt tokens, {len(code_data['ground_truth_tokens'])} target tokens)")

    summ_data = generate_summarization_workload()
    with open(DATA_DIR / "summarization_cnndm_tokens.json", "w", encoding="utf-8") as f:
        json.dump(summ_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'summarization_cnndm_tokens.json'} ({len(summ_data['prompt_tokens'])} prompt tokens, {len(summ_data['ground_truth_tokens'])} target tokens)")

    json_data = generate_json_workload()
    with open(DATA_DIR / "structured_json_tokens.json", "w", encoding="utf-8") as f:
        json.dump(json_data, f, indent=2)
    print(f"Generated {DATA_DIR / 'structured_json_tokens.json'} ({len(json_data['prompt_tokens'])} prompt tokens, {len(json_data['ground_truth_tokens'])} target tokens)")

if __name__ == "__main__":
    main()
