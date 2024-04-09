# Python package

## First of all
```shell
python -m venv .venv
source .venv/bin/activate
pip install "python/[dev]"
```

## How to generate python codes
```shell
cd proto
buf generate --template buf.gen.python.yaml
```

## How to generate documents for python package
```shell
sphinx-build -b html ./python/docs/source ./python/docs/build
```
