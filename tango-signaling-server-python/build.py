#!/usr/bin/env python3
"""
Build script to generate protobuf Python modules from .proto files.
"""

import subprocess
import sys
import os
from pathlib import Path

def main():
    # Get the path to the proto file
    proto_path = Path(__file__).parent.parent / "tango-signaling" / "src" / "proto" / "signaling.proto"
    
    if not proto_path.exists():
        print(f"Error: Proto file not found at {proto_path}", file=sys.stderr)
        sys.exit(1)
    
    # Create output directory
    output_dir = Path(__file__).parent / "tango" / "signaling"
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Generate Python protobuf code
    try:
        subprocess.run(
            [
                sys.executable,
                "-m",
                "grpc_tools.protoc",
                f"--python_out={output_dir.parent}",
                f"--pyi_out={output_dir.parent}",
                str(proto_path),
            ],
            check=True,
        )
        print(f"Successfully generated protobuf modules in {output_dir}")
    except subprocess.CalledProcessError as e:
        print(f"Error generating protobuf modules: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
