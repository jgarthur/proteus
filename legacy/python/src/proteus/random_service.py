from __future__ import annotations

from typing import Iterable

import numpy as np


class RandomService:
    def __init__(self, seed: int | None = None, state: dict | None = None) -> None:
        bit_generator = np.random.PCG64(seed if seed is not None else 0)
        if state is not None:
            bit_generator.state = state
        self.generator = np.random.Generator(bit_generator)

    @property
    def state(self) -> dict:
        return self.generator.bit_generator.state

    def integers(self, low: int, high: int | None = None) -> int:
        return int(self.generator.integers(low, high))

    def bernoulli(self, probability: float, size: tuple[int, ...] | None = None):
        return self.generator.random(size=size) < float(probability)

    def binomial(self, trials, probability: float):
        return self.generator.binomial(trials, float(probability))

    def weighted_choice_index(self, weights: Iterable[int]) -> int:
        weight_list = [max(0, int(weight)) for weight in weights]
        total = sum(weight_list)
        if total <= 0:
            return 0
        threshold = float(self.generator.random()) * total
        running = 0.0
        for index, weight in enumerate(weight_list):
            running += weight
            if threshold < running:
                return index
        return len(weight_list) - 1
