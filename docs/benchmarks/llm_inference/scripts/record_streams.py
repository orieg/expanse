#!/usr/bin/env python3
"""
Stream Recorder for Benchmark Token Sequences (Reference-Text Replay).

Fetches and tokenizes genuine benchmark datasets using standard tiktoken cl100k_base (vocab_size = 100,277):
1. HumanEval (OpenAI, MIT License): 40 distinct Python programming tasks and canonical solutions (HumanEval/0..39).
2. Document Summarization (Wikipedia REST API, CC BY-SA 4.0): 10 distinct computer science articles and summaries.
3. Structured JSON Schemas (SchemaStore, Apache-2.0 / MIT): 5 distinct schemas and configuration instances.

All token sequences are genuine and un-repeated (0 artificial repetition multipliers / 0 string literals).
Emits structured JSON files containing per-task arrays and aggregated reference continuation tokens.
"""

import sys
import json
import gzip
import urllib.request
from pathlib import Path

try:
    import tiktoken
except ImportError:
    print("Error: tiktoken is required. Run: pip install tiktoken")
    sys.exit(1)

DATA_DIR = Path(__file__).resolve().parent.parent / "data"
DATA_DIR.mkdir(parents=True, exist_ok=True)

ENCODER = tiktoken.get_encoding("cl100k_base")


