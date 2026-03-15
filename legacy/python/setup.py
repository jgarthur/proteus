from __future__ import annotations

from pathlib import Path

from setuptools import Extension, find_packages, setup


def build_extensions():
    source_dir = Path(__file__).parent / "src" / "proteus"
    pyx_path = source_dir / "_engine_cython_ops.pyx"
    try:
        from Cython.Build import cythonize
    except ImportError:
        return []
    if not pyx_path.exists():
        return []
    return cythonize(
        [
            Extension(
                "proteus._engine_cython_ops",
                [str(pyx_path)],
            )
        ],
        compiler_directives={
            "language_level": "3",
            "boundscheck": False,
            "wraparound": False,
            "initializedcheck": False,
            "nonecheck": False,
        },
    )


setup(
    package_dir={"": "src"},
    packages=find_packages("src"),
    ext_modules=build_extensions(),
)
