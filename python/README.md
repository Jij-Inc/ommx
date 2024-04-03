## About This Workspace
- `ommx`
  - The python package for OMMX
- `protogen.py`
  - Python code generator from `*.proto` files

## Generate Python Code
```shell
# $SRC_DIR: The source directory where protobuf code lives.
# $DST_DIR: The distination directory where you want the generated code to go.
python protogen.py $SRC_DIR $DST_DIR
```