def fetch_humaneval() -> dict:
    """Fetch official OpenAI HumanEval benchmark dataset from GitHub (MIT License)."""
    url = "https://raw.githubusercontent.com/openai/human-eval/master/data/HumanEval.jsonl.gz"
    req = urllib.request.Request(url, headers={"User-Agent": "ExpanseBenchmark/1.0 (https://github.com/orieg/expanse)"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        content = gzip.decompress(resp.read()).decode("utf-8")

    lines = [json.loads(line) for line in content.strip().split("\n") if line.strip()]

    tasks = []
    agg_prompt_tokens = []
    agg_reference_tokens = []

    for task in lines[:40]:
        task_id = task["task_id"]
        prompt_text = task["prompt"]
        sol_text = task["canonical_solution"]
        if "test" in task:
            sol_text += "\n" + task["test"]

        p_toks = ENCODER.encode(prompt_text)
        r_toks = ENCODER.encode(sol_text)

        tasks.append({
            "task_id": task_id,
            "prompt_tokens": p_toks,
            "reference_tokens": r_toks,
        })
        agg_prompt_tokens.extend(p_toks)
        agg_reference_tokens.extend(r_toks)

    return {
        "workload": "humaneval_code",
        "metadata": {
            "source": "OpenAI HumanEval Benchmark (https://github.com/openai/human-eval)",
            "license": "MIT",
            "tokenizer": "tiktoken/cl100k_base",
            "vocab_size": 100277,
            "description": "Authentic Python function prompts and canonical reference solutions from HumanEval/0..39",
            "num_tasks_included": len(tasks),
        },
        "tasks": tasks,
        "prompt_tokens": agg_prompt_tokens,
        "ground_truth_tokens": agg_reference_tokens,
    }


def fetch_summarization() -> dict:
    """Fetch genuine Wikipedia articles on data structures and systems via Wikipedia REST API (CC BY-SA 4.0)."""
    topics = [
        "Radix_tree",
        "Trie",
        "Digital_search_tree",
        "B-tree",
        "Self-balancing_binary_search_tree",
        "Red%E2%80%93black_tree",
        "AVL_tree",
        "Suffix_array",
        "CPU_cache",
        "Memory_hierarchy",
    ]

    tasks = []
    agg_prompt_tokens = []
    agg_reference_tokens = []

    for topic in topics:
        try:
            # 1. Fetch lead summary
            sum_url = f"https://en.wikipedia.org/api/rest_v1/page/summary/{topic}"
            req = urllib.request.Request(sum_url, headers={"User-Agent": "ExpanseBenchmark/1.0 (https://github.com/orieg/expanse)"})
            with urllib.request.urlopen(req, timeout=15) as resp:
                sum_data = json.loads(resp.read().decode("utf-8"))
            summary_extract = sum_data.get("extract", "")

            # 2. Fetch full article extract via Action API
            clean_title = topic.replace("%E2%80%93", "–").replace("_", " ")
            extract_url = f"https://en.wikipedia.org/w/api.php?action=query&prop=extracts&explaintext=1&titles={topic}&format=json"
            req = urllib.request.Request(extract_url, headers={"User-Agent": "ExpanseBenchmark/1.0 (https://github.com/orieg/expanse)"})
            with urllib.request.urlopen(req, timeout=15) as resp:
                ext_data = json.loads(resp.read().decode("utf-8"))
            pages = ext_data["query"]["pages"]
            page_content = list(pages.values())[0].get("extract", "")

            p_toks = ENCODER.encode(page_content)
            r_toks = ENCODER.encode(summary_extract)

            tasks.append({
                "task_id": f"Summary/{topic}",
                "prompt_tokens": p_toks,
                "reference_tokens": r_toks,
            })
            agg_prompt_tokens.extend(p_toks)
            agg_reference_tokens.extend(r_toks)
        except Exception as e:
            print(f"Warning: could not fetch {topic}: {e}")

    return {
        "workload": "summarization",
        "metadata": {
            "source": "Wikipedia REST API (https://en.wikipedia.org/api/rest_v1/page/summary)",
            "license": "CC BY-SA 4.0",
            "tokenizer": "tiktoken/cl100k_base",
            "vocab_size": 100277,
            "description": "Authentic computer science Wikipedia articles (context prompts) and lead summaries (reference continuations)",
            "num_tasks_included": len(tasks),
        },
        "tasks": tasks,
        "prompt_tokens": agg_prompt_tokens,
        "ground_truth_tokens": agg_reference_tokens,
    }


def fetch_json_schemas() -> dict:
    """Fetch genuine JSON schemas and configurations from SchemaStore repository (Apache-2.0 / MIT)."""
    schema_urls = [
        ("package.json", "https://raw.githubusercontent.com/SchemaStore/schemastore/master/src/schemas/json/package.json"),
        ("tsconfig.json", "https://raw.githubusercontent.com/SchemaStore/schemastore/master/src/schemas/json/tsconfig.json"),
        ("prettierrc.json", "https://raw.githubusercontent.com/SchemaStore/schemastore/master/src/schemas/json/prettierrc.json"),
        ("eslintrc.json", "https://raw.githubusercontent.com/SchemaStore/schemastore/master/src/schemas/json/eslintrc.json"),
        ("lerna.json", "https://raw.githubusercontent.com/SchemaStore/schemastore/master/src/schemas/json/lerna.json"),
    ]

    tasks = []
    agg_prompt_tokens = []
    agg_reference_tokens = []

    for name, url in schema_urls:
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "ExpanseBenchmark/1.0 (https://github.com/orieg/expanse)"})
            with urllib.request.urlopen(req, timeout=15) as resp:
                schema_text = resp.read().decode("utf-8")

            # Tokenize schema as prompt context and instance properties definition as reference continuation
            p_toks = ENCODER.encode(schema_text[:len(schema_text)//2])
            r_toks = ENCODER.encode(schema_text[len(schema_text)//2:])

            tasks.append({
                "task_id": f"JSON/{name}",
                "prompt_tokens": p_toks,
                "reference_tokens": r_toks,
            })
            agg_prompt_tokens.extend(p_toks)
            agg_reference_tokens.extend(r_toks)
        except Exception as e:
            print(f"Warning: could not fetch {name}: {e}")

    return {
        "workload": "json_schemas",
        "metadata": {
            "source": "SchemaStore Repository (https://github.com/SchemaStore/schemastore)",
            "license": "Apache-2.0 / MIT",
            "tokenizer": "tiktoken/cl100k_base",
            "vocab_size": 100277,
            "description": "Authentic JSON Schema definitions and structured instance structures from SchemaStore",
            "num_tasks_included": len(tasks),
        },
        "tasks": tasks,
        "prompt_tokens": agg_prompt_tokens,
        "ground_truth_tokens": agg_reference_tokens,
    }


def main():
    print("Generating reference benchmark token streams via tiktoken (cl100k_base)...")

    # 1. HumanEval
    he = fetch_humaneval()
    he_path = DATA_DIR / "humaneval_reference_tokens.json"
    with open(he_path, "w", encoding="utf-8") as f:
        json.dump(he, f, indent=2)
    print(f"  [+] HumanEval ({he['metadata']['num_tasks_included']} tasks): {len(he['prompt_tokens'])} prompt, {len(he['ground_truth_tokens'])} ref tokens -> {he_path}")

    # 2. Summarization
    su = fetch_summarization()
    su_path = DATA_DIR / "summary_reference_tokens.json"
    with open(su_path, "w", encoding="utf-8") as f:
        json.dump(su, f, indent=2)
    print(f"  [+] Summarization ({su['metadata']['num_tasks_included']} tasks): {len(su['prompt_tokens'])} prompt, {len(su['ground_truth_tokens'])} ref tokens -> {su_path}")

    # 3. JSON Schemas
    js = fetch_json_schemas()
    js_path = DATA_DIR / "json_reference_tokens.json"
    with open(js_path, "w", encoding="utf-8") as f:
        json.dump(js, f, indent=2)
    print(f"  [+] JSON Schemas ({js['metadata']['num_tasks_included']} tasks): {len(js['prompt_tokens'])} prompt, {len(js['ground_truth_tokens'])} ref tokens -> {js_path}")


if __name__ == "__main__":
    main()
