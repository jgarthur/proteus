"""Behavioral checks for reproduction manifests and offline lineage inspection."""

import json
import hashlib
from pathlib import Path
import tempfile
import unittest

from analyze_genomes import inspect
from reproduction_experiment import manifest, summarize


class GenomeToolsTests(unittest.TestCase):
    """Verify controlled interventions and ancestry boundaries."""

    def test_factorial_conditions_change_only_the_selected_parameters(self):
        """Each intervention preserves the seed, resources, organism and other rates."""
        baseline = manifest("squarestep", "reference", 19, 2000, 100)
        for condition, keys in [
            ("no-maintenance", {"maintenance_rate_log2"}),
            ("mutation-default", {"mutation_base_log2", "mutation_background_log2"}),
            ("mutation-default-no-maintenance", {"maintenance_rate_log2", "mutation_base_log2", "mutation_background_log2"}),
        ]:
            changed = manifest("squarestep", condition, 19, 2000, 100)
            differences = {key for key, value in baseline["simulation"].items()
                           if changed["simulation"][key] != value}
            self.assertEqual(differences, keys)
            self.assertEqual(baseline["bootstrap"], changed["bootstrap"])
            self.assertEqual(baseline["limits"], changed["limits"])

    def test_birth_log_recovers_an_unsampled_child_and_marks_a_missing_parent(self):
        """A child absent from census still has a birth, but an absent parent stays unknown."""
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            files = {
                "manifest.json": {"audit_schema_version": "1.0.0", "trace_start": 0, "trace_end": 2},
                "summary.json": {"tick": 2},
                "births.jsonl": {"uid": 12, "parent_uid": 8, "code": [39, 39]},
            }
            for name, data in files.items():
                (directory / name).write_text(json.dumps(data) + "\n")
            for name in ["lineage.jsonl", "genomes.jsonl", "events.jsonl", "actions.jsonl"]:
                (directory / name).write_text("")
            result = inspect(directory, 12)
            self.assertEqual(result["samples"], [])
            self.assertEqual(result["creation_ancestry"][0]["birth"]["code"], [39, 39])
            self.assertEqual(result["creation_ancestry"][1], {
                "uid": 8, "parent_uid": None, "birth": None, "identity_observed": False,
            })

    def test_summary_preserves_pending_outcomes_and_the_actual_late_window(self):
        """A non-cadence final tick reports its shorter interval with matched denominators."""
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            code = manifest("squarestep", "reference", 1, 12, 10)["bootstrap"]["programs"][0]["code"]
            digest = hashlib.sha256(bytes(code)).hexdigest()
            before = {"boots": 5, "boots_with_known_construction": 5,
                      "matching_creator_at_creation_boots": 4,
                      "clean_single_contributor_boots": 4, "clean_matching_boots": 3}
            after = {"boots": 8, "boots_with_known_construction": 8,
                     "matching_creator_at_creation_boots": 6,
                     "clean_single_contributor_boots": 6, "clean_matching_boots": 5,
                     "constructions_started": 12, "constructions_died_unbooted": 2}
            (directory / "summary.json").write_text(json.dumps({
                "tick": 12, "genome_events": after, "pending_constructions": 2,
                "input_digest": "test",
            }))
            samples = [{"tick": 10, "genome_events": before},
                       {"tick": 12, "genome_events": after, "live": 2,
                        "live_genome_counts": {digest: 2}}]
            (directory / "metrics.jsonl").write_text("".join(json.dumps(row) + "\n" for row in samples))
            (directory / "genomes.jsonl").write_text(json.dumps({"genome": digest, "code": code}) + "\n")
            result = summarize(directory, "squarestep", "reference", 1)
            self.assertEqual(result["pending_constructions"], 2)
            self.assertEqual(result["late_interval_start"], 10)
            self.assertEqual(result["late_boots"], 3)
            self.assertEqual(result["late_boots_with_known_construction"], 3)
            self.assertEqual(result["late_matching_boots"], 2)
            self.assertEqual(result["late_clean_boots"], 2)
            self.assertEqual(result["late_clean_matching_boots"], 2)
            self.assertEqual(result["mean_live_size"], 9)
            self.assertEqual(result["live_original_genome"], 2)

    def test_invalid_ancestry_cycle_is_rejected(self):
        """Malformed artifacts cannot make the ancestor traversal loop forever."""
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "manifest.json").write_text(json.dumps({"audit_schema_version": "1.0.0"}))
            (directory / "summary.json").write_text("{}")
            (directory / "births.jsonl").write_text('{"uid":1,"parent_uid":2}\n{"uid":2,"parent_uid":1}\n')
            (directory / "lineage.jsonl").write_text("")
            with self.assertRaisesRegex(ValueError, "cycle"):
                inspect(directory, 1)


if __name__ == "__main__":
    unittest.main()
