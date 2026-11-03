#!/usr/bin/env python3
"""Run the unchanged eleven boundary checks for the frozen grapheme slice."""
import check_harfrust_dotted_circle_kernel_gates as shared


if __name__ == "__main__":
    shared.PREFIX = "grapheme-kernel"
    shared.main()
