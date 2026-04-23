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

# トレースとプロファイリング

OMMXは主要なエントリポイントから [OpenTelemetry](https://opentelemetry.io/) のスパンを出力します。これを人間が読める形にまとめるための薄いラッパーが `ommx.tracing` に2つ用意されています。

- **`%%ommx_trace`** — 単一セルの実行中に発生したスパンをネストしたテキストツリーとして表示するJupyterセルマジック。加えてChrome Trace Event Format形式のJSONダウンロードリンクも表示されます。
- **{class}`~ommx.tracing.capture_trace` / {func}`~ommx.tracing.traced`** — 同じ仕組みを通常のPythonスクリプト、テスト、CIから利用するためのコンテキストマネージャとデコレータ。

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

1. セル内で発生した全スパンを**ネストしたテキストツリー**として描画。各ノードには持続時間と代表的な属性が付与されます。
2. Chrome Trace Event Format形式のトレース全体の**ダウンロードリンク**。生成されたJSONファイルを [Perfetto](https://ui.perfetto.dev/) / [speedscope](https://www.speedscope.app/) / `chrome://tracing` に読み込ませることでフレームグラフとして閲覧できます。

```{note}
このセル出力（テキストツリー+ダウンロードリンク）は最小限の開始点として提供しているものであり、今後改善される予定です（例: セル内へのインタラクティブなフレームグラフの埋め込みなど）。レイアウトやマークアップの細部は不安定として扱ってください。
```

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

`trace` はブロック終了時に値が埋められる {class}`~ommx.tracing.TraceResult` です。

- {attr}`~ommx.tracing.TraceResult.spans` — カスタム処理用の生の {class}`~opentelemetry.sdk.trace.ReadableSpan` のリスト
- {meth}`~ommx.tracing.TraceResult.text_tree` — セルマジックと同じネストしたテキストツリー
- {meth}`~ommx.tracing.TraceResult.chrome_trace_json` — トレースをJSON文字列として返す
- {meth}`~ommx.tracing.TraceResult.save_chrome_trace` — JSONをディスクに書き出す（必要な親ディレクトリは自動的に作成）

ブロック内で例外が発生した場合でも `trace.spans` は埋められており（失敗したスパンには `[ERROR]` マーカーが付く）、外側の `except` や `finally` から内容を調査・保存できます。元の例外はそのまま伝播します。OMMXが例外を握り潰すことはありません。

```{code-cell} ipython3
import tempfile
from pathlib import Path

output_path = Path(tempfile.gettempdir()) / "ommx_trace.json"
trace.save_chrome_trace(output_path)
print(f"{output_path} に {output_path.stat().st_size} bytes 書き出しました")
```

### デコレータ (`@traced`)

{func}`~ommx.tracing.traced` は {class}`~ommx.tracing.capture_trace` の糖衣構文です。

```{code-cell} ipython3
import tempfile
from pathlib import Path

from ommx.tracing import traced

evaluate_output = Path(tempfile.gettempdir()) / "evaluate_trace.json"

@traced(output=str(evaluate_output))
def evaluate_once(inst):
    return inst.evaluate({0: 1.0, 1: 1.0})

solution = evaluate_once(instance)
print(f"トレースを {evaluate_output} に書き出しました")
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
- `output` を指定した場合、正常終了時はChrome Trace JSONが書き出されます。例外発生時も書き出しを試みますが、保存時のI/Oエラーなどは元の例外を上書きしないよう抑制されるため、例外パスでの保存は best-effort です。
- `async def` もサポートされています。`inspect.iscoroutinefunction` でコルーチン関数を検知し、トレースブロック内で `await` します。この検知がないと、コルーチン生成直後にキャプチャウィンドウが閉じてしまい、スパンが全て失われます。

## スパン命名規則

OMMXは `tracing` のデフォルトのスパン名（`evaluate`、`reduce_capabilities`、`push`、`pull` などの関数名そのまま）を採用しています。モジュールパスはOTelの**インストルメンテーションスコープ**に記録されるので、同じ関数名でもスコープ名から区別可能です。

複数の型で同名のメソッドが存在する場合（例: `OciArchive` と `Remote` ストレージバックエンドでの `Artifact::push`）、Rust側では独自のスパン名ではなくスパン**フィールド** (`fields(artifact_storage = "oci_archive")` など) で区別します。これらのフィールドはツリー表示ではOTel属性として、Chrome Traceでは `args` 辞書として表示されます。

(own-tracer-provider)=
## 独自のTracerProviderを使う

`ommx.tracing` は {class}`~opentelemetry.sdk.trace.TracerProvider` が登録されていない場合のみインプロセスの `TracerProvider` をインストールします。OTLP、Jaeger、Honeycombなどの外部バックエンドに送信したい場合は、**OMMX拡張への最初の呼び出し前に**独自のプロバイダを設定してください。

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

1. **providerの設定は `ommx.tracing` の初回利用前、かつRust拡張の最初の呼び出し前に行ってください。** Python OTel APIでは最初の {func}`~opentelemetry.trace.set_tracer_provider` 呼び出しのみが有効で、かつ `ommx.tracing` は初回利用時にproviderが未設定であればデフォルトの {class}`~opentelemetry.sdk.trace.TracerProvider` を自らインストールします。このため、そのあとでユーザーが `set_tracer_provider(your_provider)` を呼んでも無視されます。また、Rust→Pythonブリッジは最初の計測対象Rust呼び出し時に初期化されるため、OTel設定はスクリプトやノートブックの最上部で行うようにしてください。
2. **`ommx.tracing` はアクティブなproviderにコレクタを追加するだけで、置き換えは行いません。** スパンはOMMXのレンダラと指定したOTLPエクスポータの両方に到達します。

{meth}`~opentelemetry.sdk.trace.TracerProvider.add_span_processor` をサポートしない非SDKのproviderがアクティブな場合（稀ですが一部のベンダーSDKはこの挙動です）、{class}`~ommx.tracing.capture_trace` は `__enter__` 時点で `RuntimeError` を送出します。エラーメッセージに記載されているとおり、{class}`opentelemetry.sdk.trace.TracerProvider` を自前でインストールし、エクスポータをもう一つの `SpanProcessor` として同じproviderに追加してください。

## トラブルシューティング

### ツリーに `(no spans)` としか表示されない

最も多い原因は、トレース対象のブロック内で計測対象のOMMXコールサイトに到達していないことです。コレクタは {class}`~ommx.tracing.capture_trace` ウィンドウ内で発生した `trace_id` のスパンのみをキャプチャしますが、スパンは計測されたコールサイトからのみ生成されます（素のPythonの制御フローからは生成されません）。OMMXのすべてのメソッドが計測対象というわけではなく、コンストラクタや単純なアクセサは通常計測されません。ブロックが計測対象呼び出し（`Instance.evaluate`、`Instance.evaluate_samples`、`Instance.reduce_capabilities`、`Artifact` の `push` / `pull` / `load` / `save` 等のエントリポイント、アダプタの `solve` など）に到達しているか確認してください。

もう1つの可能性は、非SDKの {class}`~opentelemetry.sdk.trace.TracerProvider` がアクティブで `ommx.tracing` がコレクタを取り付けられなかったケースです。この場合は {class}`~ommx.tracing.capture_trace` が `RuntimeError` を投げるので、そのメッセージに従って修正してください。

### OTLPバックエンドにはトレースが出ているのにセルマジックは `(no spans)` と表示される

コレクタは `trace_id` でキー付けされています。{class}`~ommx.tracing.capture_trace`（およびセルマジック）は、無関係な環境スパンがキャプチャウィンドウに混入するのを防ぐため、**新しい**OTelコンテキストから開始します。このため `trace_id` も新規に生成されます。つまり、無関係な親のもとで自分で {meth}`tracer.start_as_current_span(..., context=...) <opentelemetry.trace.Tracer.start_as_current_span>` を呼び出して作ったスパンはセルマジックの出力には現れません（OTLPには届いていますが）。セルマジックや {class}`~ommx.tracing.capture_trace` を最外側のスパンとし、自前のスパンはその内側にネストしてください。

### 並行処理とasync

{class}`~ommx.tracing.capture_trace` ブロック内では、同じ論理スレッドのスパンはOTelがコンテキスト変数経由で現在のスパンを伝播するため、正しくネストされます。ただし以下の点に注意してください。

- **ブロック外で開始したバックグラウンドスレッド**は、ブロックのOTelコンテキストを継承しません。これらのスレッドのスパンはキャプチャされません。
- **{func}`asyncio.create_task` でスケジュールしたasyncioタスク**は作成時点の {class}`contextvars.Context` をコピーするので、{class}`~ommx.tracing.capture_trace` ブロック内で作成したタスクはキャプチャされます。ブロック外で作成したタスクはキャプチャされません。
- `async def` 関数には {func}`~ommx.tracing.traced` を使用してください。デコレータがトレースブロック内で `await` してくれます。

### テキストツリー／セル出力にスパンが空で表示される

持続時間が `0.0 µs` と表示されるスパンは、ほぼ確実にスパンが終了する前にレンダラに到達しています（どこかで `start_as_current_span` の後処理が抜けている）。レンダラはクラッシュを避けるために `0.0` のフォールバックを返します。開いたスパンのコンテキストマネージャがすべて閉じられていることを確認してください。最も多い原因は、手動で `tracer.start_span(...)` を呼び出して終了し忘れているケースです。

### 最初の呼び出しの挙動について

`ommx.tracing` やRust→Pythonブリッジの初期化後に、アクティブな {class}`~opentelemetry.sdk.trace.TracerProvider` の**差し替え**を前提にしないでください。初回利用時に以下の2つの状態が固定され、後から取り消せません。

1. `ommx.tracing` は {class}`capture_trace.__enter__ <ommx.tracing.capture_trace>` や最初の `%%ommx_trace` セル実行時、providerが未設定であれば {func}`~opentelemetry.trace.set_tracer_provider` に新規の {class}`~opentelemetry.sdk.trace.TracerProvider` を渡して呼び出します。Python OTelは最初の {func}`~opentelemetry.trace.set_tracer_provider` のみを採用するため、そのあとにユーザーがproviderを設定しても無視されます。
2. 基盤となる `pyo3-tracing-opentelemetry` ブリッジは、最初の計測対象Rust呼び出しの時点で `tracing` subscriberをプロセス全体にインストールします。providerを後から差し替えても、後続の呼び出しのスパンはすでにインストール済みのsubscriberを経由して流れます。

OTLPエクスポートが必要な場合は、**OMMXへの最初の呼び出し前に**providerを設定してください。実行中に挙動を調整したい場合は、providerを差し替えるのではなく、既存のSDK providerに {meth}`provider.add_span_processor(new_processor) <opentelemetry.sdk.trace.TracerProvider.add_span_processor>` のように `SpanProcessor` を追加してください。

## APIリファレンス

上記で紹介したシンボルの完全なシグネチャとdocstringは、ソースコードから自動生成された [`ommx.tracing` のautoapiページ](../autoapi/ommx/tracing/index.rst) を参照してください。
