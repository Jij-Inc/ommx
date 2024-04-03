import glob
import subprocess
import sys


if __name__ == "__main__":
    args = sys.argv
    SRC_DIR = args[1]
    DST_DIR = args[2]

    file_paths = glob.glob(f"{SRC_DIR}/**/*.proto", recursive=True)
    
    for path in file_paths:
        subprocess.Popen([
            "protoc",
            f"-I={SRC_DIR}",
            f"--python_out={DST_DIR}",
            f"{path}",
        ])
