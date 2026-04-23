---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# OMMXワークロードのトレースとプロファイリング

OMMXはRustコア (`tracing` + `pyo3-tracing-opentelemetry`) とPython側の主要エントリポイントから [OpenTelemetry](https://opentelemetry.io/) のスパンを出力します。これを人間が読める形にまとめるための薄いラッパーが `ommx.tracing` に2つ用意されています。

- **`%%ommx_trace`** — 単一セルの実行中に発生したスパンをネストしたテキストツリーとして表示するJupyterセルマジック。加えてChrome Trace Event Format形式のJSONダウンロードリンクも表示されます。
- **`capture_trace` / `@traced`** — 同じ仕組みを通常のPythonスクリプト、テスト、CIから利用するためのコンテキストマネージャとデコレータ。

両者は1つのインプロセスコレクタを共有しています。import時に何かを設定する必要はなく、OTelエクスポータをインストールする必要もありません。コレクタは最初の利用時に遅延インストールされます。OTelバックエンドへトレースを送信したい場合のみ、後述の [独自のTracerProviderを使う](#own-tracer-provider) セクションを参照してください。

## クイックツアー

### セルマジック (`%%ommx_trace`)

ノートブックごとに一度、拡張を読み込みます（通常は最初のセル）。

```
%load_ext ommx.tracing
```

以降、任意のセルの先頭に `%%ommx_trace` を付けるだけで計測されます。

```
%%ommx_trace
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")
instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={},
    sense=Instance.MAXIMIZE,
)
solution = instance.evaluate({0: 1.0, 1: 1.0})
```

セルの出力として以下の2つが表示されます。

1. セル内で発生した全スパン（RustおよびPython双方）を**ネストしたテキストツリー**として描画。各ノードには持続時間と代表的な属性が付与されます。
2. Chrome Trace Event Format形式のトレース全体の**ダウンロードリンク**。生成されたJSONファイルを [Perfetto](https://ui.perfetto.dev/) / [speedscope](https://www.speedscope.app/) / `chrome://tracing` に読み込ませることでフレームグラフとして閲覧できます。

セル内で例外が発生した場合も、トレースのHTMLは通常どおり描画されます（失敗したスパンには `[ERROR]` マーカーが付きます）。そしてその後例外は再送出されるので、`nbconvert --execute`、papermill、pytest-nbval などのノートブック自動化ツールから見てもセルは失敗扱いとなります。

### コンテキストマネージャ (`capture_trace`)

通常のPythonスクリプトからも同じ仕組みを利用できます。

```{code-cell} ipython3
from ommx.tracing import capture_trace
from ommx.v1 import Instance, DecisionVariable

x = DecisionVariable.binary(0, name="x")
y = DecisionVariable.binary(1, name="y")

instance = Instance.from_components(
    decision_variables=[x, y],
    objective=x + y,
    constraints={},
    sense=Instance.MAXIMIZE,
)

with capture_trace() as trace:
    solution = instance.evaluate({0: 1.0, 1: 1.0})

print(trace.text_tree())
```

`trace` はブロック終了時に値が埋められる `TraceResult` です。

- `trace.spans` — カスタム処理用の生の `list[ReadableSpan]`
- `trace.text_tree()` — セルマジックと同じネストしたテキストツリー
- `trace.chrome_trace_json()` — トレースをJSON文字列として返す
- `trace.save_chrome_trace(path)` — JSONをディスクに書き出す（必要な親ディレクトリは自動的に作成）

ブロック内で例外が発生した場合でも `trace.spans` は埋められており（失敗したスパンには `[ERROR]` マーカーが付く）、外側の `except` や `finally` から内容を調査・保存できます。元の例外はそのまま伝播します。OMMXが例外を握り潰すことはありません。

```{code-cell} ipython3
from pathlib import Path

trace.save_chrome_trace("/tmp/ommx_trace.json")
print(f"書き出しサイズ: {Path('/tmp/ommx_trace.json').stat().st_size} bytes")
```

### デコレータ (`@traced`)

`@traced` は `capture_trace` の糖衣構文です。

```{code-cell} ipython3
from ommx.tracing import traced

@traced(output="/tmp/evaluate_trace.json")
def evaluate_once(inst):
    return inst.evaluate({0: 1.0, 1: 1.0})

solution = evaluate_once(instance)
print("トレースを /tmp/evaluate_trace.json に書き出しました")
```

3つの呼び出し形式すべてがサポートされています。

```python
@traced
def f(): ...

@traced()
def f(): ...

@traced(name="build_qubo", output="qubo.json")
def f(): ...
```

主な挙動:

- `name` を省略するとルートスパン名は `fn.__qualname__` になります。複数のデコレート関数のトレースを区別しやすくするためです。
- `output` を指定した場合、**正常終了時も例外発生時も** Chrome Trace JSONが書き出されます。情報は捨てられません。
- `async def` もサポートされています。`inspect.iscoroutinefunction` でコルーチン関数を検知し、トレースブロック内で `await` します。この検知がないと、コルーチン生成直後にキャプチャウィンドウが閉じてしまい、スパンが全て失われます。

## スパン命名規則

OMMXは `tracing` のデフォルトのスパン名（`from_bytes`、`evaluate`、`reduce_capabilities` などの関数名そのまま）を採用しています。モジュールパスはOTelの**インストルメンテーションスコープ**とスパン属性 `code.namespace` に記録されるので、同じ関数名でもスコープ名や属性から区別可能です。

複数の型で同名のメソッドが存在する場合（例: `Instance.evaluate` と `SampleSet.evaluate`）、Rust側では独自のスパン名ではなくスパン**フィールド** (`fields(artifact_storage = ...)` など) で区別します。これらのフィールドはツリー表示ではOTel属性として、Chrome Traceでは `args` 辞書として表示されます。

(own-tracer-provider)=
## 独自のTracerProviderを使う

`ommx.tracing` は `TracerProvider` が登録されていない場合のみインプロセスの `TracerProvider` をインストールします。OTLP、Jaeger、Honeycombなどの外部バックエンドに送信したい場合は、**OMMX拡張への最初の呼び出し前に**独自のプロバイダを設定してください。

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter

provider = TracerProvider()
provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
trace.set_tracer_provider(provider)

# 以降、通常通りOMMXをimport / 呼び出しできます。`%%ommx_trace` や
# `capture_trace` は既存のproviderに対してコレクタを追加するだけで、
# OTLPエクスポータと共存します。
from ommx.v1 import Instance
```

注意点が2つあります。

1. **OpenTelemetryは最初の `set_tracer_provider` 呼び出しのみ採用します。** 最初の `Instance.from_bytes(...)` など計測対象の呼び出しの後にproviderを設定しても無視されます。OTel設定はスクリプトやノートブックの最上部で行うようにしてください。
2. **`ommx.tracing` はアクティブなproviderにコレクタを追加するだけで、置き換えは行いません。** スパンはOMMXのレンダラと指定したOTLPエクスポータの両方に到達します。

`add_span_processor` をサポートしない非SDKのproviderがアクティブな場合（稀ですが一部のベンダーSDKはこの挙動です）、`capture_trace` は `__enter__` 時点で `RuntimeError` を送出します。エラーメッセージに記載されているとおり、`opentelemetry.sdk.trace.TracerProvider` を自前でインストールし、エクスポータをもう一つの `SpanProcessor` として同じproviderに追加してください。

## トラブルシューティング

### ツリーに `(no spans)` としか表示されない

最も多い原因は、トレース対象のブロック内で計測対象のOMMX関数が呼ばれていないことです。コレクタは `capture_trace` ウィンドウ内で発生した `trace_id` のスパンのみをキャプチャしますが、スパンは計測されたコールサイトからのみ生成されます（素のPythonの制御フローからは生成されません）。ブロックの中に実際のOMMX呼び出し（`Instance.from_bytes`、`Instance.evaluate`、アダプタの `solve` など）が含まれているか確認してください。

もう1つの可能性は、非SDKの `TracerProvider` がアクティブで `ommx.tracing` がコレクタを取り付けられなかったケースです。この場合は `capture_trace` が `RuntimeError` を投げるので、そのメッセージに従って修正してください。

### OTLPバックエンドにはトレースが出ているのにセルマジックは `(no spans)` と表示される

コレクタは `trace_id` でキー付けされています。`capture_trace`（およびセルマジック）は、無関係な環境スパンがキャプチャウィンドウに混入するのを防ぐため、**新しい**OTelコンテキストから開始します。このため `trace_id` も新規に生成されます。つまり、無関係な親のもとで自分で `tracer.start_as_current_span(..., context=...)` を呼び出して作ったスパンはセルマジックの出力には現れません（OTLPには届いていますが）。セルマジックや `capture_trace` を最外側のスパンとし、自前のスパンはその内側にネストしてください。

### 並行処理とasync

`capture_trace` ブロック内では、同じ論理スレッドのスパンはOTelがコンテキスト変数経由で現在のスパンを伝播するため、正しくネストされます。ただし以下の点に注意してください。

- **ブロック外で開始したバックグラウンドスレッド**は、ブロックのOTelコンテキストを継承しません。これらのスレッドのスパンはキャプチャされません。
- **`asyncio.create_task` でスケジュールしたasyncioタスク**は作成時点の `contextvars.Context` をコピーするので、`capture_trace` ブロック内で作成したタスクはキャプチャされます。ブロック外で作成したタスクはキャプチャされません。
- `async def` 関数には `@traced` を使用してください。デコレータがトレースブロック内で `await` してくれます。

### テキストツリー／セル出力にスパンが空で表示される

持続時間が `0.0 µs` と表示されるスパンは、ほぼ確実にスパンが終了する前にレンダラに到達しています（どこかで `start_as_current_span` の後処理が抜けている）。レンダラはクラッシュを避けるために `0.0` のフォールバックを返します。開いたスパンのコンテキストマネージャがすべて閉じられていることを確認してください。最も多い原因は、手動で `tracer.start_span(...)` を呼び出して終了し忘れているケースです。

### 最初の呼び出しの挙動について

Rust → Python OTelブリッジはエクスポートのたびにアクティブな `TracerProvider` を解決するので、プログラム実行中にprovider を切り替えるのは安全です。ただし**pyo3拡張は最初の呼び出し時に計測済みのsubscriberをキャッシュします**。providerを設定しない状態で最初に `ommx` をimportしたあとにOTLPエクスポータを追加しようとしても、後続の呼び出しから出るスパンはすでにインストール済みのsubscriberを経由してしまいます。OTLPエクスポートが必要な場合は、**OMMXへの最初の呼び出し前に**providerを設定してください。
