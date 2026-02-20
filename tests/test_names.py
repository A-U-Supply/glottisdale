"""Tests for run name generation."""

import re
from datetime import date
from pathlib import Path

from glottisdale.names import ADJECTIVES, NOUNS, generate_name, generate_run_id, create_run_dir


def test_adjectives_are_lowercase_alpha():
    """All adjectives contain only lowercase letters and hyphens."""
    for adj in ADJECTIVES:
        assert re.fullmatch(r"[a-z]+(-[a-z]+)*", adj), f"Invalid adjective: {adj}"


def test_nouns_are_lowercase_alpha():
    """All nouns contain only lowercase letters and hyphens."""
    for noun in NOUNS:
        assert re.fullmatch(r"[a-z]+(-[a-z]+)*", noun), f"Invalid noun: {noun}"


def test_word_list_sizes():
    """Lists are large enough for 40k+ combinations."""
    assert len(ADJECTIVES) >= 200
    assert len(NOUNS) >= 200
    assert len(ADJECTIVES) * len(NOUNS) >= 40_000


def test_no_duplicate_adjectives():
    assert len(ADJECTIVES) == len(set(ADJECTIVES))


def test_no_duplicate_nouns():
    assert len(NOUNS) == len(set(NOUNS))


def test_generate_name_format():
    """Name is adjective-noun format."""
    name = generate_name()
    parts = name.split("-")
    # At least 2 parts (adjective and noun, each may have internal hyphens
    # but the simplest case is exactly 2)
    assert len(parts) >= 2
    assert name  # non-empty


def test_generate_name_deterministic_with_seed():
    """Same seed produces same name."""
    name1 = generate_name(seed=42)
    name2 = generate_name(seed=42)
    assert name1 == name2


def test_generate_name_different_seeds():
    """Different seeds produce different names (with very high probability)."""
    name1 = generate_name(seed=1)
    name2 = generate_name(seed=2)
    assert name1 != name2


def test_generate_name_without_seed_varies():
    """Without seed, names should vary (generate 10, expect at least 2 unique)."""
    names = {generate_name() for _ in range(10)}
    assert len(names) >= 2


def test_generate_run_id_format():
    """Run ID is YYYY-MM-DD-adjective-noun."""
    run_id = generate_run_id(seed=42)
    today = date.today().isoformat()
    assert run_id.startswith(today + "-")
    # The part after the date should be a valid name
    name_part = run_id[len(today) + 1:]
    assert len(name_part) > 0


def test_generate_run_id_deterministic():
    run_id1 = generate_run_id(seed=99)
    run_id2 = generate_run_id(seed=99)
    assert run_id1 == run_id2


def test_create_run_dir_creates_directory(tmp_path):
    """create_run_dir makes the directory and returns its path."""
    run_dir = create_run_dir(tmp_path, seed=42)
    assert run_dir.exists()
    assert run_dir.is_dir()
    assert run_dir.parent == tmp_path


def test_create_run_dir_collision_appends_suffix(tmp_path):
    """If the directory already exists, append -2, -3, etc."""
    first = create_run_dir(tmp_path, seed=42)
    second = create_run_dir(tmp_path, seed=42)
    assert first != second
    assert second.name.endswith("-2")
    third = create_run_dir(tmp_path, seed=42)
    assert third.name.endswith("-3")


def test_create_run_dir_with_custom_name(tmp_path):
    """Custom run_name overrides the generated adjective-noun part."""
    run_dir = create_run_dir(tmp_path, run_name="final-take")
    today = date.today().isoformat()
    assert run_dir.name == f"{today}-final-take"


def test_create_run_dir_custom_name_collision(tmp_path):
    """Custom names also get collision suffixes."""
    first = create_run_dir(tmp_path, run_name="my-run")
    second = create_run_dir(tmp_path, run_name="my-run")
    assert second.name.endswith("-2")
